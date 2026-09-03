# Milestone 40 — URL/query percent-decoding — acceptance

## Scope

See `SPEC.md`. `string_url_decode` (new native) plus
`examples/router/router.an`'s `find_param` actually using it.

## Acceptance criteria

- [x] `crates/runtime/src/value.rs`: `NativeFunction::StringUrlDecode`.
- [x] `crates/runtime/src/stdlib.rs`: bound as `string_url_decode`
      under `import string`; a hand-rolled `url_decode`/`hex_digit`
      pair does strict RFC 3986 decoding (no `+`-to-space), lenient on
      a trailing/malformed `%` (copied through literally) and on
      invalid UTF-8 after decoding (`String::from_utf8_lossy`).
- [x] `crates/typechecker/src/stdlib.rs`: signature `(String) ->
      String`.
- [x] No `crates/vm` changes needed — same reasoning, same confirmation
      method as milestone 39.
- [x] `examples/router/router.an`: a new `decode_query_value` helper
      (`string_url_decode(string_replace(raw, "+", " "))`) that
      `find_param` now runs a matched value through before returning
      it. 1 new test in `examples/router/router_test.an` covering a
      literal `+`, a multi-byte percent-encoded character, and the
      literal-vs-encoded-plus distinction (`%2B` survives as `+`,
      unlike a raw `+`, which becomes a space) — verified directly
      against the real built binary (`aint check`/`aint test`, all 7
      tests in that file passing, the 6 pre-existing ones unchanged).
- [x] 2 new typechecker tests, 4 new interpreter tests (basic
      decoding, a multi-byte UTF-8 character, the literal-`+`-is-left-
      alone property, and lenient handling of a trailing/malformed
      `%`).
- [x] `examples/url_decode.an` (new) — `string_url_decode` directly,
      plus the same `decode_query_value` composition
      `examples/router/router.an` now uses, so the exact pattern a
      real caller would write is exercised end to end. Verified
      against the real built binary: `aint check`/`run`/`test` and
      `aint run --vm` produce byte-identical output (no VM parity
      gap, as predicted), asserted directly by a new CLI integration
      test.
- [x] `docs/SPECIFICATION.md` §9's stdlib table gets the new function;
      the milestone-40 "not started" known-gap entry removed now that
      it's done. `crates/cli/src/main.rs`'s `aint scaffold` system
      prompt updated.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **No percent-*encoding*, no key/path decoding, no general byte-level
  string primitive.** Explicitly out of scope — see `SPEC.md`.
- **`examples/router/router_test.an` isn't wrapped in a `cargo test`
  integration test** — a pre-existing gap in the router example
  (nothing under `examples/router/` was, before or after this
  milestone), not something introduced here. Verified directly against
  the real built binary instead, the same standard this project always
  applies to example programs alongside (not instead of) `cargo test`
  coverage of the underlying stdlib logic itself.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `string_url_decode` works identically under both the
interpreter and the bytecode VM — no parity gap, matching milestone
39's `string_replace` exactly, since it's a native call rather than new
AST/AIR shape. `aint-website`'s `/try` page (and every other caller of
`router_query_param`) now gets a readable query value with no change
on its own end. Verified by the full pre-existing test suite passing
unchanged, 6 new unit tests, 1 new router-level test against the real
binary, and a new dedicated example run through
`aint check`/`run`/`run --vm`/`test`, output verified byte-identical
between the interpreter and the VM.
