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

- **The actual download path (a real tag, a real release, a real
  `curl`/`Invoke-WebRequest` fetch) is unverified as of this
  `ACCEPTANCE.md`.** Nothing has been tagged yet — pushing `v0.1.0` (or
  similar) is a real, visible, external action (a public GitHub
  Release), held for explicit confirmation rather than done
  unilaterally while writing this milestone. What's verified here is
  everything that *doesn't* require one: script syntax, the
  unsupported-platform error path, and the workflow's own YAML
  structure. The first real tag push *is* this milestone's true
  end-to-end test — to be confirmed once pushed.
- **No `linux-aarch64` build** — the matrix only covers Linux x86_64;
  `install.sh` fails clearly (not a 404) if run on Linux/aarch64,
  naming the gap directly rather than guessing.
- **Unsigned binaries** on macOS/Windows — expect a first-run OS
  warning; code-signing is real, separate, unattempted work.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by `.github/workflows/release.yml`, `install.sh`,
`install.ps1`, and `README.md`'s updated install section — pending the
first real tag push to verify the download path end to end.
