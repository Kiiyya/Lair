use std::path::PathBuf;


/// A git repository alone isn't enough to determine the source code version to use.
/// We may want a specific branch or tag to be used instead.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum GitVersion {
    /// E.g. `main`.
    Branch(String),
    /// Full commit hash.
    Rev(String),
    /// Some git tag.
    Tag(String),
}

/// *Dependency descriptor*: package name together with version. Enough to info to find and download
/// the source code. This is just POD.
///
/// This should determine the exact source code
/// Ideally (loc1 == loc2 ==> hash(loc1.download()) == hash(loc2.download())), assuming same point
/// in time.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Descriptor {
    Root {
        name: String,
    },

    Git {
        /// Package name, for example `CoolCollections`.
        name: String,
        url: String,
        /// Do we refer to a branch, commit hash, or tag?
        version: GitVersion,
    },

    /// Origin of source code is somewhere on the local computer.
    ///
    /// In the future, maybe make a distinction between local to the workspace and local to some
    /// arbitrary absolute path?
    Local {
        /// Package name, for example `CoolCollections`.
        name: String,
        path: PathBuf,
    },
}

impl Descriptor {
    /// Get the package name, for example `CoolCollections`.
    pub fn name(&self) -> &str {
        match self {
            Descriptor::Git { name, .. } => name,
            Descriptor::Local { name, .. } => name,
            Descriptor::Root { name } => name,
        }
    }
}
