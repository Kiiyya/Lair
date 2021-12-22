//! Reading `Egg.toml`.
use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::Descriptor;
use crate::descriptor::GitVersion;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct EggTopDecl {
    /// Name of the package, for example "CoolCollections".
    name: String,

    /// SemVer like "0.1.0".
    version: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EggDependency {
    /// Url to git repository, for example `https://github.com/Kiiyya/CoolCollections`.
    git: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct EggManifest {
    #[serde(rename = "egg")]
    topdecl: EggTopDecl,

    /// Package name --> (where to find it, version, etc...).
    dependencies: BTreeMap<String, EggDependency>,
}

#[derive(Clone, Debug)]
pub struct Manifest {
    pub name: String,
    pub version: String,

    pub dependencies: BTreeSet<Descriptor>,
}

impl Manifest {
    // ugly, but for now...
    pub fn from_string(s: impl AsRef<str>) -> Result<Manifest, anyhow::Error> {
        let egg: EggManifest = toml::from_str(s.as_ref())?;
        let manifest = Self {
            name: egg.topdecl.name,
            version: egg.topdecl.version,
            dependencies: egg.dependencies.iter().map(|(name, dep)|
                Descriptor::Git {
                    name: name.to_owned(),
                    url: dep.git.to_owned(),
                    version: GitVersion::Branch("main".to_string()),
                }
            ).collect(),
        };

        Ok(manifest)
    }
}
