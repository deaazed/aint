# Milestone 36 — Package dependencies over git — acceptance

## Scope

See `SPEC.md`. A git-sourced `Dependency` variant, resolution that
materializes a git dependency into a shared local cache before treating
it like any other resolved package, `aint add --git`, and — closing the
gap named in both milestone 23's and milestone 29's own `SPEC.md`s — a
bare-name `import "package-name" as alias` that actually resolves
through `aint.lock` into a package's `lib.an`.

## Acceptance criteria

- [x] `Dependency` (`crates/package/src/manifest.rs`) is an untagged
      `Path { path } | Git { git, rev: Option<String> }` enum; both
      forms round-trip through TOML (`round_trips_through_toml`,
      `a_git_dependency_round_trips_through_toml`,
      `a_git_dependency_with_no_rev_round_trips_too`).
- [x] `crates/package/src/git.rs` shells out to a real `git` binary
      (`clone`/`fetch`/`checkout`/`rev_parse_head`) — no embedded git
      implementation, no mocking of the filesystem or of git itself.
- [x] `resolve.rs` materializes a git dependency into
      `~/.aint/cache/git/<sanitized-url>/`
      (`%USERPROFILE%\.aint\cache\git\...` on Windows) before doing
      anything else with it — from that point on it's just a local path
      as far as cycle detection, diamond-conflict detection, and
      declared-name verification are concerned. 5 new tests in
      `resolve.rs`, all against real local git repositories created with
      `git init`/`git commit`/`git tag` in temp directories (no network):
      a fresh clone resolves and locks correctly, re-resolving reuses
      the existing clone rather than re-cloning, a `rev` pins to a tag,
      an unknown `rev` fails clearly, and the resolved commit is stable
      across repeated resolves.
- [x] `LockedPackage` records `source: Option<GitSource { git, commit }>`
      for a git-resolved package — the exact commit `git rev-parse HEAD`
      reported after checkout, reproducible even if `rev` was a moving
      branch name.
- [x] `aint add --git <url> [--rev <rev>]` (`crates/cli/src/main.rs`)
      resolves and re-locks the whole graph exactly like a path
      dependency, just sourced differently; `clap`'s
      `conflicts_with`/`requires` keep `path` and `git`/`rev` mutually
      exclusive at the argument-parsing level.
- [x] **The `aint-package` ↔ `aint-loader` disconnection is closed.**
      `crates/loader/src/lib.rs` gained `PackageContext`
      (`NoRoot`/`NoLockfile`/`Resolved(HashMap<String, PathBuf>)`),
      discovered once from the entry file by walking up for the nearest
      `aint.toml` and reading its `aint.lock`. A bare-name
      `import "name" as alias` (no `./`/`../` prefix) resolves through
      it to `<locked-path>/lib.an` — everything downstream (renaming,
      no-diamond-imports, the fn/enum/tool/infer-only top-level rule) is
      unchanged, since a package import is still just an import once
      resolved. 4 new tests: a successful bare-name import renaming and
      splicing in a dependency's `lib.an`
      (`a_bare_name_import_resolves_via_the_lockfile_to_lib_an`), and one
      each for the three specific, distinct failure modes
      (`an_unknown_package_name_is_a_clear_error`,
      `a_package_import_with_no_aint_toml_above_is_a_clear_error`,
      `a_package_import_with_no_lockfile_is_a_clear_error`) — a typo'd
      name, no package root at all, and a package root with no lockfile
      yet, each reported as what it actually is rather than one generic
      "not found."
- [x] **Verified end to end against a real local git remote, through the
      real CLI binary** (not just unit tests): `aint init` a library
      package, turn it into a real git repo with a real tag, `aint init`
      a separate consumer package, `aint add --git <local-repo-path>
      --rev v1.0.0`, then a bare-name `import "greeter-lib" as greeter`
      in the consumer's `main.an` — `aint check` and `aint run` both
      succeed and print the library's real output. Torn down after
      (temp directories and the local `.aint/cache/git` removed) rather
      than left behind.
- [x] `examples/package_import/` (`greeter-lib/` + `app/`) commits a
      second, always-reviewable demonstration of the same
      `aint-loader`/`aint-package` connection using a path dependency —
      verified the same way (`aint add ../greeter-lib` from `app/`, then
      `aint check`/`aint run`) before being committed. `aint.lock` is
      deliberately *not* committed: `LockedPackage.path` is always an
      absolute, canonicalized, machine-specific path (true for path
      dependencies since milestone 23, unchanged here), so committing
      one generated on this machine wouldn't be portable to a fresh
      clone. `app/main.an`'s own comment says exactly what to run
      (`aint add ../greeter-lib`) and why the lockfile isn't there.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **No name → URL index.** `aint add --git` always takes a real URL —
  there's still no `aint add some-lib` that looks a short name up
  anywhere. Named as explicitly out of scope in `SPEC.md`; the natural
  next increment on top of this milestone, not attempted here.
- **No version-range resolution for git dependencies.** `rev` is a
  literal git ref, resolved once — no semver matching.
- **A committed, ready-to-run git-dependency example doesn't exist**,
  and structurally can't: both `aint.lock`'s path-dependency and its
  git-dependency entries record an absolute, machine-specific path (the
  local checkout, or the shared git cache respectively), so nothing
  generated on this machine would resolve correctly from a fresh clone
  on someone else's. `examples/package_import/` demonstrates the
  mechanism with a path dependency instead, and the git path was
  verified live (see above) rather than committed as a runnable asset.
- **No private/authenticated git remotes are specifically handled** —
  whatever the caller's own `git` can already reach (SSH keys, a
  configured credential helper) works; nothing new was built for auth,
  as `SPEC.md` already called out.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `aint add --git` resolves and locks a real git dependency
using a real `git` binary, and — closing a gap that had sat open across
two prior milestones — a bare-name `import` now actually reaches that
dependency's code. Verified against a real local git remote through the
real CLI binary, in addition to 5 new `aint-package` tests and 4 new
`aint-loader` tests, all passing alongside the full existing suite.
