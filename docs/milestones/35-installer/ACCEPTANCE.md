# Milestone 35 — Installer — acceptance

## Scope

See `SPEC.md`. A release workflow producing real GitHub Release
binaries on a tagged push, plus `install.sh`/`install.ps1` fetching
them — no new hosting, everything served by GitHub itself.

## Acceptance criteria

- [x] `.github/workflows/release.yml`: triggered on `v*` tags, builds
      `aint` in release mode across a 4-way matrix (Linux x86_64, macOS
      aarch64, macOS x86_64, Windows x86_64), packages each as
      `aint-<os>-<arch>.tar.gz` (`.zip` on Windows), and publishes them
      as assets on a real GitHub Release with auto-generated notes.
- [x] `install.sh`: detects OS/arch via `uname`, downloads the matching
      asset from `releases/latest` (never a pinned version), installs
      to `~/.aint/bin/aint` (overridable via `AINT_INSTALL_DIR`), warns
      if that directory isn't already on `PATH`. Verified directly: a
      syntax check (`bash -n`) passes, and a real run against this
      unsupported-for-now platform (Windows, under Git Bash) fails with
      the intended clear message naming the platform and pointing at
      building from source — not a confusing `curl`/`tar` error.
- [x] `install.ps1`: the Windows equivalent — downloads the `.zip`
      asset, installs to `%USERPROFILE%\.aint\bin\aint.exe`, checks the
      user `PATH`. Verified directly: parses with no syntax errors
      (`[System.Management.Automation.Language.Parser]::ParseFile`).
- [x] `README.md`'s install section leads with both scripts;
      `cargo build` is demoted to "Building from source," still fully
      documented for contributors and uncovered platforms.
- [x] `cargo build`/`test`/`clippy`/`fmt --check` all clean — this
      milestone adds no Rust code, so this is confirming no regression,
      not new coverage.

## Known, honestly-stated gaps

- **The actual download path is now verified.** `v0.1.0` was tagged and
  pushed; the first real run built all four targets but failed at the
  last step — the default `GITHUB_TOKEN` had no `contents: write`
  permission, so `softprops/action-gh-release` couldn't create the
  release (fixed in `b6cba5a`, then the tag was moved onto that commit
  and re-pushed). The second run published a real GitHub Release with
  all four assets. `install.ps1` was then run for real against it
  (`AINT_INSTALL_DIR` pointed at a scratch directory) and the resulting
  `aint.exe` printed `aint 0.1.0` and ran correctly. Getting this far
  also surfaced that the repository itself was still private —
  unauthenticated downloads 404 regardless of what the release contains
  — confirmed directly with `curl`/`Invoke-WebRequest` before and after
  flipping visibility. The repository is now public.
- **No `linux-aarch64` build** — the matrix only covers Linux x86_64;
  `install.sh` fails clearly (not a 404) if run on Linux/aarch64,
  naming the gap directly rather than guessing.
- **Unsigned binaries** on macOS/Windows — expect a first-run OS
  warning; code-signing is real, separate, unattempted work.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by `.github/workflows/release.yml`, `install.sh`,
`install.ps1`, and `README.md`'s updated install section — verified end
to end against a real, public `v0.1.0` release.
