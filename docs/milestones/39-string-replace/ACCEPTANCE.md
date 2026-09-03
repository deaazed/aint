# Milestone 39 — String stdlib: replace — acceptance

## Scope

See `SPEC.md`. `string_replace(s, target, replacement) -> String` —
one new native function, no VM parity gap.

## Acceptance criteria

- [x] `crates/runtime/src/value.rs`: `NativeFunction::StringReplace`.
- [x] `crates/runtime/src/stdlib.rs`: bound as `string_replace` under
      `import string`; replaces every occurrence via `str::replace`,
      except an empty `target` leaves the string unchanged (not Rust's
      own between-every-character behavior for an empty pattern).
- [x] `crates/typechecker/src/stdlib.rs`: signature
      `(String, String, String) -> String`.
- [x] No `crates/vm` changes needed — confirmed by building the whole
      workspace clean immediately after the above three edits: the VM
      compiler already resolves every native call through the same
      shared `stdlib::module_bindings` table, and its VM loop calls
      `aint_runtime::stdlib::call` directly.
- [x] 1 new typechecker test, 5 new interpreter tests (basic
      replacement, an absent target, an empty target, and both
      shrinking and growing the string).
- [x] `examples/string_replace.an` (new) — an `escape_html` rebuilding
      the exact motivating case (`aint-website`'s hand-rolled
      `replace_all`) as four straight-line `string_replace` calls
      instead of a custom recursive-join helper. Verified against the
      real built binary: `aint check` (clean), `aint run` and
      `aint run --vm` (byte-identical output, verified by a dedicated
      CLI integration test asserting both against the same expected
      string — no VM parity gap, as predicted), `aint test`
      (`1 run, 1 passed, 0 failed`).
- [x] `docs/SPECIFICATION.md` §9's stdlib table gets the new function;
      the milestone-39 "not started" known-gap entry removed now that
      it's done. `crates/cli/src/main.rs`'s `aint scaffold` system
      prompt updated so generated code can use `string_replace`
      instead of not having a way to.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **No `string_starts_with`/`string_ends_with`/other string natives.**
  Not what `aint-website`'s retrospective actually found missing —
  see `SPEC.md`'s "Design decisions."

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `string_replace` works identically under both the tree-
walking interpreter and the bytecode VM — no parity gap, unlike
milestones 37/38's if-expressions and `&&`/`||`, since this is a plain
native function call rather than new AST/AIR shape. Verified by the
full pre-existing test suite passing unchanged, 6 new unit tests, and
a real example run through `aint check`/`run`/`run --vm`/`test`
against the actual built binary, output verified byte-identical
between the interpreter and the VM.
