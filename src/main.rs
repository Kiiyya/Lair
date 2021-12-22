#![feature(exit_status_error)]
#![feature(map_try_insert)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::{fs::create_dir_all, io::ErrorKind, path::Path};

use descriptor::Descriptor;
use futures::future::{join_all, join, try_join, try_join_all};
use itertools::Itertools;
use lazy::Lazy;
use maplit::btreemap;
use structopt::StructOpt;

use crate::egg::Manifest;

pub mod egg;
pub mod lazy;
pub mod descriptor;


/// A node in the dependency tree. Uses interior mutability, so is clone-able.
#[derive(Debug, Clone)]
pub struct Node {
    pub manifest: Arc<Lazy<Result<Manifest, ManifestFetchError>>>,

    /// Base path, so that `{source_path}/Egg.toml`.
    pub source_path: Arc<Lazy<Result<PathBuf, SourceFetchError>>>,

    /// Compiled TTC files done? If yes, they can be found here (usually `{source_path}/build/ttc`).
    pub ttc: Arc<Lazy<Result<PathBuf, BuildTtcError>>>,
}

impl Node {
    pub fn new(
        manifest: Lazy<Result<Manifest, ManifestFetchError>>,
        source_path: Lazy<Result<PathBuf, SourceFetchError>>,
        ttc: Lazy<Result<PathBuf, BuildTtcError>>,
    ) -> Self {
        Self {
            manifest: Arc::new(manifest),
            source_path: Arc::new(source_path),
            ttc: Arc::new(ttc),
        }
    }

    pub fn new_partial(
        manifest: Manifest,
        source_path: impl AsRef<Path>,
        ttc: Lazy<Result<PathBuf, BuildTtcError>>,
    ) -> Self {
        Self {
            manifest: Arc::new(Lazy::new_immediate(Ok(manifest))),
            source_path: Arc::new(Lazy::new_immediate(Ok(source_path.as_ref().to_owned()))),
            ttc: Arc::new(ttc),
        }
    }

