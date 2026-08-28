//! Builds the flattened, deduplicated dependency graph a `Lockfile`
//! records - a depth-first walk from the root package's manifest,
//! following each `path` dependency to its own `aint.toml` and
//! recursing. Two things make this a real resolution algorithm and
//! not just "copy every path into a list": cycle detection (A depends
//! on B depends on A) and diamond-conflict detection (two different
//! packages both depending on something named `x`, but at two
//! different, unrelated paths). See
//! `docs/milestones/23-package-manager/SPEC.md`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::lockfile::{LockedPackage, Lockfile};
use crate::manifest::{Manifest, ManifestError};

#[derive(Debug, Clone, PartialEq)]
pub enum ResolveError {
    Io {
        path: String,
        message: String,
    },
    InvalidManifest(ManifestError),
    /// A dependency declared as `name = { path = ".." }` whose own
    /// `aint.toml` reports a *different* `package.name` - the path
    /// was followed, but what's there isn't what was asked for.
    NameMismatch {
        declared: String,
        found: String,
        path: String,
    },
    /// Two dependencies (or a dependency and the root package) claim
    /// the same name but resolve to two different, unrelated
    /// directories - not sound to flatten into one lockfile entry.
    ConflictingPaths {
        name: String,
        first: String,
        second: String,
    },
    /// A depends on B depends on ... depends on A. Reported with the
    /// full cycle, not just "a cycle exists somewhere."
    Cycle {
        cycle: Vec<String>,
    },
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ResolveError::Io { path, message } => write!(f, "could not read {path}: {message}"),
            ResolveError::InvalidManifest(err) => write!(f, "{err}"),
            ResolveError::NameMismatch {
                declared,
                found,
                path,
            } => write!(
                f,
                "dependency `{declared}` points at {path}, whose package is named `{found}`, not `{declared}`"
            ),
            ResolveError::ConflictingPaths { name, first, second } => write!(
                f,
                "two dependencies are both named `{name}` but resolve to different paths: {first} and {second}"
            ),
            ResolveError::Cycle { cycle } => {
                write!(f, "cyclic dependency: {}", cycle.join(" -> "))
            }
        }
    }
}

impl std::error::Error for ResolveError {}

/// One name's resolved location - `None` for the root package itself.
struct Resolved {
    path: Option<PathBuf>,
    version: String,
}

/// Resolves `root_dir`'s full dependency graph into a [`Lockfile`].
pub fn resolve(root_dir: &Path) -> Result<Lockfile, ResolveError> {
    let root_dir = canonicalize(root_dir)?;
    let root_manifest = read_manifest(&root_dir)?;

    let mut resolved: BTreeMap<String, Resolved> = BTreeMap::new();
    resolved.insert(
        root_manifest.package.name.clone(),
        Resolved {
            path: None,
            version: root_manifest.package.version.clone(),
        },
    );

    let mut stack = vec![root_manifest.package.name.clone()];
    visit(&root_dir, &root_manifest, &mut resolved, &mut stack)?;

    let packages = resolved
        .into_iter()
        .map(|(name, entry)| LockedPackage {
            name,
            version: entry.version,
            path: entry.path.map(|p| p.display().to_string()),
        })
        .collect();
    Ok(Lockfile { packages })
}

fn visit(
    pkg_dir: &Path,
    manifest: &Manifest,
    resolved: &mut BTreeMap<String, Resolved>,
    stack: &mut Vec<String>,
) -> Result<(), ResolveError> {
    for (dep_name, dep) in &manifest.dependencies {
        let dep_path = canonicalize(&pkg_dir.join(&dep.path))?;
        let dep_manifest = read_manifest(&dep_path)?;

        if &dep_manifest.package.name != dep_name {
            return Err(ResolveError::NameMismatch {
                declared: dep_name.clone(),
                found: dep_manifest.package.name.clone(),
                path: dep_path.display().to_string(),
            });
        }

        if stack.contains(dep_name) {
            let start = stack
                .iter()
                .position(|n| n == dep_name)
                .expect("just checked");
            let mut cycle = stack[start..].to_vec();
            cycle.push(dep_name.clone());
            return Err(ResolveError::Cycle { cycle });
        }

        if let Some(existing) = resolved.get(dep_name) {
            let same_path = existing.path.as_deref() == Some(dep_path.as_path());
            if !same_path {
                return Err(ResolveError::ConflictingPaths {
                    name: dep_name.clone(),
                    first: existing
                        .path
                        .as_ref()
                        .map(|p| p.display().to_string())
                        .unwrap_or_else(|| "(the root package)".to_string()),
                    second: dep_path.display().to_string(),
                });
            }
            // Already resolved to this exact path - a diamond, not a
            // conflict. Don't re-walk it (also avoids infinite work
            // if two independent branches share a large subgraph).
            continue;
        }

        resolved.insert(
            dep_name.clone(),
            Resolved {
                path: Some(dep_path.clone()),
                version: dep_manifest.package.version.clone(),
            },
        );
        stack.push(dep_name.clone());
        visit(&dep_path, &dep_manifest, resolved, stack)?;
        stack.pop();
    }
    Ok(())
}

fn read_manifest(dir: &Path) -> Result<Manifest, ResolveError> {
    Manifest::read_from_dir(dir).map_err(ResolveError::InvalidManifest)
}

