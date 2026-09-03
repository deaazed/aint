# Milestone 37 — Conditional expressions — acceptance

## Scope

See `SPEC.md`. `if`/`else` as an expression (`ExprKind::If`, single-
expression branches, `else` required), plus `else if` as parser-level
sugar for `else { if ... }` in both the statement and expression
forms.

## Acceptance criteria

- [x] `ExprKind::If { condition, then_value, else_value }`
      (`crates/ast/src/expr.rs`) — a new, separate node from
      `StmtKind::If`.
- [x] `crates/parser`: `parse_if_expr` reachable only from expression
      position (`parse_primary`, alongside lambdas); a bare `if` at
      statement position is completely unaffected (`parse_statement`
      still dispatches to the unchanged `parse_if_statement` first).
      `else if` recurses in both `parse_if_statement` (wraps the
      recursive result in a single-statement `Block`) and
      `parse_if_expr` (uses the recursive result directly as
      `else_value`). 4 new parser tests: a bare if-expression parses,
      an `else if` chain parses flat (via the `describe_expr` S-expr
      helper), an if-expression with no `else` is a clear parse error,
      and a statement-form `else if` desugars to the expected nested
      `Block`/`If` shape.
- [x] `crates/typechecker`: condition must be `Bool`, both branches
      must type-check to the same `Type` (that type is the whole
      expression's type), reusing `TypeError::Mismatch` for both
      failure shapes. 4 new tests: common-type success, non-`Bool`
      condition rejected, mismatched branch types rejected, an
      `else if` chain type-checks through every link.
- [x] `crates/runtime`: evaluates the condition, then whichever
      branch's single expression was taken — no new `Value` variant
      needed. 2 new tests: the taken branch's value is returned in
      both directions, and an `else if` chain evaluates the first
      matching branch.
- [x] `crates/ir`: `ExprKind::If` fails immediately at lowering with a
      new `LowerError::UnsupportedIfExpr` — no AIR node for it, same
      shape as `UnsupportedLambda`. The *statement* form (`else if`
      included, since it's pure sugar) is completely unaffected and
      still lowers and runs under `aint run --vm` exactly as before.
- [x] `crates/loader`: `rename_expr`'s cross-file-import renaming walks
      into `ExprKind::If`'s three sub-expressions — required for the
      match to stay exhaustive, verified by the full existing
      `aint-loader` test suite still passing unchanged.
- [x] `crates/fmt`: prints the expression form as `if cond { a } else
      { b }` (or flat `else if ... else { }` for a chain, by checking
      whether `else_value` is itself `ExprKind::If`); recognizes the
      statement form's desugared `else if` shape (a `Block` holding
      exactly one `If` statement) and prints it back as `else if`
      rather than an extra nested `{ }`. 3 new focused unit tests in
      `crates/fmt/src/lib.rs`, plus the existing example-corpus
      idempotence/AST-preservation test unaffected (unchanged, since
      no shipped comment-free example used the new syntax before this
      milestone's own new example, which — like every other shipped
      file with a `//` comment — is exempt from that test rather than
      added to it).
- [x] `examples/conditional_expressions.an` (new) — a `sign` function
      using the expression form with a two-level `else if` chain, a
      `grade` function using the same `else if` sugar in the existing
      statement form, and a `let` bound to an if-expression's result.
      Verified against the real built binary: `aint check` (clean),
      `aint run` (`negative\nzero\npositive\nF\nB\nA\nyes\n`, matching
      by hand-tracing both functions), `aint run --vm` (fails clearly
      with `UnsupportedIfExpr`, not a miscompilation), `aint test`
      (`1 run, 1 passed, 0 failed`), `aint fmt --check` (refused for
      the right, existing reason — the file has `//` comments — not a
      new failure mode). 2 new CLI integration tests in
      `crates/cli/tests/examples.rs` mirror the existing
      `closures.an` pattern exactly (`a_program_using_if_expressions_
      fails_clearly_under_the_vm`, plus print/test-block checks).
- [x] `docs/SPECIFICATION.md` §4.2 rewritten to document both forms and
      `else if`; a new known-gap entry for the VM parity gap, placed
      next to the existing closures one it mirrors; the milestone-37
      "not started" gap entry (added when Phase 3 was first planned)
      removed now that it's done. `crates/cli/src/main.rs`'s
      `aint scaffold` system prompt updated so generated code can use
      `else if` and if-expressions instead of always duplicating a
      `return` per branch.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **`if`/`else` used as a value doesn't run under `aint run --vm`.**
  Documented, not attempted — see `SPEC.md`'s "Design decisions." The
  statement form (`else if` included) is unaffected.
- **Branches are exactly one expression, not a block with a tail
  expression.** A `let`/intermediate statement inside a branch still
  needs the old duplicated-`return` shape. Real, deferred — see
  `SPEC.md`.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `if`/`else` is usable as a value, `else if` chains flatly in
both forms, and every existing program is unaffected — verified by the
full pre-existing test suite passing unchanged, 15 new unit/integration
tests across five crates, and a real example run through `aint check`/
`run`/`run --vm`/`test`/`fmt --check` against the actual built binary.
