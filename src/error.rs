use std::sync::Arc;


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

