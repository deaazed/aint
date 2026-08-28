# Milestone 23 — Package manager — acceptance

## Scope

See `SPEC.md`. A new `aint-package` crate (manifest, lockfile, local
path dependency resolution) plus `aint init`/`aint add` CLI commands —
the bookkeeping layer `ROADMAP.md` actually asks for, minus the
registry (no such service exists) and minus actually wiring a resolved
dependency's source into a compiled program (AINT has no multi-file
compilation model at all yet — a separate, named prerequisite, not
something rushed in here).

## Acceptance criteria

- [x] New crate `aint-package` (`crates/package`), added to the
      workspace: `manifest.rs` (`Manifest`, `PackageMetadata`,
      `Dependency`, TOML round-trip), `lockfile.rs` (`Lockfile`,
      `LockedPackage`, TOML round-trip), `resolve.rs`
      (`resolve(&Path) -> Result<Lockfile, ResolveError>`).
- [x] `Manifest`/`Lockfile` round-trip through TOML byte-for-byte
      equivalent (parse what was just serialized), verified directly,
      including the empty-dependencies and empty-lockfile cases.
- [x] Malformed TOML and a manifest missing `[package]` are both
      rejected with a clear `ManifestError::Parse`, verified directly.
- [x] `resolve` correctly handles: no dependencies, a linear chain
      (three levels), a diamond where both edges point at the *same*
      directory (resolves once, not flagged as a conflict), a diamond
      where two edges point at *different* directories under the same
      name (rejected as `ResolveError::ConflictingPaths`), a direct
      two-package cycle (rejected as `ResolveError::Cycle` naming the
      full chain), a dependency whose own manifest declares a
      different name than it was added under (rejected as
      `ResolveError::NameMismatch`), and a missing dependency path
      (a clear `ResolveError::Io`, not a panic) — all seven verified
      directly against real temporary directory trees, not mocked
      file reads.
- [x] `aint init [path]` (CLI): creates `<path>/aint.toml` (name
      inferred from the directory) and a starter `<path>/main.an`;
      refuses to overwrite an existing manifest. Both verified through
      the real built binary.
- [x] `aint add <path>` (CLI): infers the dependency's name from its
      own manifest, records it in the current directory's
      `aint.toml`, fully re-resolves the graph, and rewrites
      `aint.lock`. Verified through the real binary: a successful add
      updates both files with the right content; adding a dependency
      that would introduce a cycle fails clearly (the CLI surfaces
      `resolve`'s own `Cycle` error, unmodified) with a non-zero exit;
      running `add` with no `aint.toml` in the current directory fails
      clearly, suggesting `aint init`.
- [x] `cargo test --workspace` passes with no regressions: 350 tests
      total, up from 332 before this milestone (18 new: 13 in
      `aint-package`, 5 CLI integration tests against the real
      binary).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are
      clean across the whole workspace, including the new crate.

## Known, honestly-stated gap

Nothing in `aint run`, `aint test`, or `aint run --vm` reads
`aint.lock` or does anything with a resolved dependency's source —
AINT has no way for one `.an` file to reference another at all (every
`import` still resolves only to the six fixed stdlib modules). See
`SPEC.md`'s "The prerequisite gap this milestone surfaces but doesn't
solve." This milestone is the manifest/lockfile/resolution bookkeeping
layer only, not a working import-a-dependency's-code story.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — a registry, consuming a
resolved dependency's source, root-relative lockfile paths, version
ranges/semver resolution, and anything resembling a build system.

## Outcome

Satisfied by `crates/package` (new crate: `manifest.rs`,
`lockfile.rs`, `resolve.rs`, `lib.rs`) and `crates/cli/src/main.rs`'s
new `init`/`add` functions and `Command::Init`/`Command::Add`
variants. 350 tests total across the workspace, all passing: 18 new,
covering manifest/lockfile round-tripping, all seven dependency-graph
resolution cases (including both cycle and diamond-conflict
detection), and the real `aint init`/`aint add` CLI path end to end,
including a cyclic-dependency rejection surfaced correctly through the
CLI.
