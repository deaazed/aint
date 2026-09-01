# Milestone 35 — Installer

## Scope

Getting a working `aint` binary has meant `git clone` + `cargo build`
since the project existed — fine for a contributor, a real wall for
anyone who isn't already a Rust developer and just wants to try the
language. This milestone builds the standard, low-risk path every
comparable tool (`rustup`, `bun`, `deno`) uses: prebuilt binaries
attached to tagged GitHub releases, fetched by a small install script.
No new hosting, no domain, nothing beyond what GitHub already provides.

## What this milestone actually builds

**`.github/workflows/release.yml`**, triggered on pushing a `v*` tag:
builds `aint` in release mode natively on `ubuntu-latest` (`x86_64
-unknown-linux-gnu`), `macos-latest` (both `aarch64-apple-darwin`,
native, and `x86_64-apple-darwin`, cross-compiled via an added
rustup target), and `windows-latest` (`x86_64-pc-windows-msvc`) — one
matrix job per platform, each uploading its own archive
(`aint-<os>-<arch>.tar.gz`, or `.zip` on Windows) as a build artifact.
A final job collects every artifact and publishes them as assets on a
real GitHub Release for that tag, with auto-generated release notes.

**`install.sh`**, at the repo root, the same `curl -fsSL <url> | sh`
shape every comparable installer uses: detects OS (`Linux`/`Darwin`)
and architecture (`x86_64`/`aarch64`) via `uname`, downloads the
matching asset from the *latest* GitHub release
(`.../releases/latest/download/<asset>`, so it never needs updating
for a new version), extracts the binary to `~/.aint/bin/aint`
(overridable via `AINT_INSTALL_DIR`), and tells you to add that
directory to `PATH` if it isn't already there. Fails clearly — naming
the unsupported platform, or the URL that 404'd — rather than a
confusing tar/curl error.

**`install.ps1`**, the Windows equivalent (`irm <url> | iex`): same
shape, downloads the `.zip` asset, extracts to
`%USERPROFILE%\.aint\bin\aint.exe`, checks `PATH`.

**`README.md`'s "Building" section now leads with the installer**,
`cargo build` demoted to "or, from source" — still fully documented
and supported, since contributors and unsupported platforms still need
it.

## Design decisions

**No new infrastructure.** Both scripts are served straight from the
repository (`raw.githubusercontent.com/deaazed/aint/main/install.sh`)
and binaries come from GitHub Releases — nothing beyond what pushing a
tag already gets for free. Matches the recommendation made and
accepted before starting this: a registry needing a real server is a
different, later decision; an installer doesn't need one at all.

**`--target`-based cross-compilation for `x86_64-apple-darwin`
specifically**, since `macos-latest` runners are Apple Silicon
natively — every other platform in the matrix builds for exactly the
architecture it's already running on.

**The install script always fetches `releases/latest`, never a pinned
version.** Simpler for v1, and consistent with "one canonical way to
get the current release" — versioned/pinned installs are a reasonable
future addition, not attempted here.

## Explicitly out of scope

- **A package-manager listing** (Homebrew formula, `apt`/`winget`
  package, a Docker image). The install script is the one channel this
  milestone builds.
- **Auto-update.** Re-running the install script is how you upgrade;
  no self-update mechanism in the binary itself.
- **Pinned/versioned installs** (`install.sh --version 1.2.0`). Always
  latest, for now.
- **Code-signing / notarization** for the macOS and Windows binaries.
  Real, separate work with its own account/cost requirements, not
  attempted here — expect an unsigned-binary warning on first run.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented — and once a real
tag has actually been pushed and produced a real release to verify
against, since that's the one part of this milestone that can't be
verified without it.
