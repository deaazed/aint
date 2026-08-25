# Milestone 05 — Core type system — acceptance

## Scope

See `SPEC.md`. Typed `fn` signatures, a real static type checker, and
`aint run` rejecting ill-typed programs before they execute.

## Acceptance criteria

- [x] Each of the 7 type forms parses correctly, including nested
      generics (`List<Int>`, `Option<String>`); an unrecognized type
      name is a `ParseError`, not deferred to the type checker.
- [x] `fn add(a: Int, b: Int) -> Int { ... }` parses with the right
      `Param` name/`ty` pairs and `return_type`; `examples/fibonacci.an`
      updated to typed signatures and still runs correctly.
- [x] `1 + 2` infers `Int`, `1.0 + 2.0` infers `Float`; `1 + 1.0` is a
      type error; `1 == "x"` is a type error (stricter than the
      interpreter's own permissive runtime behavior, on purpose);
      unary negation requires `Int` or `Float`.
- [x] `let` infers its type from the initializer; a non-`Bool` `if`
      condition is a type error; a `let` inside an `if`-block isn't
      visible to type-checking after the block ends.
- [x] `add("hello", true)` against `fn add(a: Int, b: Int) -> Int` is
      rejected with a positioned `ArgumentTypeMismatch` pointing at the
      first bad argument; wrong argument count is a positioned
      `ArityMismatch`, now caught before running; a function that
      forward-references a later-declared top-level function
      type-checks; self-recursion (fibonacci) type-checks; calling an
      undefined name or a non-function are each positioned errors.
- [x] Returning the wrong type is a positioned `ReturnTypeMismatch`; a
      non-`Unit` function with no `return`, or with an `if` lacking an
      `else` on its only return path, is a positioned `MissingReturn`;
      `if`/`else` where both branches return counts as returning; a
      `Unit`-returning function needs no `return` at all.
- [x] `aint run` on a well-typed program behaves exactly as in
      milestone 04; on an ill-typed program it exits non-zero, prints a
      positioned message to stderr, and produces **no stdout output at
      all** — verified against the actual built binary via a subprocess
      test, proving the interpreter genuinely never ran.
- [x] `cargo test -p aint-typechecker` and `cargo test -p aint` both
      pass, including the new CLI subprocess tests.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — constructing `List`/`Option` values, typed `let`,
user-defined named types, general generics, a general native-function
signature mechanism, and multi-error collection are all deferred.

## Outcome

Satisfied by `crates/ast/src/{ty,stmt}.rs` (`Type`/`Param` and the new
`StmtKind::Fn` shape), `crates/parser/src/parser.rs` (`parse_type`,
typed `fn` parsing), `crates/typechecker/src/{checker,error,lib}.rs`
(new), `crates/runtime/src/interpreter.rs` (updated `Fn` handling),
`crates/cli/src/main.rs` (typechecking wired in), and
`crates/cli/tests/examples.rs` (new). 84 tests total across the
workspace, all passing: 18 new in `aint-typechecker`, 3 new CLI
subprocess tests, plus updates to existing parser/runtime tests that
used the old untyped `fn` syntax.
