# Milestone 23 — Package manager

## Scope

`ROADMAP.md`:

> `aint init`, `aint add`, `axiom.toml`-equivalent manifest, lockfile,
> dependency resolution, registry. Comes after the language itself
> works, not before.

Six names. One of them (registry) names an actual hosted service that
doesn't exist and isn't buildable inside this repository. The rest —
manifest, lockfile, `init`, `add`, dependency resolution — are real,
buildable, and are what this milestone builds, in a new crate,
`aint-package`.

## The prerequisite gap this milestone surfaces but doesn't solve

Before writing any of this, it's worth being direct about something
`ROADMAP.md`'s one-line description doesn't surface: **AINT has no
multi-file program model at all.** Every example, every test, every
`.an` file compiled so far is exactly one file. `import <name>`
(milestone 06) resolves to one of six fixed stdlib module names
(`math`, `string`, `time`, `collections`, `distribution`, `option`) —
never a user-authored path, never another package's source.

A package manager's job is to make *other packages' code* available to
*your* code. Without any way for one `.an` file to reference another
at all, "dependency" can only mean "a directory this tool knows about
and records in a lockfile" — not "code you can call." Building real
cross-file compilation (parser support for qualified names or a new
import form, typechecker changes to resolve types and function
signatures across module boundaries, namespacing/collision rules,
interpreter and VM changes to load and link another package's AST) is
its own substantial effort — arguably larger than the bookkeeping
layer below — and isn't attempted here. It's named directly so it's
findable, not discovered by someone trying to `import` a path
dependency and finding nothing recognizes the syntax.

## What this milestone actually builds

**`aint-package`**, a new crate: `Manifest` (`aint.toml`), `Lockfile`
(`aint.lock`), and `resolve` (the dependency-graph algorithm). Wired
into the CLI as `aint init` and `aint add`.

**Manifest (`aint.toml`)**:

```toml
[package]
name = "my-project"
version = "0.1.0"

[dependencies]
some-lib = { path = "../some-lib" }
```

Named `aint.toml`, not literally `axiom.toml` — `ROADMAP.md`'s
"axiom.toml-equivalent" reads as "a Cargo.toml-shaped manifest," the
same way Rust's own manifest isn't named after whatever tool inspired
its shape.

**Dependencies are `path` only.** There's no registry (see above), so
there's nothing to satisfy a bare name or a version range against.
Rather than model a `RegistryDependency` variant nothing could ever
resolve, `Dependency` only has a `path` field — an honest reflection
of what's actually resolvable today.

**Dependency resolution is a real graph algorithm, not just path
copying.** Starting from the root package's manifest, `resolve`
depth-first-walks every `path` dependency's own `aint.toml`,
recursing into *its* dependencies, and:

- **Detects cycles** (A depends on B depends on A), reporting the full
  cycle chain, not just "a cycle exists somewhere."
- **Detects diamond conflicts**: two different dependency edges naming
  the same package name but pointing at two different, unrelated
  directories are rejected — not sound to flatten into one lockfile
  entry. The identical case where both edges point at the exact same
  directory (a legitimate diamond — two packages sharing one common
  dependency) resolves once, correctly, not as a false-positive
  conflict.
- **Verifies each dependency's own manifest actually declares the name
  it was added under** — a `path` pointing somewhere whose
  `[package].name` doesn't match what the depending manifest called it
  is rejected, not silently aliased.

**The lockfile records absolute, canonicalized paths, not
root-relative ones.** Simpler and unambiguous — no path-diffing logic
needed, no chance of an incorrectly-computed relative path — at the
real cost of being non-portable if `aint.lock` were ever committed and
shared across checkouts at different absolute locations on different
machines. Stated as a real, known limitation, not silently accepted:
see "Explicitly out of scope."

**`aint init [path]`**: creates `<path>/aint.toml` (package name
inferred from the directory name) and a starter `<path>/main.an`.
Refuses to run if a manifest already exists there — this creates a new
package, it doesn't reset one.

**`aint add <path>`**: reads the dependency's own manifest at `<path>`
to get its declared name (matching `cargo add --path`'s own behavior
— the user doesn't separately assert a name that has to agree with
what's actually there), adds it to the current directory's
`aint.toml`, then **re-resolves the whole graph and rewrites
`aint.lock` from scratch** — not just appending one new entry, since
one new edge can change what the full, flattened graph looks like (a
newly shared transitive dependency, a newly introduced cycle).

## Design decisions

**A new crate, not a CLI-only feature.** `aint-package`'s logic
(manifest parsing, lockfile format, graph resolution) is real,
independently testable logic with no CLI argument-parsing concerns
mixed in — the same reasoning `aint-ir` and `aint-vm` got their own
crates rather than living inside `aint-runtime` or `crates/cli`
directly.

**`toml` (via `serde`) for the manifest/lockfile format**, not a
hand-rolled parser. `serde` was already a dependency (`aint-runtime`,
milestone 16's `HttpModel`); `toml` integrates with it directly, and a
manifest/lockfile format is exactly the kind of structured,
well-specified format a hand-rolled parser would only reinvent worse.

**Resolution genuinely reads the filesystem — there's no in-memory
mock of "a package."** A dependency is a real directory with a real
`aint.toml` in it; `resolve`'s own tests build small real directory
trees under `std::env::temp_dir()` rather than mocking file reads,
since the whole point is proving the *real* algorithm (path joining,
canonicalization, cycle/conflict detection against real, resolvable
paths) works — not a model of it.

## Explicitly out of scope

- **A registry.** No hosted service exists; standing one up is
  infrastructure work far outside a single milestone. `aint add`
  only ever takes a local path.
- **Actually consuming a resolved dependency's AINT source from
  another `.an` file.** See "The prerequisite gap," above — this is
  the honest, load-bearing limitation of this milestone. `aint.lock`
  fully describes a dependency graph; nothing in `aint run`,
  `aint test`, or `aint run --vm` reads it or does anything with it
  yet.
- **Root-relative lockfile paths.** Stated above — absolute,
  canonicalized paths only, for now.
- **Version ranges, semver resolution, or picking among multiple
  candidate versions of the same dependency.** Meaningless without
  a registry to offer more than one version of anything.
- **Anything resembling a build system** (source file lists beyond
  a single `main.an`, build scripts, feature flags). `aint init`
  scaffolds the smallest thing that's a valid package: a manifest and
  one entry-point file.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
