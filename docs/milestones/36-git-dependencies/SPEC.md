# Milestone 36 — Package dependencies over git

## Scope

`aint add` has only ever taken a local path since milestone 23 — real
enough to prove the manifest/lockfile/resolution machinery works, not
enough to actually use someone else's AINT code without vendoring it
into your own checkout by hand. Confirmed before starting: a hosted
registry (crates.io/npm-style) is a real infrastructure commitment —
a server, a database, a domain, ongoing responsibility for uptime and
abuse — that this project isn't taking on yet. This milestone builds
the lighter alternative that was chosen instead: dependencies resolved
straight from a git URL, the same model Go modules use. No server, no
database, no domain — `git` itself is the transport.

This milestone also closes a second gap named directly in both
milestone 23's and milestone 29's own `SPEC.md`s: `aint-package` (the
manifest/lockfile layer) and `aint-loader` (cross-file `import`) have
been two disconnected systems the entire time — a resolved dependency
was never actually *importable* from inside a program. A registry
that can't be `import`-ed isn't a real capability, so this milestone
connects them.

## What this milestone actually builds

**A dependency can now name a git source:**

```toml
[dependencies]
some-lib = { path = "../some-lib" }
other-lib = { git = "https://github.com/user/other-lib" }
pinned-lib = { git = "https://github.com/user/pinned-lib", rev = "v1.2.0" }
```

`Dependency` becomes an untagged enum (`Path { path }` /
`Git { git, rev: Option<String> }`) rather than a single struct —
TOML's own shape (`{ path = ".." }` vs `{ git = "..", rev = ".." }`)
already disambiguates which one a table is, so no explicit tag is
needed.

**Resolution materializes a git dependency into a local cache before
doing anything else** — `~/.aint/cache/git/<sanitized-url>/`
(`%USERPROFILE%\.aint\cache\git\...` on Windows), cloned on first use,
fetched and checked out to `rev` (or left on the default branch HEAD)
on every resolve after that. Once materialized, a git dependency is
just a local path as far as the rest of `resolve.rs` is concerned —
the entire existing cycle-detection, diamond-conflict-detection, and
declared-name-verification logic runs completely unchanged. Shells out
to the real `git` binary (`std::process::Command`) rather than
embedding a git implementation — the same "read the real filesystem,
don't mock it" reasoning `resolve.rs`'s own tests already use, extended
to "run the real `git`, don't reimplement it."

**The lockfile records what was actually resolved**, not just where
it ended up: a `LockedPackage` from a git dependency carries the
source URL and the exact commit `git rev-parse HEAD` reported after
checkout — reproducible even if `rev` was a moving branch name, the
same reasoning `Cargo.lock` records an exact git commit rather than
trusting a ref to stay put.

**`aint add`** gains a `--git <url>` form (`--rev <rev>` optional)
alongside the existing bare-path form — resolves and locks exactly
like a path dependency, just sourced differently.

**`import "package-name" as alias` — no `./` or `../` prefix — now
resolves against the current package's `aint.lock`, not a relative
file path.** `aint-loader` walks up from the entry file's directory to
find the nearest `aint.toml`, reads its already-resolved `aint.lock`
(never re-resolves — that's `aint add`'s job, not a compile-time side
effect), finds the named package's path, and imports `<path>/lib.an`
— a package's *library* entry point, distinct from `main.an`'s
*program* entry point, the same split Rust's own `lib.rs`/`main.rs`
convention draws. Everything downstream (renaming, the no-diamond-
imports restriction, the fn/enum/tool/infer-only top-level rule) is
unchanged — a package import is still just an import, once resolved.

## Design decisions

**Requires `git` on `PATH`.** Not vendored, not reimplemented — an
honest dependency on a tool virtually everyone doing this already has
installed, the same assumption `cargo`'s own git-dependency support
makes.

**No name index / lookup-by-bare-name for `aint add` in this
milestone.** `aint add --git <url>` always takes a real URL — there's
no `aint add some-lib` yet that looks a short name up somewhere. That
layer (a small, static name → URL index) is real, separate, and
additive on top of what this milestone builds, not attempted here; see
`ROADMAP.md`.

**Tests use local, offline git repositories** (`git init --bare` plus
a real `git clone` against a `file://`/local path — no network
involved), exactly the same reasoning `aint-package`'s existing
path-dependency tests already use real temporary directory trees
instead of mocking the filesystem, extended to "a real local git
remote, not a mock of one."

**The cache is shared across every project on the machine**, keyed by
URL — cloning the same dependency for every project that uses it would
be real, avoidable cost, the same reasoning Cargo's own
`~/.cargo/git/` cache exists.

## Explicitly out of scope

- **A name → URL index** (so `aint add some-lib` works without typing
  a URL). Real, separate, additive work — see above.
- **Version ranges / semver resolution** for git dependencies — `rev`
  is a literal git ref (tag, branch, or commit), resolved once, not
  matched against a range.
- **Private/authenticated git remotes** — whatever `git` itself can
  reach with the caller's existing credentials (SSH keys, a configured
  credential helper) works; nothing new is built for auth.
- **A hosted registry service.** Named directly and ruled out before
  starting — see "Scope," above.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