fn canonicalize(path: &Path) -> Result<PathBuf, ResolveError> {
    path.canonicalize().map_err(|err| ResolveError::Io {
        path: path.display().to_string(),
        message: err.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Dependency;
    use std::fs;

    /// A scratch directory tree for one test, cleaned up on drop -
    /// `resolve` genuinely reads from disk (it has to: dependency
    /// resolution means following real paths to real manifests), so
    /// these tests build small real package trees under
    /// `std::env::temp_dir()` rather than mocking the filesystem.
    struct TempWorkspace {
        root: PathBuf,
    }

    impl TempWorkspace {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("aint_package_test_{name}_{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("should create the workspace root");
            Self { root }
        }

        fn write_package(&self, rel_dir: &str, manifest: &Manifest) -> PathBuf {
            let dir = self.root.join(rel_dir);
            fs::create_dir_all(&dir).expect("should create package dir");
            manifest.write_to_dir(&dir).expect("should write manifest");
            dir
        }

        fn path(&self, rel_dir: &str) -> PathBuf {
            self.root.join(rel_dir)
        }
    }

    impl Drop for TempWorkspace {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn dep(path: &str) -> Dependency {
        Dependency {
            path: path.to_string(),
        }
    }

    #[test]
    fn resolves_a_package_with_no_dependencies() {
        let ws = TempWorkspace::new("no_deps");
        let root_manifest = Manifest::new("root", "0.1.0");
        let root_dir = ws.write_package("root", &root_manifest);

        let lockfile = resolve(&root_dir).expect("should resolve");
        assert_eq!(lockfile.packages.len(), 1);
        assert_eq!(lockfile.packages[0].name, "root");
        assert_eq!(lockfile.packages[0].path, None);
    }

    #[test]
    fn resolves_a_simple_chain() {
        let ws = TempWorkspace::new("chain");
        ws.write_package("leaf", &Manifest::new("leaf", "0.1.0"));

        let mut mid = Manifest::new("mid", "0.1.0");
        mid.dependencies.insert("leaf".to_string(), dep("../leaf"));
        ws.write_package("mid", &mid);

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert("mid".to_string(), dep("../mid"));
        let root_dir = ws.write_package("root", &root);

        let lockfile = resolve(&root_dir).expect("should resolve");
        let names: Vec<&str> = lockfile.packages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["leaf", "mid", "root"]);
    }

    #[test]
    fn a_diamond_with_matching_paths_resolves_once() {
        let ws = TempWorkspace::new("diamond_ok");
        ws.write_package("shared", &Manifest::new("shared", "0.1.0"));

        let mut a = Manifest::new("a", "0.1.0");
        a.dependencies
            .insert("shared".to_string(), dep("../shared"));
        ws.write_package("a", &a);

        let mut b = Manifest::new("b", "0.1.0");
        b.dependencies
            .insert("shared".to_string(), dep("../shared"));
        ws.write_package("b", &b);

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert("a".to_string(), dep("../a"));
        root.dependencies.insert("b".to_string(), dep("../b"));
        let root_dir = ws.write_package("root", &root);

        let lockfile = resolve(&root_dir).expect("should resolve");
        let shared_count = lockfile
            .packages
            .iter()
            .filter(|p| p.name == "shared")
            .count();
        assert_eq!(shared_count, 1);
        assert_eq!(lockfile.packages.len(), 4);
    }

    #[test]
    fn a_diamond_with_conflicting_paths_is_rejected() {
        let ws = TempWorkspace::new("diamond_conflict");
        ws.write_package("shared-v1", &Manifest::new("shared", "0.1.0"));
        ws.write_package("shared-v2", &Manifest::new("shared", "0.2.0"));

        let mut a = Manifest::new("a", "0.1.0");
        a.dependencies
            .insert("shared".to_string(), dep("../shared-v1"));
        ws.write_package("a", &a);

        let mut b = Manifest::new("b", "0.1.0");
        b.dependencies
            .insert("shared".to_string(), dep("../shared-v2"));
        ws.write_package("b", &b);

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert("a".to_string(), dep("../a"));
        root.dependencies.insert("b".to_string(), dep("../b"));
        let root_dir = ws.write_package("root", &root);

        let err = resolve(&root_dir).expect_err("should reject a diamond conflict");
        assert!(matches!(err, ResolveError::ConflictingPaths { .. }));
    }

    #[test]
    fn a_direct_cycle_is_rejected() {
        let ws = TempWorkspace::new("cycle_direct");
        let a_dir = ws.path("a");
        let b_dir = ws.path("b");
        let _ = std::fs::create_dir_all(&a_dir);
        let _ = std::fs::create_dir_all(&b_dir);

        let mut a = Manifest::new("a", "0.1.0");
        a.dependencies.insert("b".to_string(), dep("../b"));
        a.write_to_dir(&a_dir).expect("write a");

        let mut b = Manifest::new("b", "0.1.0");
        b.dependencies.insert("a".to_string(), dep("../a"));
        b.write_to_dir(&b_dir).expect("write b");

        let err = resolve(&a_dir).expect_err("should reject a cycle");
        assert!(matches!(err, ResolveError::Cycle { .. }));
    }

    #[test]
    fn a_dependency_whose_manifest_declares_a_different_name_is_rejected() {
        let ws = TempWorkspace::new("name_mismatch");
        ws.write_package("actual", &Manifest::new("actual-name", "0.1.0"));

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies
            .insert("expected-name".to_string(), dep("../actual"));
        let root_dir = ws.write_package("root", &root);

        let err = resolve(&root_dir).expect_err("should reject a name mismatch");
        assert!(matches!(err, ResolveError::NameMismatch { .. }));
    }

    #[test]
    fn a_missing_dependency_path_is_a_clear_io_error() {
        let ws = TempWorkspace::new("missing_dep");
        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies
            .insert("nonexistent".to_string(), dep("../nonexistent"));
        let root_dir = ws.write_package("root", &root);

        let err = resolve(&root_dir).expect_err("should fail to resolve");
        assert!(matches!(err, ResolveError::Io { .. }));
    }
}
