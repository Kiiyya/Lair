#![feature(exit_status_error)]
#![feature(map_try_insert)]

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::{fs::create_dir_all, io::ErrorKind, path::Path};

use descriptor::Descriptor;
use lazy::Lazy;
use maplit::btreemap;
use structopt::StructOpt;
use tokio::sync::Mutex;

use crate::egg::Manifest;

pub mod egg;
pub mod lazy;
pub mod descriptor;


/// A node in the dependency tree. Uses interior mutability, so is clone-able.
#[derive(Debug, Clone)]
pub struct Node {
    pub manifest: Arc<Lazy<Result<Manifest, anyhow::Error>>>,

    /// Base path, so that `{source_path}/Egg.toml`.
    pub source_path: Arc<Lazy<Result<PathBuf, SourceFetchError>>>,

    /// Compiled TTC files done? If yes, they can be found here (usually `{source_path}/build/ttc`).
    pub ttc: Arc<Lazy<Result<PathBuf, anyhow::Error>>>,
}

impl Node {
    pub fn new(
        manifest: Lazy<Result<Manifest, anyhow::Error>>,
        source_path: Lazy<Result<PathBuf, SourceFetchError>>,
        ttc: Lazy<Result<PathBuf, anyhow::Error>>,
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
        ttc: Lazy<Result<PathBuf, anyhow::Error>>,
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

#[derive(Debug)]
struct LairInner {
    /// Flat collection of package descriptors associated with their data.
    db: Mutex<BTreeMap<Descriptor, Node>>,
}

#[derive(Debug, Clone)]
pub struct Lair {
    inner: Arc<LairInner>,
}

impl Lair {
    pub fn new(root_manifest: Manifest, root_path: impl AsRef<Path>) -> (Self, Descriptor) {
        let root_descriptor = Descriptor::Root { name: root_manifest.name.clone() };
        let root_node = Node::new_partial(
            root_manifest,
            root_path.as_ref(),
            Lazy::new(async { todo!() }),
        );

        let root_descriptor_clone = root_descriptor.clone();

        let myself = Self {
            inner: Arc::new(LairInner {
                db: Mutex::new(btreemap!{
                    root_descriptor => root_node
                }),
            })
        };

        (myself, root_descriptor_clone)
    }

    pub async fn get(&self, desc: Descriptor) -> Node {
        let mut db = self.inner.db.lock().await;
        {
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
                let node = {Node::new(
                    Lazy::new(async move { self_clone1.fetch_manifest(desc_clone1).await }),
                    Lazy::new(async move { self_clone2.fetch_source(desc_clone2).await }),
                    Lazy::new(async move { self_clone3.build_ttc(desc_clone3).await } )
                )};
                db.insert(desc.clone(), node.clone()).unwrap();
                node
            }
        }
    }

    async fn build_ttc(&self, desc: Descriptor) -> Result<PathBuf, anyhow::Error> {
        todo!()
    }

    /// Returns path to source code, so that `{return value}/Egg.toml` exists.
    async fn fetch_source(&self, desc: Descriptor) -> Result<PathBuf, SourceFetchError> {
        match desc {
            Descriptor::Root { name } => todo!(),
            Descriptor::Git { name, url, version } => {
                let path = PathBuf::from(format!("build/deps/{}", name)); // TODO: make sure directory doesn't exist yet.
                let _repo = git2::Repository::clone(&url, &path)?;
                println!("Git-cloned {} into {:?}", name, path);
                Ok(path)
            },
            Descriptor::Local { name, path } => todo!(),
        }
    }

    async fn fetch_manifest(&self, desc: Descriptor) -> Result<Manifest, anyhow::Error> {
        let node = self.get(desc).await;
        let mut path = node.source_path.get().await?;
        path.push("Egg.toml");

        egg::Manifest::from_string(std::fs::read_to_string(path)?)
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
            let root = lair.get(root).await;

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