# Milestone 31 — Web framework ergonomics — acceptance

## Scope

See `SPEC.md`. One new stdlib primitive (`string_split`) plus a real,
importable route-table library written entirely in AINT
(`examples/router/router.an`), fixing the concrete, measured pain of
`handle_request`'s nested-`if`/`else` pyramid — minus path parameters/
wildcards and any change to `http_serve` itself, both named directly
as out of scope.

## Acceptance criteria

- [x] `string_split(s: String, sep: String) -> List<String>` added to
      the `string` module: `NativeFunction::StringSplit`
      (`crates/runtime/src/value.rs`), its implementation
      (`crates/runtime/src/stdlib.rs`, splitting on every occurrence,
      a one-element list of the whole string when the separator is
      absent or empty), its `module_bindings`/`module_functions`
      entries in both `aint-runtime` and `aint-typechecker`.
- [x] Verified directly: splitting on a present separator, splitting
      when the separator doesn't occur (one-element list), and typing
      (`List<String>`, usable with `collections_length`) — 2 interpreter
      tests, 1 typechecker test.
- [x] Verified under `aint run --vm`: `string_split` runs correctly
      with zero VM-specific changes, confirming the generic
      native-dispatch design (milestone 22) still holds for a new
      stdlib addition.
- [x] `examples/router/router.an`: `route`/`dispatch` (a parallel
      `List<String>`/`List<fn(...) -> String>` route table, recursively
      matched, falling through to a `not_found` handler), with any
      `?query` string stripped before path comparison but the full
      path still passed to the matched handler. `query_string`/
      `query_param`/`find_param`: query-string parsing built entirely
      on `string_split`, using `""` as the not-found sentinel (no
      `Option<String>` construction syntax exists — same pattern
      `examples/customer_support/server.an`'s own `field()` helper
      already established).
- [x] `examples/router/router_test.an`: 6 tests against `router.an`
      directly (no server) — dispatch to the matching handler,
      fall-through to `not_found`, a query string not breaking path
      matching, a handler receiving the full path, and `query_param`
      both finding a value and correctly returning `""` for a missing
      key or no query string at all. All 6 pass via the real binary
      (`aint test`).
- [x] `examples/router/demo.an`: a real server (`/`, `/about`,
      `/greet` — the last reading a `name` query parameter) registered
      as a flat table, not nested `if`/`else`. Verified live: `aint
      run`, then every route curled — `/`, `/about`, `/greet` (no
      query, falls back to "stranger"), `/greet?name=Ada` (reads the
      query param correctly), and an unmatched path (falls through to
      the 404 handler) — all returning `200` with the expected HTML
      body.
- [x] `cargo test --workspace` passes with no regressions: 418 tests
      total, up from 415 before this milestone (3 new: 2 interpreter,
      1 typechecker — `router_test.an`'s 6 tests run through the real
      binary, not `cargo test`, matching how `examples/testing.an` and
      `examples/customer_support/priority_logic_test.an` are verified).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Known, honestly-stated gaps

- **No path parameters or wildcards.** `router.an` matches exact paths
  only. See `SPEC.md`'s "Design decisions."
- **No change to `http_serve` itself** — still one connection at a
  time. Untouched by this milestone.
- **Form-body parsing isn't written out as a named library function**
  — `string_split` makes it possible the same way query-string parsing
  is, but nothing in this milestone's verification needed it.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by `string_split` (new stdlib primitive, `aint-runtime` +
`aint-typechecker`) and `examples/router/` (`router.an`,
`router_test.an`, `demo.an`) — verified end to end through the real
binary: `aint check`/`test`/`run`, plus a live `curl` pass against
every route, including one exercising query-string parsing built
entirely on the new primitive. 418 tests total across the workspace,
all passing.
