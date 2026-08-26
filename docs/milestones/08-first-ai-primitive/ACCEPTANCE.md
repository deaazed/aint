# Milestone 08 — First AI primitive — acceptance

## Scope

See `SPEC.md`. `infer` as a signature-only declaration, `Inference<T>`,
a `Model` trait, and `MockModel` — the first real AI-native primitive
in the language.

## Acceptance criteria

- [x] `infer` lexes as a keyword.
- [x] `infer name(params) -> Type` parses with no body — new
      `StmtKind::Infer`, distinct from `StmtKind::Fn` rather than a
      body-optional variant of it.
- [x] Calling an `infer`-declared function's call-expression type is
      `Inference<T>`, not `T` directly — using the result without
      `await` (e.g. as an `if` condition) is a positioned type error.
- [x] `await` on an `Inference<T>` yields `T`; `await` still also
      accepts `Task<T>` from milestone 07 — both are real, distinct
      types, not aliases of each other.
- [x] Calling an `infer` function still checks argument count and
      types against its declared signature; an `infer` function is
      visible to code declared before it (same forward-reference
      hoisting as top-level `fn`).
- [x] Calling an `infer` function without awaiting it does not touch
      the model at all — verified with an unconfigured `MockModel`
      that would error if it ran, called but not awaited, program
      still succeeds. Mirrors the equivalent milestone-07 proof for
      `async fn`.
- [x] `Model` is a plain generic trait (`M: Model`, static dispatch),
      implemented once by `MockModel`; `Interpreter<W, M: Model =
      MockModel>` keeps `Interpreter::new()` and
      `Interpreter::with_output(...)` compiling and behaving exactly as
      before this milestone.
- [x] `MockModel::new().mock(name, value)` configures a canned response
      per function name; awaiting an `infer` call with a response
      configured returns it; awaiting one with nothing configured
      produces a positioned `RuntimeError::ModelError` with a clear
      message (`no mock response configured for `name``), not a panic
      or a guessed default.
- [x] The exact unconfigured-model error message is verified through
      the real built binary (`aint run` on a temp `.an` file), not just
      library-level tests — same rigor as the milestone-05 type-error
      CLI test.
- [x] `aint run` on all five existing examples
      (`hello`/`fibonacci`/`stdlib`/`showcase`/`async.an`) is
      unaffected, verified against the actual built binary.
- [x] `cargo test --workspace` passes with no regressions: 143 tests
      total, up from 130 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace, including `#[allow(async_fn_in_trait)]`
      on `Model` (documented at the trait definition — no `dyn Model`
      anywhere, so the lint's underlying concern doesn't apply).

## Notable finding during implementation

A test helper that built a `MockModel` (with a configured `Value`)
*outside* the big-stack thread established in milestone 07, then
captured it into the thread's closure, failed to compile — `Rc`-based
`Value` isn't `Send`, and that's just as true of a `Value` sitting
inside a `MockModel`'s response table as it is of one sitting inside an
`Interpreter`. The fix follows the same rule milestone 07 already
established for `Interpreter` itself: `run_capturing_with_model` takes
a `Send` *builder closure* for the model, and calls it from inside the
spawned thread, rather than taking a pre-built `MockModel`.

## Explicitly out of scope

See `SPEC.md` — `enum`/structured return types, `Distribution<T>` and
uncertainty, real model backends, AINT-level `mock`/`test` syntax,
tracing metadata on `Inference<T>`, tool calls, and effects are all
deferred with documented reasoning. No new `examples/*.an` file for
this milestone, for the same reason: there's nothing left to run
without either milestone 15 or milestone 16 in place, which either
oversells the feature or ships a program designed to fail.

## Outcome

Satisfied by `crates/lexer/src/token.rs` (`infer` keyword),
`crates/ast/src/{stmt,ty}.rs` (`StmtKind::Infer`, `Type::Inference`),
`crates/parser/src/parser.rs` (`parse_infer_statement`),
`crates/typechecker/src/checker.rs` (`CallMode` replacing a plain
`is_async` bool, `Inference<T>` wrapping and unwrapping),
`crates/runtime/src/model.rs` (new: `Model`, `InferenceRequest`,
`MockModel`), `crates/runtime/src/{value,error,interpreter,lib}.rs`
(`Value::InferenceFn`/`Value::Inference`, `RuntimeError::ModelError`,
the generic `Interpreter<W, M>`), and
`crates/cli/tests/examples.rs`. 143 tests total across the workspace,
all passing: 2 new parser tests, 4 new typechecker tests, 6 new
runtime tests (4 interpreter, 2 on `MockModel` directly), and 1 new
CLI integration test.
