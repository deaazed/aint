//! `aint.toml` — the manifest every AINT package has. Named `aint.toml`,
//! not literally `axiom.toml` (`ROADMAP.md`'s "axiom.toml-equivalent"
//! meant "a Cargo.toml-shaped manifest," not that literal filename —
//! same reasoning Rust's own `Cargo.toml` isn't named after whatever
//! tool inspired it). See `docs/milestones/23-package-manager/SPEC.md`.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

pub const MANIFEST_FILE_NAME: &str = "aint.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub package: PackageMetadata,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PackageMetadata {
    pub name: String,
    pub version: String,
}

/// A dependency's declaration in `[dependencies]` — a local path, or
/// (milestone 36) a git source. `#[serde(untagged)]`: TOML's own shape
/// (`{ path = ".." }` vs `{ git = "..", rev = ".." }`) already
/// disambiguates which variant a table is, so no explicit tag field is
/// needed. Deliberately not a third, registry-backed variant — there's
/// still no name index to resolve a bare name against (see SPEC.md).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Path {
        path: String,
    },
    Git {
        git: String,
        #[serde(skip_serializing_if = "Option::is_none", default)]
        rev: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ManifestError {
    Io { path: String, message: String },
    Parse { path: String, message: String },
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManifestError::Io { path, message } => {
                write!(f, "could not read {path}: {message}")
            }
            ManifestError::Parse { path, message } => {
                write!(f, "{path}: {message}")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    pub fn new(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            package: PackageMetadata {
                name: name.into(),
                version: version.into(),
            },
            dependencies: BTreeMap::new(),
        }
    }

    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        toml::from_str(text).map_err(|err| ManifestError::Parse {
            path: MANIFEST_FILE_NAME.to_string(),
            message: err.message().to_string(),
        })
    }

    pub fn to_toml_string(&self) -> String {
        toml::to_string_pretty(self).expect("Manifest always serializes")
    }

    /// Reads `<dir>/aint.toml`.
    pub fn read_from_dir(dir: &Path) -> Result<Self, ManifestError> {
        let path = dir.join(MANIFEST_FILE_NAME);
        let text = fs::read_to_string(&path).map_err(|err| ManifestError::Io {
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
        Self::parse(&text).map_err(|err| match err {
            ManifestError::Parse { message, .. } => ManifestError::Parse {
                path: path.display().to_string(),
                message,
            },
            other => other,
        })
    }

    /// Writes `<dir>/aint.toml`, overwriting anything already there.
    pub fn write_to_dir(&self, dir: &Path) -> Result<(), ManifestError> {
        let path = dir.join(MANIFEST_FILE_NAME);
        fs::write(&path, self.to_toml_string()).map_err(|err| ManifestError::Io {
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
        let mut manifest = Manifest::new("my-project", "0.1.0");
        manifest.dependencies.insert(
            "some-lib".to_string(),
            Dependency::Path {
                path: "../some-lib".to_string(),
            },
        );
        let text = manifest.to_toml_string();
        let parsed = Manifest::parse(&text).expect("should parse what we just wrote");
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn a_git_dependency_round_trips_through_toml() {
        let mut manifest = Manifest::new("my-project", "0.1.0");
        manifest.dependencies.insert(
            "some-lib".to_string(),
            Dependency::Git {
                git: "https://github.com/user/some-lib".to_string(),
                rev: Some("v1.2.0".to_string()),
            },
        );
        let text = manifest.to_toml_string();
        let parsed = Manifest::parse(&text).expect("should parse what we just wrote");
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn a_git_dependency_with_no_rev_round_trips_too() {
        let mut manifest = Manifest::new("my-project", "0.1.0");
        manifest.dependencies.insert(
            "some-lib".to_string(),
            Dependency::Git {
                git: "https://github.com/user/some-lib".to_string(),
                rev: None,
            },
        );
        let text = manifest.to_toml_string();
        let parsed = Manifest::parse(&text).expect("should parse what we just wrote");
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn parses_a_manifest_with_no_dependencies() {
        let manifest = Manifest::parse("[package]\nname = \"my-project\"\nversion = \"0.1.0\"\n")
            .expect("should parse");
        assert_eq!(manifest.package.name, "my-project");
        assert!(manifest.dependencies.is_empty());
    }

    #[test]
    fn rejects_malformed_toml() {
        let err = Manifest::parse("not valid toml [[[").unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }));
    }

    #[test]
    fn rejects_a_manifest_missing_the_package_table() {
        let err = Manifest::parse("[dependencies]\n").unwrap_err();
        assert!(matches!(err, ManifestError::Parse { .. }));
    }
}
