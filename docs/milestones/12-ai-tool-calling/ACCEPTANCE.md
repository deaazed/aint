# Milestone 12 — AI tool calling — acceptance

## Scope

See `SPEC.md`. `infer` and `tool` actually connect: a model can
request a tool call mid-inference instead of answering directly, the
runtime validates and executes it, and the result feeds back into the
next call to the model — repeating until it answers.

## Acceptance criteria

- [x] `Model::infer` returns `Result<InferenceOutcome, RuntimeError>`
      (`Answer(Value)` or `CallTool { tool, args }`) instead of
      `Result<Value, RuntimeError>`.
- [x] `eval_inference` is a loop: `Answer` validates and returns
      exactly as milestones 09-10 already built (unchanged,
      `validate_inference_result` untouched); `CallTool` validates the
      requested tool exists, checks argument count *and type* against
      its declared signature, executes it via `MockTool`, validates the
      result, appends `{tool, args, result}` to a running history, and
      calls the model again.
- [x] Argument **type** validation for a model-requested tool call is
      new, runtime code (`validate_value_matches_type`) — the first
      place AINT has ever needed to check a `Value` against a `Type` at
      runtime, since every earlier call site was AINT source the
      static checker had already validated. Covers
      `Int`/`Float`/`Bool`/`String`/`Unit`/`Enum`/`List`/`Option`,
      recursing into `List`/`Option`'s inner type.
- [x] A model requesting a tool that isn't declared is
      `RuntimeError::ToolError` — "a model cannot invoke a tool that
      doesn't exist," now true dynamically, not just statically (which
      milestone 11 already covered for AINT-source calls).
- [x] A model requesting a tool call with the wrong argument count or
      an argument of the wrong type is rejected before the tool ever
      runs (`ArityMismatch` / `SchemaViolation` respectively).
- [x] `InferenceRequest` carries `available_tools` (every declared
      tool's signature) and `history` (prior tool exchanges this
      inference) — both ignored by `MockModel` today, both exactly the
      shape a real adapter (milestone 16) needs.
- [x] `MockModel` is now scriptable: `.script(name, outcomes)` queues a
      sequence of `InferenceOutcome`s, popped one per call.
      `.mock(name, value)` is sugar for a single-`Answer` script — every
      pre-existing test across milestones 08-11 that calls `.mock(...)`
      passes unmodified, confirmed by running the full suite without
      touching any of those call sites.
- [x] Two dedicated tests prove the actual "foundation for agents"
      claim: one model call requesting one tool then answering, and one
      requesting *two* tools in sequence before answering — a real,
      if scripted, multi-step tool-calling conversation running end to
      end through `eval_inference`'s loop.
- [x] `ToolFn.params` changed from `Vec<String>` to `Vec<Type>` (param
      names were always unused — no body ever bound them; the types are
      what runtime validation actually needs).
- [x] `aint run` on all six existing examples is unaffected. No new
      `examples/*.an` (same reasoning as 08-11 — see SPEC.md).
- [x] `cargo test --workspace` passes with no regressions: 202 tests
      total, up from 195 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — a cap on tool-calling loop iterations or budget
(deliberately deferred to milestone 17, not just unimplemented),
tools calling other tools, and effects/permissions on tool calls (13,
20).

## Outcome

Satisfied by `crates/runtime/src/model.rs` (`InferenceOutcome`, the
scriptable `MockModel`, `InferenceRequest`'s new fields),
`crates/runtime/src/tool.rs` (`ToolSignature`, `ToolExchange`),
`crates/runtime/src/value.rs` (`ToolFn.params: Vec<Type>`), and
`crates/runtime/src/interpreter.rs` (`tools_registry`, the rewritten
`eval_inference` loop, `call_requested_tool`, `available_tools`,
`validate_value_matches_type`). 202 tests total across the workspace,
all passing: 2 new `model.rs` tests, and 7 new `interpreter.rs` tests
(2 multi-step conversations, 1 unknown-tool, 1 wrong-argument-type, 1
wrong-argument-count — plus every pre-existing milestone 08-11 test
passing completely unmodified against the changed `Model` trait).
