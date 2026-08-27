# Milestone 18 — Compiler IR (AIR) — acceptance

## Scope

See `SPEC.md`. A lowering pass from the type-checked AST to AIR, with
`infer`/`tool` calls and `Distribution<T>`'s operations represented as
explicit node kinds instead of generic function calls — the
prerequisite `ROADMAP.md` names for milestones 19 (optimization) and
22 (bytecode VM), neither of which this milestone touches.

## Acceptance criteria

- [x] `AirProgram`/`AirStmt`/`AirExpr` are a parallel type set in
      `aint-ir`, not a generic-ified `aint-ast` — `aint-ast` itself is
      completely unchanged by this milestone.
- [x] A call to a declared `infer` function lowers to
      `AirExpr::Infer`; a call to a declared `tool` lowers to
      `AirExpr::ToolCall` — both verified directly, including that the
      surrounding `await` still lowers to `AirExpr::Await` wrapping
      the explicit node.
- [x] Each of `distribution_argmax`/`entropy`/`sample`/
      `require_confidence` lowers to `AirExpr::Distribution` tagged
      with which one (verified for all four in one parameterized
      test); `distribution_probability` lowers to its own
      `AirExpr::Probability` node, matching `ROADMAP.md` naming it
      separately from `DISTRIBUTION`.
- [x] `option_is_some`/`option_unwrap` — not named in `ROADMAP.md`'s
      list — lower to plain `AirExpr::Call`, confirmed directly, not
      given special treatment despite `Option<T>` being AI-adjacent.
- [x] Every other call (`fn`, `async fn`, `print`, any stdlib function)
      lowers to `AirExpr::Call` — verified for a plain `fn`, `print`,
      and a stdlib function (`math_sqrt`).
- [x] Every one of the 13 `StmtKind` variants lowers correctly,
      verified in one test covering all of them in a single program
      (`budget`/`enum`/`infer`/`tool`/`fn`/`async fn`/`import`/`let`/
      expression/`test`-with-`mock`-and-`assert`), plus a dedicated
      test for `if`/`else` lowering both branches.
- [x] Lowering's `infer`/`tool` name recognition is top-level-only, by
      its own separate pre-pass (not sharing `aint-typechecker`'s
      internals) — verified directly against a block-nested `infer`
      declaration, confirming it's correctly *not* recognized (a
      documented limitation, not silently wrong behavior).
- [x] `crates/runtime` is untouched by this milestone — `aint run`/
      `aint test` still execute the tree-walking interpreter exactly
      as before; AIR is not wired into either.
- [x] `cargo test --workspace` passes with no regressions: 281 tests
      total, up from 269 before this milestone (12 new, all in
      `aint-ir`).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — wiring AIR into the interpreter or CLI, any
optimization (19), a bytecode form or VM (22), struct/record lowering
(no such AST node exists), and recognizing block-nested `infer`/`tool`
declarations.

## Outcome

Satisfied by `crates/ir/src/air.rs` (new: `AirProgram`/`AirStmt`/
`AirExpr`/`DistributionOp`), `crates/ir/src/lower.rs` (new: `lower`,
`LowerError`, the `Lowerer` pre-pass and recursive lowering),
`crates/ir/src/lib.rs` (re-exports), and `crates/ir/Cargo.toml`
(`aint-parser`/`aint-typechecker` dev-dependencies). 281 tests total
across the workspace, all passing: 12 new tests in `aint-ir` covering
every AI-operation node kind, the generic-call fallback, full
statement-kind coverage, and the top-level-only name recognition
limitation.
