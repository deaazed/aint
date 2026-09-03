# Milestone 38 — Comparison and logical operators — acceptance

## Scope

See `SPEC.md`. `<=`, `>=`, `!` (mechanical, no VM parity gap) and
`&&`, `||` (real short-circuit evaluation, VM-rejected like closures
and if-expressions).

## Acceptance criteria

- [x] `crates/lexer`: `TokenKind::Bang` (standalone `!`, not just
      `!=`), `LessEqual`, `GreaterEqual`, `AmpAmp`, `PipePipe`. A lone
      `&`/`|` (not doubled) is still a clear `UnknownCharacter`, not a
      new token. New/updated lexer tests cover every new token and the
      lone-`&`/`|` rejection.
- [x] `crates/ast`: `BinaryOp` gains `LessEq`/`GreaterEq`/`And`/`Or`;
      `UnaryOp` gains `Not`.
- [x] `crates/parser`: two new precedence levels (`parse_or` above
      `parse_and` above the existing `parse_equality`), `parse_expr`
      now starts at `parse_or`; `<=`/`>=` fold into the existing
      `parse_comparison`; `!` parses in `parse_unary` alongside `-`.
      7 new precedence/parsing tests (`<=`/`>=`, `!` and its binding
      relative to `==`, `&&` binding tighter than `||`, a mixed chain
      proving the full new ordering).
- [x] `crates/typechecker`: `<=`/`>=` type-check like `<`/`>`; `!`
      requires `Bool`, yields `Bool`; `&&`/`||` require `Bool` on both
      sides, yield `Bool`. Both branches of `&&`/`||` are always
      checked regardless of short-circuiting — type-checking has no
      notion of "this might not run." 4 new tests.
- [x] `crates/runtime`: `<=`/`>=`/`!` evaluate normally. `&&`/`||`
      short-circuit for real — `eval_expr`'s `ExprKind::Binary` arm
      special-cases `BinaryOp::And`/`Or` *before* evaluating the right
      operand, rather than evaluating both and applying the op after
      (every other operator's path, unchanged). 8 new tests, including
      two that prove short-circuiting isn't just an ordering nicety: a
      right operand that would divide by zero if evaluated genuinely
      doesn't run (for both `&&` and `||`), and a right operand with
      its own `print` genuinely produces no output when short-circuited
      away.
- [x] `crates/ir`: `<=`/`>=`/`!` lower normally — no AIR changes needed,
      since `AirExpr::Binary`/`Unary` already carry `aint_ast::BinaryOp`/
      `UnaryOp` directly. `&&`/`||` are rejected at lowering with a new
      `LowerError::UnsupportedShortCircuit`, mirroring
      `UnsupportedLambda`/`UnsupportedIfExpr`'s shape exactly. 2 new
      unit tests (successful lowering for the mechanical operators,
      rejection for both `&&` and `||`).
- [x] `crates/vm`: `eval_unary`/`eval_binary` in `vm.rs` (which
      duplicates `aint-runtime`'s own by design, per `SPEC.md`) get the
      new mechanical-operator arms; `And`/`Or` are `unreachable!()`
      there, since `aint-ir` never lets one reach AIR in the first
      place.
- [x] `crates/fmt`: `binary_precedence`/`binary_symbol`/the unary
      symbol match all updated; precedence-based parenthesization
      verified to both omit unneeded parens (`a <= b && b >= a || !c`
      round-trips with none) and keep required ones (`(a || b) && c`
      keeps its parens, since `&&` binds tighter than the source's
      `||`). 2 new focused unit tests.
- [x] `examples/comparison_operators.an` (new) — `<=`/`>=`/`!` only,
      deliberately kept free of `&&`/`||` to prove there's no VM parity
      gap for these three: `aint run` and `aint run --vm` produce
      *identical* output, verified with a dedicated CLI integration
      test asserting both outputs equal the same expected string, not
      just that both succeed.
- [x] `examples/logical_operators.an` (new) — `&&`/`||`, including a
      division-by-zero-avoiding boundary check that would crash the
      program if short-circuiting were broken, not just print things
      in a surprising order. Verified: `aint check`/`run`/`test` all
      succeed with the predicted output; `aint run --vm` fails clearly
      with `UnsupportedShortCircuit`, not a miscompilation.
- [x] `docs/SPECIFICATION.md` §5 rewritten with the full new operator
      grammar, updated precedence line, and short-circuiting stated
      explicitly as a real semantic property, not an implementation
      detail; two new known-gap entries (the VM parity gap, and the
      now-resolved milestone-38-not-started entry removed).
      `crates/cli/src/main.rs`'s `aint scaffold` system prompt updated
      so generated code can use the new operators instead of always
      restating a boundary check with only `<`/`>`/`==`.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **`&&`/`||` don't run under `aint run --vm`.** Documented, not
  attempted — see `SPEC.md`'s "Design decisions." `<=`/`>=`/`!` have
  no such gap.
- **No bitwise operators, no compound assignment.** Explicitly out of
  scope — see `SPEC.md`.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. All five operators work under the tree-walking interpreter;
`<=`/`>=`/`!` work identically under the bytecode VM with zero parity
gap; `&&`/`||` short-circuit for real (proven, not assumed, by tests
that would fail with a crash or unexpected output if they didn't) and
are rejected cleanly rather than silently miscompiled under the VM.
Verified by the full pre-existing test suite passing unchanged, 23 new
unit/integration tests across six crates, and two real examples run
through `aint check`/`run`/`run --vm`/`test`/`fmt --check` against the
actual built binary.
