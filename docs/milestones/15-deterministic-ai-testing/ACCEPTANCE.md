# Milestone 15 — Deterministic AI testing — acceptance

## Scope

See `SPEC.md`. `test`/`mock`/`assert` as real language constructs, and
a new `aint test` subcommand — the first AINT-level way to configure
what `infer`/`tool` calls return, closing a gap every AI-touching
milestone since 08 documented and deferred.

## Acceptance criteria

- [x] `test "name" { ... }`, `mock function -> value`, and `assert
      condition` all parse; `test`'s name is a string literal, matching
      `LANGUAGE_DESIGN.md`'s own sketch.
- [x] `mock` outside a `test` block is a positioned type error, not a
      silent no-op.
- [x] `mock`'s target must resolve to a declared `infer` or `tool`
      (not a plain `fn`, not undeclared); its value's type must
      exactly equal that declaration's return type — both checked
      statically, both verified with dedicated tests.
- [x] `assert`'s condition must be `Bool`; `assert` type-checks and
      works identically inside or outside a `test` block (verified
      directly) — it's `aint run` vs. `aint test`'s different handling
      of the resulting `RuntimeError::AssertionFailed` that gives it
      test-specific behavior, not the statement itself.
- [x] Each `test` block runs in a completely fresh `Interpreter`;
      state from one test (mocks, or anything else) never leaks into
      another — verified directly with two tests where only the first
      mocks the shared `infer` function, and the second fails with a
      clear "unconfigured" error rather than inheriting the first
      test's mock.
- [x] A `test` block can call a helper `fn` declared elsewhere in the
      file (every non-`Test` top-level statement is re-executed into
      each test's fresh interpreter before that test's body runs).
- [x] `mock` values are evaluated by a small standalone evaluator with
      no running interpreter involved — literals and
      `EnumName_Variant` references only, exactly as scoped in
      SPEC.md. This sidesteps a real chicken-and-egg problem: the
      `MockModel`/`MockTool` a test needs has to exist *before* that
      test's `Interpreter` does.
- [x] `test` blocks are inert during `aint run` (skipped entirely, no
      behavior change to any pre-15 program) and only run via the new
      `aint test <file>` subcommand.
- [x] `aint test` reports each test's pass/fail status and a summary
      line, exits `0` iff every test passed — verified through the
      real built binary, both the all-passing case
      (`examples/testing.an`) and a deliberately-failing temp-file
      case (checking exact "FAILED"/summary-count output).
- [x] `examples/testing.an` is the **first example able to meaningfully
      use `infer`/`tool` at all** — every prior AI-touching milestone
      (08 through 14) documented this exact gap and deferred it here.
      Three tests, all passing, mocking both an `infer` and a `tool`
      together in one realistic small program (a sentiment-based
      customer greeting).
- [x] `cargo test --workspace` passes with no regressions: 248 tests
      total, up from 225 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — mocking `Distribution<T>`-returning `infer` functions,
scripting a multi-step tool-calling conversation from AINT source
(`MockModel::script` remains Rust-only), custom assertion messages,
general expressions as `mock` values, and multi-file/filtered/parallel
test discovery.

## Outcome

Satisfied by `crates/lexer/src/token.rs` (`test`/`mock`/`assert`
keywords), `crates/ast/src/stmt.rs` (`StmtKind::Test`/`Mock`/`Assert`),
`crates/parser/src/parser.rs` (`parse_test_statement`/
`parse_mock_statement`/`parse_assert_statement`),
`crates/typechecker/src/checker.rs` (`in_test` tracking, `mock`/
`assert` checking), `crates/runtime/src/test_runner.rs` (new: the
whole `aint test` execution model), `crates/runtime/src/{error,
interpreter}.rs` (`RuntimeError::AssertionFailed`/
`UnsupportedMockValue`, `Interpreter::run_statements`, the `Test`/
`Mock`/`Assert` no-op/check arms in `exec_stmt`), `crates/cli/src/
main.rs` (`aint test` subcommand), and `examples/testing.an`. 248
tests total across the workspace, all passing: 5 new parser tests, 10
new typechecker tests, 6 new `test_runner.rs` tests, 1 new runtime
integration test for the example, and 3 new CLI integration tests.
