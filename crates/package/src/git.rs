//! Shells out to the real `git` binary rather than embedding a git
//! implementation — the same "read the real filesystem, don't mock
//! it" reasoning `resolve.rs`'s own tests already follow, extended to
//! "run the real `git`, don't reimplement it." See
//! `docs/milestones/36-git-dependencies/SPEC.md`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a git dependency's clone lives on disk, keyed by its exact
/// URL — every non-alphanumeric character becomes `_`, so two
/// different URLs never collide. An exact-match cache key, not a
/// content hash: simple, and the directory name stays legible for
/// anyone poking around the cache by hand.
pub fn cache_dir_for(cache_root: &Path, url: &str) -> PathBuf {
    let sanitized: String = url
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    cache_root.join(sanitized)
}

pub fn clone(url: &str, dest: &Path) -> Result<(), String> {
    run(Command::new("git")
        .args(["clone", "--quiet", url])
        .arg(dest))
}

/// Best-effort — a stale-but-present cache is still usable if this
/// fails (a transient network issue, most likely); `checkout` still
/// fails clearly afterward if the requested `rev` genuinely isn't
/// reachable either way.
pub fn fetch(repo_dir: &Path) -> Result<(), String> {
    run(Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["fetch", "--quiet", "--all", "--tags"]))
}

pub fn checkout(repo_dir: &Path, rev: &str) -> Result<(), String> {
    run(Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["checkout", "--quiet", rev]))
}

pub fn rev_parse_head(repo_dir: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_dir)
        .args(["rev-parse", "HEAD"])
        .output()
        .map_err(|err| format!("could not run git: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run(command: &mut Command) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|err| format!("could not run git: {err}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(())
}
