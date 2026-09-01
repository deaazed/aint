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
/// deliberately isn't root-relative yet). `path` always points at
/// *where the resolved source actually lives on disk* - for a git
/// dependency (milestone 36), that's its local cache directory, not
/// the URL; `source` records where it actually came from, so the
/// lockfile stays meaningful to a human reading it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LockedPackage {
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub source: Option<GitSource>,
}

/// Where a git dependency actually came from, and the exact commit it
/// resolved to - recorded even if `rev` (in `aint.toml`) was a moving
/// branch name, so the lockfile stays reproducible the way
/// `Cargo.lock` recording an exact git commit is. See SPEC.md.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GitSource {
    pub git: String,
    pub commit: String,
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
                    source: None,
                },
                LockedPackage {
                    name: "some-lib".to_string(),
                    version: "0.2.0".to_string(),
                    path: Some("/abs/path/some-lib".to_string()),
                    source: None,
                },
                LockedPackage {
                    name: "git-lib".to_string(),
                    version: "0.3.0".to_string(),
                    path: Some("/abs/path/cache/git-lib".to_string()),
                    source: Some(GitSource {
                        git: "https://github.com/user/git-lib".to_string(),
                        commit: "abc123".to_string(),
                    }),
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
