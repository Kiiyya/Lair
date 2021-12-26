#![feature(exit_status_error)]
#![feature(map_try_insert)]
#![feature(arc_new_cyclic)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::{fs::create_dir_all, io::ErrorKind, path::Path};

use anyhow::Context;
use descriptor::Descriptor;
use error::{ManifestFetchError, SourceFetchError, BuildTtcError};
use futures::future::join;
use lazy::Lazy;
use maplit::btreemap;
use node::Node;
use structopt::StructOpt;
use tracing::simple::SimpleTracer;
use tracing::{Tracer, SourceProgress, BuildProgress, ManifestProgress, SourceProgressMethod};

use crate::manifest::Manifest;
use crate::paths::Idris2Paths;

pub mod manifest;
pub mod lazy;
pub mod descriptor;
pub mod error;
pub mod node;
pub mod paths;
pub mod tracing;

#[derive(Debug)]
struct LairInner<Tr: Tracer = ()> {
    /// Flat collection of package descriptors associated with their data.
    db: std::sync::Mutex<BTreeMap<Descriptor, Arc<Node<Tr>>>>,

    /// The root node, i.e. our root package.
    root: Arc<Node<Tr>>,

    tracer: Tr,
}

#[derive(Debug, Clone)]
pub struct Lair<Tr: Tracer = ()> {
    inner: Arc<LairInner<Tr>>,
}

impl<Tr: Tracer> Lair<Tr> {
    /// Does not start anything yet, only initializes the root node with recipes.
    ///
    /// You can then try getting the TTC files for the root node, which will trigger the
    /// recipes stored inside the root node, which in turn will trigger fetching its dependencies'
    /// manifests, sources, TTCs, and so forth recursively.
    pub fn new(root_manifest: Manifest, root_path: impl AsRef<Path>) -> Self
        where Tr: Default
    {
        let root_descriptor = Descriptor::Root { name: root_manifest.name.clone() };
        let root_descriptor_clone = root_descriptor.clone();
        let root_descriptor_clone2 = root_descriptor.clone();

        let inner: Arc<LairInner<Tr>> = Arc::new_cyclic(move |weak| {
            let weak = weak.clone();
            let root_node = Arc::new(Node::new_partial(
                weak.clone(),
                root_descriptor.clone(),
                root_manifest,
                root_path.as_ref(),
                Lazy::new(async move {
                    let inner: Arc<LairInner<Tr>> = weak.upgrade().context("Failed to upgrade weak Arc.")?;
                    inner.build_ttc(root_descriptor_clone).await
                }),
            ));

            LairInner {
                db: Mutex::new(btreemap! {
                    root_descriptor => root_node.clone(),
                }),
                root: root_node,
                tracer: Tr::default(),
            }
        });

        inner.tracer.new_descriptor(&root_descriptor_clone2);
        Self { inner }
    }

    /// Get the root node.
    pub fn root(&self) -> &Node<Tr> {
        &self.inner.root
    }

    /// Gets a node, or creates the node, which contains recipes on how to fetch or build
    /// the sources, TTC, manifest.
    ///
    /// We return a [`Node`], which uses [`Arc`] (reference counting) internally.
    /// We don't return `&Node`, since that would violate our internal Mutex.
    pub fn node(&self, desc: &Descriptor) -> Arc<Node<Tr>> {
        self.inner.node(desc)
    }

    pub async fn build(&self) -> Result<(), anyhow::Error> {
        let build_deps_dir = PathBuf::from("build").join("deps");
        create_dir_all(build_deps_dir)?; // ./build/deps

        self.root().ttc().await?;

        Ok(())
    }

    pub async fn run(&self) -> Result<(), anyhow::Error> {
        let deps_ttc_paths = self.root().dependencies_ttc_paths().await?; // will complete instantly, because we've already built everything.

        Command::new("idris2")
            .env("IDRIS2_PATH", deps_ttc_paths.join_idris2())
            .arg("--source-dir").arg("src")
            .arg(self.root().main().await?)
            .arg("--exec").arg("main")
            .status().unwrap().exit_ok().unwrap(); // TODO: fix both unwraps here, check for errors idris returned.

        Ok(())
    }
}

impl<Tr: Tracer> LairInner<Tr> {
    pub fn node(self: &Arc<Self>, desc: &Descriptor) -> Arc<Node<Tr>> {
        let mut db = self.db.lock().unwrap();

        if let Some(node) = db.get(desc) {
            node.clone()
        } else {
            let desc_clone1: Descriptor = desc.clone();
            let desc_clone2: Descriptor = desc.clone();
            let desc_clone3: Descriptor = desc.clone();

            // Create a new node, with recipes on how to obtain its source/ttcs, which will be
            // invoked when necessary.
            // This only creates futures, which may or may not be invoked in the future (harr harr).
            let node = Arc::new(Node::new(
                Arc::downgrade(self),
                desc.clone(),
                Lazy::new_weak(self, move |lair| async move { lair.fetch_manifest(desc_clone1).await }),
                Lazy::new_weak(self, move |lair| async move { lair.fetch_source(desc_clone2).await }),
                Lazy::new_weak(self, move |lair| async move { lair.build_ttc(desc_clone3).await }),
            ));

            self.tracer.new_descriptor(desc);

            db.insert(desc.clone(), node.clone());
            node
        }
    }

