use std::path::PathBuf;

use itertools::Itertools;

#[cfg(target_os = "windows")]
pub const PATH_SEP: &str = ";";
#[cfg(not(target_os = "windows"))]
pub const PATH_SEP: &str = ":";

pub trait Idris2Paths {
    /// Join paths with ":" or ";" as separator, depending on OS.
    ///
    /// Resulting in, for example:
    ///
    /// `build/deps/CoolCollections/build/ttc:build/deps/NotJson/build/ttc`
    fn join_idris2(&self) -> String;
}

impl Idris2Paths for Vec<PathBuf> {
    fn join_idris2(&self) -> String {
        self.iter()
            .map(|path| path.to_str().unwrap()) // TODO: use OsStr instead.
            .join(PATH_SEP)
    }
}
