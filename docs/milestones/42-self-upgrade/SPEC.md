# Milestone 42 — `aint upgrade`

## Scope

Getting `aint` has meant re-running `install.sh`/`install.ps1` by hand
since milestone 35 — there's no way to upgrade an existing install
except fetching the whole script again. This milestone adds
`aint upgrade`: the same download, but able to replace the binary
that's currently running instead of being invoked fresh each time.

**Deliberately not automatic, and not something that upgrades every
installed machine.** There's no telemetry or phone-home in `aint`, and
none is added here — a compiler silently rewriting itself without
being asked is a trust violation, not a feature. `rustup`, `deno`, and
`bun` all work the same way this does: pull-based, one explicit
command, run by whoever owns the machine. "Automatically upgrade every
installed copy" isn't a thing any of those tools do either, and isn't
attempted here.

## What this milestone actually builds

**`aint upgrade`**: checks the latest GitHub Release tag against
`env!("CARGO_PKG_VERSION")`; if it's actually newer (a real
`major.minor.patch` comparison, not just inequality — see "Design
decisions"), downloads the matching platform asset — the exact same
`aint-<os>-<arch>.tar.gz`/`.zip` `install.sh`/`install.ps1` already
fetch — and replaces the running binary in place.

**`aint upgrade --check`**: reports whether a newer version exists
without installing it, exiting non-zero if one is — the same
CI-friendly convention `aint fmt --check` already uses.

**Platform coverage matches the install scripts exactly**: Windows/
macOS/Linux x86_64, macOS aarch64. Linux/aarch64 (and anything else)
fails clearly, pointing at building from source, the same message
`install.sh` already gives for the platform it can't cover either.

## Design decisions

**A real version comparison, not equality.** `latest != current` would
wrongly call it an "upgrade" the moment `current` is *ahead* of the
last published release — reachable in practice (a dev build; the
window between a tag being pushed and its release workflow actually
finishing, during which `releases/latest` still reports the *previous*
tag) and confirmed while testing this milestone: checking a freshly
built `0.2.0` against the API mid-release still showed `v0.1.1` as
latest, and an equality check would have offered to "upgrade" v0.2.0
down to v0.1.1. `parse_semver` does a real three-integer comparison;
only `lat > cur` counts as an upgrade.

**Extraction shells out, rather than adding a dependency.** `tar` on
Unix, PowerShell's `Expand-Archive` on Windows — the exact tools
`install.sh`/`install.ps1` already assume are present. Adding a Rust
tar/zip crate for one command wasn't worth the dependency weight.

**Self-replacement via rename, not overwrite — required on Windows,
harmless on Unix.** Neither OS allows overwriting a running process's
own backing file directly. Both allow *renaming* it: the running
process keeps its open handle to the underlying file regardless of
what it's currently called.
- Unix: `fs::rename(new_binary, current_exe)` directly — the OS keeps
  the old inode alive for this still-running process; it just stops
  being reachable by that path.
- Windows: the running binary is renamed aside to `aint.exe.old`
  first, then the new binary is moved into the name it vacated. The
  `.old` file is deleted best-effort afterward — it may still be
  locked by *this very process* until it exits (confirmed directly:
  a real end-to-end upgrade left a genuine, expected `aint.exe.old`
  behind, cleaned up automatically the next time `aint upgrade` runs,
  since it removes any leftover `.old` file before renaming again).

**`reqwest`'s blocking client, not a new `tokio::runtime::Runtime`.**
`run`/`test` each build their own runtime because the interpreter
itself is async throughout; `upgrade` has nothing else concurrent
happening, and reqwest's blocking client already spins up its own
internal runtime, so it needs nothing from `aint` beyond the
dependency itself — which adds nothing new to the workspace's
dependency graph, since `aint-runtime`'s `HttpModel` already depends
on the same `reqwest` version for its async use.

## Explicitly out of scope

- **Any form of automatic, scheduled, or background upgrading.** See
  "Scope" above — a deliberate choice, not a gap.
- **Upgrading a project's own pinned/required toolchain version.**
  `aint.toml` has no `rust-version`-equivalent field, and nothing here
  adds one — there's been no breaking change yet to make that
  meaningful.
- **A migration/codemod tool for breaking language changes.** Nothing
  shipped through v0.2.0 has ever broken an existing program — every
  Phase 3 milestone verified this explicitly. Speculative migration
  tooling for a breaking change that hasn't happened yet isn't
  attempted here; see `ROADMAP.md`.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
