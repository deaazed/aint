//! Builds the flattened, deduplicated dependency graph a `Lockfile`
//! records - a depth-first walk from the root package's manifest,
//! following each dependency (a local `path`, or, since milestone 36,
//! a `git` source materialized into a local cache first) to its own
//! `aint.toml` and recursing. Two things make this a real resolution
//! algorithm and not just "copy every path into a list": cycle
//! detection (A depends on B depends on A) and diamond-conflict
//! detection (two different packages both depending on something
//! named `x`, but at two different, unrelated paths). See
//! `docs/milestones/23-package-manager/SPEC.md` and
//! `docs/milestones/36-git-dependencies/SPEC.md`.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::git;
use crate::lockfile::{GitSource, LockedPackage, Lockfile};
use crate::manifest::{Dependency, Manifest, ManifestError};

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
    /// `git clone`/`fetch`/`checkout`/`rev-parse` failed - a missing
    /// `git` binary, a bad URL, a `rev` that doesn't exist, or a
    /// genuine network failure on a cache miss. `message` is `git`'s
    /// own stderr, unmodified. See
    /// `docs/milestones/36-git-dependencies/SPEC.md`.
    Git {
        url: String,
        message: String,
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
            ResolveError::Git { url, message } => write!(f, "git {url}: {message}"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// One name's resolved location - `None` for the root package itself.
struct Resolved {
    path: Option<PathBuf>,
    version: String,
    source: Option<GitSource>,
}

/// Resolves `root_dir`'s full dependency graph into a [`Lockfile`],
/// materializing any `git` dependency into `~/.aint/cache/git/` (or
/// the Windows equivalent) along the way.
pub fn resolve(root_dir: &Path) -> Result<Lockfile, ResolveError> {
    resolve_with_git_cache(root_dir, &default_git_cache_dir())
}

fn default_git_cache_dir() -> PathBuf {
    home_dir().join(".aint").join("cache").join("git")
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Same as [`resolve`], with an explicit git cache directory - split
/// out so tests can point it at a temporary directory instead of a
/// real machine's home directory.
fn resolve_with_git_cache(
    root_dir: &Path,
    git_cache_root: &Path,
) -> Result<Lockfile, ResolveError> {
    let root_dir = canonicalize(root_dir)?;
    let root_manifest = read_manifest(&root_dir)?;

    let mut resolved: BTreeMap<String, Resolved> = BTreeMap::new();
    resolved.insert(
        root_manifest.package.name.clone(),
        Resolved {
            path: None,
            version: root_manifest.package.version.clone(),
            source: None,
        },
    );

    let mut stack = vec![root_manifest.package.name.clone()];
    visit(
        &root_dir,
        &root_manifest,
        &mut resolved,
        &mut stack,
        git_cache_root,
    )?;

    let packages = resolved
        .into_iter()
        .map(|(name, entry)| LockedPackage {
            name,
            version: entry.version,
            path: entry.path.map(|p| p.display().to_string()),
            source: entry.source,
        })
        .collect();
    Ok(Lockfile { packages })
}

fn visit(
    pkg_dir: &Path,
    manifest: &Manifest,
    resolved: &mut BTreeMap<String, Resolved>,
    stack: &mut Vec<String>,
    git_cache_root: &Path,
) -> Result<(), ResolveError> {
    for (dep_name, dep) in &manifest.dependencies {
        let (dep_path, source) = materialize(dep, pkg_dir, git_cache_root)?;
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
                source,
            },
        );
        stack.push(dep_name.clone());
        visit(&dep_path, &dep_manifest, resolved, stack, git_cache_root)?;
        stack.pop();
    }
    Ok(())
}

/// Resolves one dependency declaration to a real, local directory -
/// `path` dependencies unchanged from milestone 23; a `git`
/// dependency is materialized via [`materialize_git_with_cache`], then
/// returned as if it were a path dependency all along.
fn materialize(
    dep: &Dependency,
    pkg_dir: &Path,
    git_cache_root: &Path,
) -> Result<(PathBuf, Option<GitSource>), ResolveError> {
    match dep {
        Dependency::Path { path } => Ok((canonicalize(&pkg_dir.join(path))?, None)),
        Dependency::Git { git: url, rev } => {
            let (dir, source) = materialize_git_with_cache(url, rev.as_deref(), git_cache_root)?;
            Ok((dir, Some(source)))
        }
    }
}

/// Materializes a git dependency into the default `~/.aint/cache/git`
/// cache (or the Windows equivalent) and returns its local path plus
/// resolved source info. Public so `aint add --git` (milestone 36) can
/// discover a dependency's declared package name — by reading the
/// manifest at the returned path — before inserting it into
/// `aint.toml`, the same way the existing path-dependency flow reads a
/// local directory's manifest to learn its name.
pub fn materialize_git(url: &str, rev: Option<&str>) -> Result<(PathBuf, GitSource), ResolveError> {
    materialize_git_with_cache(url, rev, &default_git_cache_dir())
}

/// Cloned (on a cache miss) or fetched-and-checked-out (on a cache
/// hit, only when `rev` is given - see
/// `docs/milestones/36-git-dependencies/SPEC.md` for why an unpinned,
/// already-cached git dependency isn't re-fetched on every resolve).
fn materialize_git_with_cache(
    url: &str,
    rev: Option<&str>,
    cache_root: &Path,
) -> Result<(PathBuf, GitSource), ResolveError> {
    let dir = git::cache_dir_for(cache_root, url);
    let err = |message: String| ResolveError::Git {
        url: url.to_string(),
        message,
    };

    if dir.exists() {
        if rev.is_some() {
            let _ = git::fetch(&dir);
        }
    } else {
        git::clone(url, &dir).map_err(err)?;
    }
    if let Some(rev) = rev {
        git::checkout(&dir, rev).map_err(err)?;
    }
    let commit = git::rev_parse_head(&dir).map_err(err)?;
    let dir = canonicalize(&dir)?;
    Ok((
        dir,
        GitSource {
            git: url.to_string(),
            commit,
        },
    ))
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
    use std::fs;
    use std::process::Command;

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
        Dependency::Path {
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

    /// Sets up a real, local, offline git repository at `dir` - a
    /// `git init` package directory, an `aint.toml` matching
    /// `package_name`, one commit, and (if `tag` is given) a tag on
    /// that commit. Real `git`, real commits, real tags - no network,
    /// no mock, the same "genuinely reads the filesystem" reasoning
    /// `TempWorkspace` itself already follows, extended to git.
    fn init_git_package(dir: &Path, package_name: &str, tag: Option<&str>) -> String {
        fs::create_dir_all(dir).expect("should create git package dir");
        Manifest::new(package_name, "0.1.0")
            .write_to_dir(dir)
            .expect("should write manifest");

        let git = |args: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(args)
                .env("GIT_AUTHOR_NAME", "test")
                .env("GIT_AUTHOR_EMAIL", "test@example.com")
                .env("GIT_COMMITTER_NAME", "test")
                .env("GIT_COMMITTER_EMAIL", "test@example.com")
                .status()
                .expect("failed to run git");
            assert!(status.success(), "git {args:?} failed");
        };

        git(&["init", "--quiet"]);
        git(&["add", "."]);
        git(&["commit", "--quiet", "-m", "initial"]);
        if let Some(tag) = tag {
            git(&["tag", tag]);
        }

        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["rev-parse", "HEAD"])
            .output()
            .expect("failed to run git rev-parse");
        String::from_utf8_lossy(&output.stdout).trim().to_string()
    }

    #[test]
    fn resolves_a_git_dependency_from_a_local_repository() {
        let ws = TempWorkspace::new("git_basic");
        let remote_dir = ws.path("remote");
        let commit = init_git_package(&remote_dir, "git-lib", None);

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert(
            "git-lib".to_string(),
            Dependency::Git {
                git: remote_dir.display().to_string(),
                rev: None,
            },
        );
        let root_dir = ws.write_package("root", &root);
        let cache_dir = ws.path("cache");

        let lockfile =
            resolve_with_git_cache(&root_dir, &cache_dir).expect("should resolve a git dep");
        let locked = lockfile
            .packages
            .iter()
            .find(|p| p.name == "git-lib")
            .expect("git-lib should be locked");
        assert!(locked.path.is_some());
        let source = locked.source.as_ref().expect("should record a GitSource");
        assert_eq!(source.git, remote_dir.display().to_string());
        assert_eq!(source.commit, commit);
    }

    #[test]
    fn resolves_a_git_dependency_pinned_to_a_tag() {
        let ws = TempWorkspace::new("git_tag");
        let remote_dir = ws.path("remote");
        let commit = init_git_package(&remote_dir, "git-lib", Some("v1.0.0"));

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert(
            "git-lib".to_string(),
            Dependency::Git {
                git: remote_dir.display().to_string(),
                rev: Some("v1.0.0".to_string()),
            },
        );
        let root_dir = ws.write_package("root", &root);
        let cache_dir = ws.path("cache");

        let lockfile =
            resolve_with_git_cache(&root_dir, &cache_dir).expect("should resolve a pinned git dep");
        let locked = lockfile
            .packages
            .iter()
            .find(|p| p.name == "git-lib")
            .expect("git-lib should be locked");
        assert_eq!(locked.source.as_ref().unwrap().commit, commit);
    }

    #[test]
    fn a_second_resolve_reuses_the_cached_clone() {
        let ws = TempWorkspace::new("git_cache_reuse");
        let remote_dir = ws.path("remote");
        init_git_package(&remote_dir, "git-lib", None);

        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert(
            "git-lib".to_string(),
            Dependency::Git {
                git: remote_dir.display().to_string(),
                rev: None,
            },
        );
        let root_dir = ws.write_package("root", &root);
        let cache_dir = ws.path("cache");

        let first = resolve_with_git_cache(&root_dir, &cache_dir).expect("first resolve");
        let second = resolve_with_git_cache(&root_dir, &cache_dir).expect("second resolve");
        assert_eq!(
            first
                .packages
                .iter()
                .find(|p| p.name == "git-lib")
                .unwrap()
                .path,
            second
                .packages
                .iter()
                .find(|p| p.name == "git-lib")
                .unwrap()
                .path,
        );
    }

    #[test]
    fn a_nonexistent_git_remote_is_a_clear_git_error() {
        let ws = TempWorkspace::new("git_missing_remote");
        let mut root = Manifest::new("root", "0.1.0");
        root.dependencies.insert(
            "nope".to_string(),
            Dependency::Git {
                git: ws.path("does-not-exist").display().to_string(),
                rev: None,
            },
        );
        let root_dir = ws.write_package("root", &root);
        let cache_dir = ws.path("cache");

        let err = resolve_with_git_cache(&root_dir, &cache_dir)
            .expect_err("should fail to clone a nonexistent remote");
        assert!(matches!(err, ResolveError::Git { .. }));
    }
}
