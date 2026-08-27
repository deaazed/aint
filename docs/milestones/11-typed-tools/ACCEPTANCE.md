# Milestone 11 — Typed tools — acceptance

## Scope

See `SPEC.md`. `tool` as a signature-only declaration — name, typed
input, typed output — structurally mirroring `infer` while staying a
separate primitive, since tools and inference are distinct effects the
roadmap already names separately (milestone 13).

## Acceptance criteria

- [x] `tool` lexes as a keyword; `tool name(params) -> Type` parses
      with no body, exactly like `infer`.
- [x] Calling a `tool`-declared function's call-expression type is
      `Tool<T>`, not `T` directly; `await` now accepts `Task<T>`,
      `Inference<T>`, or `Tool<T>`.
- [x] A tool call still checks argument count and types against its
      declared signature, and is visible to code declared before it
      (same forward-reference hoisting as `fn`/`infer`).
- [x] Calling a tool without awaiting it does not touch `MockTool` at
      all — verified with an unconfigured mock that would error if it
      ran, called but not awaited, program still succeeds.
- [x] `MockTool` is a concrete (non-generic) struct, not a trait
      implementation — `Interpreter<W, M>`'s generic parameter count is
      unchanged from milestone 08. `Interpreter::with_output_and_model`
      and `Interpreter::with_output` still compile and behave exactly
      as before; a new `Interpreter::with_output_model_and_tools`
      configures a custom `MockTool` for tests that need one.
- [x] `MockTool::new().mock(name, value)` configures a canned response
      per tool name; an unconfigured tool call produces a positioned
      `RuntimeError::ToolError`, distinct from `RuntimeError::ModelError`
      (different external system, named honestly).
- [x] Tool results are schema-validated exactly like `infer` results —
      confirmed by reusing `Interpreter::validate_inference_result`
      unmodified, and by a test where a `MockTool` is configured with
      an invalid enum variant and the call is rejected as a
      `SchemaViolation`, same as the equivalent `infer` case.
- [x] A tool and an `infer` function with related names keep fully
      independent mock tables (`MockModel` vs `MockTool`) — verified
      directly.
- [x] "A model cannot invoke a tool that doesn't exist" holds in the
      one form this milestone actually builds: an undeclared tool name
      referenced from AINT source is `TypeError::UndefinedFunction`,
      caught before the program ever runs — the same guarantee any
      undefined function call already gets. The *dynamic*, model-driven
      version (an arbitrary runtime string a type checker never saw) is
      explicitly milestone 12's job.
- [x] The exact unconfigured-tool error message is verified through
      the real built binary, mirroring milestone 08's `infer` CLI test.
- [x] `aint run` on all six existing examples is unaffected.
- [x] `cargo test --workspace` passes with no regressions: 195 tests
      total, up from 178 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — struct/record return types (`Customer`),
`effect`/`permissions`/`timeout` syntax, a model requesting a tool call
at runtime (12), real tool backends, and an `examples/*.an` exercising
a tool call end to end are all deferred with documented reasoning,
each mirroring an equivalent `infer` decision from milestones 08-10.

## Outcome

Satisfied by `crates/lexer/src/token.rs` (`tool` keyword),
`crates/ast/src/{stmt,ty}.rs` (`StmtKind::Tool`, `Type::Tool`),
`crates/parser/src/parser.rs` (`parse_tool_statement`),
`crates/typechecker/src/checker.rs` (`CallMode::ToolCall`, the same
hoisting/validation treatment as `Infer`), `crates/runtime/src/tool.rs`
(new: `MockTool`, `ToolRequest` — no trait, see SPEC.md),
`crates/runtime/src/{value,error,interpreter,lib}.rs`
(`Value::ToolFn`/`Value::ToolCall`, `RuntimeError::ToolError`, the
`tools: MockTool` field and `eval_tool_call`), and
`crates/cli/tests/examples.rs`. 195 tests total across the workspace,
all passing: 2 new parser tests, 6 new typechecker tests, 9 new
runtime tests (7 interpreter, 2 on `MockTool` directly), and 1 new CLI
integration test.