    pub fn new_full(
        manifest: Manifest,
        source_path: impl AsRef<Path>,
        ttc: impl AsRef<Path>,
    ) -> Self {
        Self {
            manifest: Arc::new(Lazy::new_immediate(Ok(manifest))),
            source_path: Arc::new(Lazy::new_immediate(Ok(source_path.as_ref().to_owned()))),
            ttc: Arc::new(Lazy::new_immediate(Ok(ttc.as_ref().to_owned()))),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SourceFetchError {
    #[error("Dummy")]
    Dummy(Arc<anyhow::Error>),

    #[error("Dummy")]
    GitError(Arc<git2::Error>),
}

impl From<git2::Error> for SourceFetchError {
    fn from(err: git2::Error) -> Self {
        Self::GitError(Arc::new(err))
    }
}

impl From<anyhow::Error> for SourceFetchError {
    fn from(e: anyhow::Error) -> Self {
        Self::Dummy(Arc::new(e))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum BuildTtcError {
    #[error("Dummy")]
    Dummy(Arc<anyhow::Error>),

    #[error("Failed to fetch source: {0}")]
    SourceFetch(#[from] SourceFetchError),

    #[error("Failed to fetch manifest: {0}")]
    ManifestFetch(#[from] ManifestFetchError),
}

impl From<anyhow::Error> for BuildTtcError {
    fn from(e: anyhow::Error) -> Self {
        Self::Dummy(Arc::new(e))
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ManifestFetchError {
    #[error("Dummy")]
    Dummy(Arc<anyhow::Error>),

    #[error("Failed to fetch source: {0}")]
    SourceFetch(#[from] SourceFetchError),

    #[error("File IO error: {0}")]
    Io(Arc<std::io::Error>),
}

impl From<anyhow::Error> for ManifestFetchError {
    fn from(e: anyhow::Error) -> Self {
        Self::Dummy(Arc::new(e))
    }
}


impl From<std::io::Error> for ManifestFetchError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(Arc::new(e))
    }
}


#[derive(Debug)]
struct LairInner {
    /// Flat collection of package descriptors associated with their data.
    db: std::sync::Mutex<BTreeMap<Descriptor, Node>>,
}

#[derive(Debug, Clone)]
pub struct Lair {
    inner: Arc<LairInner>,
}

impl Lair {
    pub fn new(root_manifest: Manifest, root_path: impl AsRef<Path>) -> (Self, Descriptor) {
        let myself = Self {
            inner: Arc::new(LairInner {
                db: std::sync::Mutex::new(BTreeMap::new()),
                // db: std::sync::Mutex::new(btreemap!{
                //     root_descriptor => root_node
                // }),
            })
        };
        let root_descriptor = Descriptor::Root { name: root_manifest.name.clone() };

        let myself_clone = myself.clone();
        let root_descriptor_clone = root_descriptor.clone();

        let root_node = Node::new_partial(
            root_manifest,
            root_path.as_ref(),
            Lazy::new(async move { myself_clone.build_ttc(root_descriptor_clone).await } ),
        );
        let mut db = myself.inner.db.lock().unwrap();
        db.insert(root_descriptor.clone(), root_node);
        drop(db);

        (myself, root_descriptor)
    }

    /// Gets a node, or creates the node, which contains recipes on how to fetch or build
    /// the sources, TTC, manifest.
    pub fn get(&self, desc: Descriptor) -> Node {
        let mut db = self.inner.db.lock().unwrap();

        if let Some(node) = db.get(&desc) {
            node.clone()
        } else {
            let self_clone1: Lair = self.clone();
            let self_clone2: Lair = self.clone();
            let self_clone3: Lair = self.clone();
            let desc_clone1: Descriptor = desc.clone();
            let desc_clone2: Descriptor = desc.clone();
            let desc_clone3: Descriptor = desc.clone();

            // Create a new node, with recipes on how to obtain its source/ttcs, which will be
            // invoked when necessary.
            // This only creates futures, which may or may not be invoked in the future (harr harr).
            let node = Node::new(
                Lazy::new(async move { self_clone1.fetch_manifest(desc_clone1).await }),
                Lazy::new(async move { self_clone2.fetch_source(desc_clone2).await }),
                Lazy::new(async move { self_clone3.build_ttc(desc_clone3).await } ),
            );
            db.insert(desc.clone(), node.clone());
            node
        }
    }

    async fn build_ttc(&self, desc: Descriptor) -> Result<PathBuf, BuildTtcError> {
        println!("{} [TTC] Starting...", desc.name());
        let node = self.get(desc.clone());
        let manifest = node.manifest.get().await?;

        // Build dependencies in parallel (and recurse, kind of). Then unpack results, making sure
        // they all built correctly, and collect into an IDRIS2_PATH.

        let futures = manifest.dependencies
            .iter()
            .cloned()
            .map(|d| async move {
                self.get(d).ttc.clone().get().await
            });
        let (source_path, results): (Result<PathBuf, SourceFetchError>, Vec<Result<PathBuf, BuildTtcError>>)
            = join(node.source_path.get(), join_all(futures)).await;

        let source_path = source_path?;
        let ttc_paths: Result<Vec<PathBuf>, _> = results.iter().cloned().collect(); // unwrap all Result<,>s.
        let ttc_paths = ttc_paths?;

        let idris2_path = ttc_paths.iter().map(|p| p.to_string_lossy()).join(":");
        Command::new("idris2")
            .current_dir(&source_path)
            .arg("--source-dir").arg("src")
            .arg("--check")
            .env("IDRIS2_PATH", &idris2_path)
            .arg(format!("src/{}.idr", desc.name()))
            .status().unwrap().exit_ok().unwrap(); // TODO: fix both unwraps here.

        println!("{} [TTC] Done (IDRIS2_PATH was \"{}\")", desc.name(), idris2_path);

        let mut ttc_path = source_path;
        ttc_path.push("build");
        ttc_path.push("ttc");
        Ok(ttc_path)
    }

    /// Returns path to source code, so that `{return value}/Egg.toml` exists.
    async fn fetch_source(&self, desc: Descriptor) -> Result<PathBuf, SourceFetchError> {
        println!("{} [SRC] Starting...", desc.name());
        match desc.clone() {
            Descriptor::Root { .. } => {
                unreachable!("There must only be one root node, and it must be initialized with a path (usually `./`) at startup.")
            },
            Descriptor::Git { name, url, version } => {
                let path = PathBuf::from(format!("build/deps/{}", name)); // TODO: make sure directory doesn't exist yet.
                let path_clone = path.clone();

                let _repo = tokio::task::spawn_blocking(move || {
                    git2::Repository::clone(&url, &path_clone)
                }).await.unwrap()?;

                println!("{} [SRC] Done, git-cloned {} into {:?}.", desc.name(), name, path);
                Ok(path)
            },
            Descriptor::Local { name, path } => todo!(),
        }
    }

    async fn fetch_manifest(&self, desc: Descriptor) -> Result<Manifest, ManifestFetchError> {
        println!("{} [MAN] Starting...", desc.name());
        let node = self.get(desc.clone());
        let mut path = node.source_path.get().await?;
        path.push("Egg.toml");

        let ret = egg::Manifest::from_string(std::fs::read_to_string(path)?)?;
        println!("{} [MAN] Done.", desc.name());
        Ok(ret)
    }
}

/// Ensure a directory and sub-dirs are gone.
/// Do not fail when it's not there in the first place.
fn clean(path: impl AsRef<Path>) -> Result<(), std::io::Error> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Command-line thingie.
#[derive(Debug, StructOpt)]
#[structopt(about = "Derp")]
enum Opt {
    Build,
    Clean,
}

async fn real_main() -> anyhow::Result<()> {
    std::env::set_current_dir("/mnt/e/hax/2021/NotJson")?;

    let opt: Opt = Opt::from_args();

    let egg: Manifest = egg::Manifest::from_string(std::fs::read_to_string("Egg.toml")?)?;

    match opt {
        Opt::Build => {
            create_dir_all("build/deps")?;

            let (lair, root) = Lair::new(egg, "./");
            let root = lair.get(root);
            let ttc_path = root.ttc.get().await?;
            println!("Done! TTCs are in {}", ttc_path.to_string_lossy());

            // this will recursively build everything.
            // let ttcs = root.ttc.get().await?;
        },
        Opt::Clean => {
            clean("build")?;
        },
    }

    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    real_main().await
}