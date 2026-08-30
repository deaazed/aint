# Milestone 33 — Rebuild the language's own website — acceptance

## Scope

See `SPEC.md`. `examples/website/site.an` (one 635-line file, a
7-level-deep nested `if`/`else` router) rebuilt as nine files
(`layout.an` + seven page files + `main.an`), routed through
`examples/router/router.an`'s flat table — minus diamond-import
support and any new content beyond what already existed, both named
directly as out of scope.

## Acceptance criteria

- [x] `examples/website/site.an` removed; replaced by `layout.an`,
      `home.an`, `docs_index.an`, `install.an`, `quickstart.an`,
      `guide.an`, `stdlib_page.an`, `reference.an`, `main.an`.
- [x] `main.an` imports all eight other files plus
      `../router/router.an`, assembles every page, and registers all
      seven routes (`/`, `/docs`, `/docs/install`, `/docs/quickstart`,
      `/docs/guide`, `/docs/stdlib`, `/docs/reference`) in one flat
      `router_route(...)` call — zero nested `if`/`else` in
      `handle_request`.
- [x] `install.an`/`quickstart.an`/`guide.an` take `shell_block` as a
      `fn(String) -> String` closure parameter rather than importing
      `layout.an` a second time — verified by `aint check` passing
      (a second import of the same file would be a clear
      `LoadError::DuplicateImport` from milestone 29, confirming this
      is a real constraint being satisfied, not an unused one).
- [x] `guide.an` and `reference.an`'s content updated to describe
      closures, modularity, and the router accurately — the previous
      "no cross-file import" known-gap line replaced with what's
      actually still true (no diamond imports, closures don't reach
      `--vm`, no generics/structs/traits).
- [x] `aint check examples/website/main.an` passes cleanly.
- [x] Verified live, exactly as the original site was: `aint run`,
      then every route curled — all seven real routes plus an
      unmatched path (falls through to the router's `not_found`
      handler) — all returning `200` with `text/html; charset=utf-8`
      and well-formed HTML (tag-balance checked on `/` and
      `/docs/guide`: `div`/`section`/`header`/`footer`/`nav`/`table`/
      `html`/`body` all balanced).
- [x] `aint fmt --check` run against every new file; the five without
      `//` comments (`docs_index.an`, `quickstart.an`, `guide.an`,
      `stdlib_page.an`, `reference.an`) reformatted to canonical style
      with `aint fmt`, reverified live afterward. The four with `//`
      comments (`layout.an`, `home.an`, `install.an`, `main.an`)
      correctly refused by `aint fmt`, matching milestone 24's
      documented comment-preservation gap — not a regression.
- [x] `cargo test --workspace` passes with no regressions: 427 tests
      total, unchanged from milestone 32 — this milestone adds no new
      Rust-side tests, only AINT example content, verified through the
      real binary the same way `examples/router/`'s own demo was.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace (no Rust changes this milestone, but
      re-verified anyway).

## Known, honestly-stated gaps

- **`layout.an` can only ever be imported by one file** — a direct
  consequence of milestone 29's no-diamond-imports restriction, worked
  around here via closures and small duplication, not lifted.
- **Cross-file error positions remain approximate** — an unchanged,
  inherited limitation from milestone 29, more visible now that a real
  program spans nine files instead of one.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by `examples/website/`'s nine files, replacing the
single-file `site.an` — verified end to end through the real binary:
`aint check`, `aint run`, a live `curl` pass against all seven routes
plus a 404, tag-balance checked, and `aint fmt` applied where possible.
427 tests total across the workspace, all passing, unchanged from
before this milestone since no Rust code changed.
