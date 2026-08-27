# Milestone 14 — AI execution tracing — acceptance

## Scope

See `SPEC.md`. `Inference #N` / `Tool Call #N` trace records, captured
unconditionally for every `infer`/`tool` call — no import, no opt-in.

## Acceptance criteria

- [x] Every `self.model.infer(...)` call (`eval_inference`, the only
      place it happens) produces an `Inference` trace record: id,
      function name, backend (`"mock"`), token usage (always `{0, 0}`
      today), real measured latency, and outcome.
- [x] Every `self.tools.call(...)` call — both the direct-call path
      (`eval_tool_call`, milestone 11) and the model-requested path
      (`call_requested_tool`, milestone 12) — routes through one new
      shared `call_tool_traced` helper, producing a `ToolCall` trace
      record with id, tool name, args, latency, and outcome.
- [x] A trace record is captured on failure as well as success — a
      failed `infer`/`tool` call still produces a record, with
      `InferenceTraceOutcome::Error`/`Err(String)` respectively.
      Verified directly with unconfigured `MockModel`/`MockTool`.
- [x] `Inference #N` and `Tool Call #N` use independent counters, each
      starting at 1, matching `ROADMAP.md`'s own notation exactly
      (`TraceRecord::label()`).
- [x] A multi-step tool-calling conversation (milestone 12) produces
      the full, correctly-interleaved sequence of trace records — one
      test scripts a model asking for two tools in sequence before
      answering and asserts the exact five-record label sequence:
      `Inference #1, Tool Call #1, Inference #2, Tool Call #2,
      Inference #3`.
- [x] Latency is a genuinely measured `Duration` (`Instant::now()` /
      `.elapsed()` around the actual call), not a placeholder —
      verified structurally (a real, small `Duration` exists), not by
      asserting an exact or nonzero value, since a mock call can
      legitimately round to zero.
- [x] `Interpreter::traces() -> Vec<TraceRecord>` is the only new
      public surface — no CLI output change, no AINT-level syntax (see
      SPEC.md for why presenting traces is out of scope here).
- [x] `aint run` on all six existing examples is unaffected; no new
      `examples/*.an` (tracing has no AINT-visible behavior to
      demonstrate).
- [x] `cargo test --workspace` passes with no regressions: 225 tests
      total, up from 219 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Notable finding during implementation

`TraceRecord` embeds `Value` (through both `InferenceTraceOutcome` and
`ToolCall`'s own `args`/`outcome` fields), and `Value` holds `Rc`
throughout — so `Vec<TraceRecord>` isn't `Send` any more than
`Interpreter` itself is. The first draft of these tests tried to
`return interpreter.traces()` out of the big-stack thread
(`run_on_big_stack`, milestone 07) the way `run_capturing` returns a
plain `String`; that doesn't compile for the same reason a `MockModel`
holding a mocked `Value` couldn't be built outside the thread in
milestone 08. Fixed by moving every assertion *inside* the big-stack
closure instead of extracting the traces out of it.

## Explicitly out of scope

See `SPEC.md` — real token counts (16), a `Model` trait method for
backend identification, CLI presentation of trace logs, and exposing
traces to AINT source itself.

## Outcome

Satisfied by `crates/runtime/src/trace.rs` (new: `TraceRecord`,
`TokenUsage`, `InferenceTraceOutcome`) and
`crates/runtime/src/interpreter.rs` (`traces`/`next_inference_id`/
`next_tool_call_id` fields, tracing wired into `eval_inference` and
the new `call_tool_traced`, `Interpreter::traces()`). 225 tests total
across the workspace, all passing: 6 new runtime tests covering
success, failure, both call paths, the multi-step conversation
sequence, and latency/token-placeholder shape.
