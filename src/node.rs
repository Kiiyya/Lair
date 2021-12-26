use std::path::{Path, PathBuf};
use std::sync::{Arc, Weak};

use futures::future::try_join_all;

use crate::LairInner;
use crate::descriptor::Descriptor;
use crate::error::{BuildTtcError, ManifestFetchError, SourceFetchError};
use crate::lazy::Lazy;
use crate::manifest::Manifest;
use crate::tracing::Tracer;

/// A node in the dependency tree.
///
/// Contains weak references to Lair.
#[derive(Debug)]
pub struct Node<Tr: Tracer = ()> {
    pub descriptor: Descriptor,

    manifest: Lazy<Result<Manifest, ManifestFetchError>>,

    base_path: Lazy<Result<PathBuf, SourceFetchError>>,

    /// Compiled TTC files done? If yes, they can be found here (usually `{base_path}/build/ttc`).
    ttc: Lazy<Result<PathBuf, BuildTtcError>>,

    lair: Weak<LairInner<Tr>>,

    // /// Used to prevent dependency cycles and deadlocks.
    // depth: usize,
}

impl<Tr: Tracer> Node<Tr> {
    pub(crate) fn new(
        lair: Weak<LairInner<Tr>>,
        descriptor: Descriptor,
        manifest: Lazy<Result<Manifest, ManifestFetchError>>,
        base_path: Lazy<Result<PathBuf, SourceFetchError>>,
        ttc: Lazy<Result<PathBuf, BuildTtcError>>,
    ) -> Self {
        Self {
            descriptor,
            manifest,
            base_path,
            ttc,
            lair,
        }
    }

    pub(crate) fn new_partial(
        lair: Weak<LairInner<Tr>>,
        descriptor: Descriptor,
        manifest: Manifest,
        base_path: impl AsRef<Path>,
        ttc: Lazy<Result<PathBuf, BuildTtcError>>,
    ) -> Self {
        Self {
            descriptor,
            manifest: Lazy::new_immediate(Ok(manifest)),
            base_path: Lazy::new_immediate(Ok(base_path.as_ref().to_owned())),
            ttc,
            lair,
        }
    }

    /// Arc-Upgrade `lair`, panic if fails.
    fn lair(&self) -> Arc<LairInner<Tr>> {
        self.lair.upgrade().expect("Failed to upgrade lair weak Arc.")
    }

    /// Package name, for example `AmazingTool`.
    pub fn name(&self) -> &str {
        self.descriptor.name()
    }

    /// If the package name is `AmazingTool`, then this will usually be
    /// `{base_path}/src/AmazingTool.idr`.
    pub async fn main(&self) -> Result<PathBuf, SourceFetchError> {
        Ok(self.base_path().await?.join("src").join(format!("{}.idr", self.name())))
    }

    pub async fn manifest(&self) -> Result<Manifest, ManifestFetchError> {
        self.manifest.get().await
    }

    /// Base path, so that `{base_path}/Egg.toml`.
    /// Download sources if necessary.
    pub async fn base_path(&self) -> Result<PathBuf, SourceFetchError> {
        self.base_path.get().await
    }

    /// TTC path, usually `{base_path}/build/ttc`.
    pub async fn ttc(&self) -> Result<PathBuf, BuildTtcError> {
        self.ttc.get().await
    }

    pub async fn dependencies(&self) -> Result<Vec<Arc<Node<Tr>>>, ManifestFetchError> {
        let lair = self.lair();
        let manifest = self.manifest().await?;
        let ret = manifest.dependencies.iter()
            .map(|dep| lair.node(dep))
            .collect();
        Ok(ret)
    }

    pub async fn dependencies_ttc_paths(&self) -> Result<Vec<PathBuf>, BuildTtcError> {
        let mut tmp = self.dependencies().await?;
        let futures = tmp.drain(..)
            .map(|dep| async move { dep.ttc().await });

        try_join_all(futures).await
    }
}
