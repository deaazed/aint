//! `aint.lock` — the resolved, flattened dependency graph `resolve.rs`
//! produces. See `docs/milestones/23-package-manager/SPEC.md`.

use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const LOCKFILE_FILE_NAME: &str = "aint.lock";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Lockfile {
    #[serde(rename = "package", default)]
    pub packages: Vec<LockedPackage>,
}

/// One resolved package - the root package itself (`path: None`) or a
/// dependency reached transitively (`path: Some(..)`, always an
/// absolute, canonicalized path - see `resolve.rs` for why this
/// deliberately isn't root-relative yet).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub path: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LockfileError {
    Io { path: String, message: String },
    Parse { path: String, message: String },
}

impl fmt::Display for LockfileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockfileError::Io { path, message } => write!(f, "could not read {path}: {message}"),
            LockfileError::Parse { path, message } => write!(f, "{path}: {message}"),
        }
    }
}

impl std::error::Error for LockfileError {}

impl Lockfile {
    pub fn parse(text: &str) -> Result<Self, LockfileError> {
        toml::from_str(text).map_err(|err| LockfileError::Parse {
            path: LOCKFILE_FILE_NAME.to_string(),
            message: err.message().to_string(),
        })
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string_pretty(self).expect("Lockfile always serializes")
    }

    pub fn write_to_dir(&self, dir: &Path) -> Result<(), LockfileError> {
        let path = dir.join(LOCKFILE_FILE_NAME);
        fs::write(&path, self.to_toml_string()).map_err(|err| LockfileError::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_toml() {
        let lockfile = Lockfile {
            packages: vec![
                LockedPackage {
                    name: "my-project".to_string(),
                    version: "0.1.0".to_string(),
                    path: None,
                },
                LockedPackage {
                    name: "some-lib".to_string(),
                    version: "0.2.0".to_string(),
                    path: Some("/abs/path/some-lib".to_string()),
                },
            ],
        };
        let text = lockfile.to_toml_string();
        let parsed = Lockfile::parse(&text).expect("should parse what we just wrote");
        assert_eq!(parsed, lockfile);
    }

    #[test]
    fn an_empty_lockfile_round_trips_too() {
        let lockfile = Lockfile::default();
        let text = lockfile.to_toml_string();
        let parsed = Lockfile::parse(&text).expect("should parse");
        assert_eq!(parsed, lockfile);
    }
}