    /// Recipe for building TTC files.
    async fn build_ttc(self: &Arc<Self>, desc: Descriptor) -> Result<PathBuf, BuildTtcError> {
        let node = self.node(&desc);

        // Build dependencies in parallel (and recurse, kind of). Then unpack results, making sure
        // they all built correctly, and collect into an IDRIS2_PATH.
        let (base_path, deps_paths) = join(node.base_path(), node.dependencies_ttc_paths()).await;
        let deps_paths = deps_paths?;
        let base_path = base_path?;

        let guard = self.tracer.building(&desc);
        let build_dir = base_path.join("build"); // `{base_path}/build`
        let source_dir = base_path.join("src"); // `{base_path}/src`
        let main_idr = node.main().await?; // `{base_path}/src/AmazingTool.idr`
        let idris2_path = deps_paths.join_idris2();

        // println!("{} [TTC] Running command: `idris2 --build-dir {} --source-dir {} --check {}` with IDRIS2_PATH=\"{}\"",
        //     desc.name(), build_dir.to_string_lossy(), source_dir.to_string_lossy(), main_idr.to_string_lossy(), idris2_path);

        Command::new("idris2")
            .arg("--build-dir").arg(build_dir)
            .arg("--source-dir").arg(source_dir)
            .arg("--check")
            .env("IDRIS2_PATH", &idris2_path)
            .arg(main_idr)
            .status().unwrap().exit_ok().unwrap(); // TODO: fix both unwraps here, check for errors idris returned.

        let ttc = base_path.join("build").join("ttc"); // `{base_path}/build/ttc`
        guard.success(&ttc);
        Ok(ttc)
    }

    /// Recipe for fetching source.
    ///
    /// Returns path to source code, so that `{return value}/Egg.toml` exists.
    async fn fetch_source(self: &Arc<Self>, desc: Descriptor) -> Result<PathBuf, SourceFetchError> {

        match desc.clone() {
            Descriptor::Root { .. } => {
                unreachable!("There must only be one root node, and it must be initialized with a path (usually `./`) at startup.")
            },
            Descriptor::Git { name, url, .. } => {
                let path = PathBuf::from(format!("build/deps/{}", name)); // TODO: make sure directory doesn't exist yet.

                if path.exists() {
                    let guard =self.tracer
                        .fetching_repo(&desc, SourceProgressMethod::AlreadyDownloaded);
                    guard.success(&path);
                    Ok(path)
                } else {
                    let guard = self.tracer.fetching_repo(&desc,
                        SourceProgressMethod::Git { url: &url} );
                    let path_clone = path.clone();
                    let _repo = tokio::task::spawn_blocking(move || {
                        // TODO: proper error handling.
                        git2::Repository::clone(&url, &path_clone)
                    }).await.unwrap()?;

                    guard.success(&path);
                    Ok(path)
                }
            },
            Descriptor::Local { .. } => todo!(),
        }
    }

    /// Recipe for fetching manifest.
    async fn fetch_manifest(self: &Arc<Self>, desc: Descriptor) -> Result<Manifest, ManifestFetchError> {
        let guard = self.tracer.fetching_manifest(&desc);

        let node = self.node(&desc);
        let path = node.base_path().await?.join("Egg.toml");

        let ret = manifest::Manifest::from_string(std::fs::read_to_string(path)?)?;
        guard.success(&ret);
        Ok(ret)
    }

}

/// Ensure a directory and sub-dirs are gone.
/// Do not fail when it's not there in the first place.
fn clean(path: impl AsRef<Path>) -> Result<(), anyhow::Error> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.into()),
    }
}

/// Command-line thingie.
#[derive(Debug, StructOpt)]
#[structopt(about = "Package manager for Idris2.")]
enum Opt {
    Build,
    Clean,
    Run,
}

async fn real_main() -> anyhow::Result<()> {
    // Read in command line options
    let opt: Opt = Opt::from_args();

    let manifest: Manifest = manifest::Manifest::from_string(std::fs::read_to_string("Egg.toml")?)?;

    match opt {
        Opt::Build => {
            let lair = Lair::<SimpleTracer>::new(manifest, "");
            lair.build().await?;

            Ok(())
        },
        Opt::Run => {
            let lair = Lair::<SimpleTracer>::new(manifest, "");
            lair.build().await?;
            lair.run().await?;

            Ok(())
        },
        Opt::Clean => {
            clean("build")
        },
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    real_main().await
}