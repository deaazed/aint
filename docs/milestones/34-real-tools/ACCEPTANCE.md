# Milestone 34 — Real tool execution — acceptance

## Scope

See `SPEC.md`. `tool name(params) -> Type { body }` — an optional
real implementation, run for real whether called directly or requested
by a model, with an explicit `mock` always taking precedence — minus
async tool bodies and VM execution, both named directly as out of
scope.

## Acceptance criteria

- [x] AST: `StmtKind::Tool` gains `body: Option<Block>`
      (`crates/ast/src/stmt.rs`).
- [x] Parser: an optional `{ ... }` block after a tool's return type
      (`parse_tool_statement`) — signature-only (no `{`) keeps parsing
      exactly as before. Verified directly: the existing
      `parses_tool_statement` test extended to assert `body.is_none()`;
      a new `parses_tool_statement_with_a_body` test.
- [x] Typechecker: a tool body is checked exactly like a `fn` body (own
      scope, parameters bound, `MissingReturn` checked via the existing
      `definitely_returns`), untracked for effect-checking, matching a
      lambda's body. Verified directly: a real body type-checks and is
      callable, a missing return is rejected, a wrong return type is
      rejected.
- [x] Runtime: `ToolFn` gains `body: Option<ToolBody>`; `ToolBody`
      captures parameter names, the block, and the defining environment
      (same reasoning as `Function::captured_env`, milestone 30), with
      a manual `PartialEq` comparing only the declaration (same
      reasoning as `Function`'s).
- [x] `call_tool_traced` — the one place a tool call is ever answered,
      direct or model-requested — checks `MockTool` first via a new
      `MockTool::get`, falls back to running a real body
      (`run_tool_body`, the tool-calling counterpart of
      `run_function`) only when nothing's mocked, falls back to
      `MockTool::call`'s own "no mock configured" error when neither
      exists.
- [x] **The mock-precedence bug is verified directly, not just
      asserted in prose**: `an_explicit_mock_wins_over_a_tools_real_body`
      — a tool with a real body, called inside a test that mocks it,
      returns the mocked value, not the real body's computed one. This
      is the one case an unconditional real-body-first implementation
      would have gotten silently wrong, and did, in the first pass
      before this test caught it.
- [x] Verified directly: a real body runs correctly for a direct call
      with nothing mocked, a real body calling a stdlib function
      (`string_concat`) runs for real, and a model-requested tool call
      (via `InferenceOutcome::CallTool`) runs a real body too, not just
      a direct `await`.
- [x] `aint-loader`'s rename pass (`rename_stmt`) renames a tool body's
      statements when the tool is imported from another file, same as
      every other body-bearing declaration.
- [x] `aint-fmt` prints a tool's body back out when present; the
      fmt-test AST-equality helper covers both the with-body and
      without-body cases structurally.
- [x] `aint-ir`'s lowering (`AirStmt::Tool`) is unchanged — it already
      dropped fields it doesn't need via its existing `..` pattern, so
      a tool body is inert there, not a new gap: any attempt to
      actually invoke a tool call still hits the pre-existing, blanket
      `await` rejection under `aint run --vm` first. Verified directly:
      `examples/real_tools.an` still fails clearly under `--vm` with
      the same `await`-unsupported message every tool call already
      produced.
- [x] New example `examples/real_tools.an`: a tool with a real body,
      called directly, plus two tests — one confirming the real body
      runs when nothing's mocked, one confirming `mock` overrides it.
      Verified through the real binary: `aint check`, `aint run`
      (prints `49`), `aint test` (both tests pass), `aint run --vm`
      (fails clearly, unchanged from before this milestone).
- [x] **Verified live against a real model** (Mistral, over
      `AINT_MODEL_URL`/`AINT_MODEL_API_KEY`) — outside this milestone's
      own scope strictly speaking, but done in the same session: a
      plain `infer` call answered correctly, and `aint scaffold` (
      milestone 32) was exercised against a real model for the first
      time, correctly refusing to report success on a response that
      didn't type-check while still writing it to disk — closing the
      "never verified against a live LLM" gap milestone 32's own
      `ACCEPTANCE.md` named directly.
- [x] `cargo test --workspace` passes with no regressions: 435 tests
      total, up from 427 before this milestone (7 new: 3 typechecker,
      4 interpreter, including the mock-precedence regression test).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Known, honestly-stated gaps

- **No async tool bodies** — same restriction closures accepted at
  milestone 30, for the same reason.
- **A real tool body still can't run under `aint run --vm`** — blocked
  by the pre-existing, unconditional `await` rejection, not attempted.
- **`permissions`/`budget` enforcement is unchanged** — real bodies
  don't get special treatment or new restrictions there.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by AST/parser/typechecker/interpreter changes across
`crates/ast`, `crates/parser`, `crates/typechecker`, `crates/runtime`
(`value.rs`, `tool.rs`, `interpreter.rs`), a matching update to
`aint-loader`'s rename pass and `aint-fmt`'s printer, and
`examples/real_tools.an` verified end to end through
`aint check`/`run`/`test`/`run --vm`. 435 tests total across the
workspace, all passing.
