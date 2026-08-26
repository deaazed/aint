# Milestone 07 — Async / concurrency — acceptance

## Scope

See `SPEC.md`. `async fn`/`await` as real language constructs, driven by
a genuine Tokio runtime.

## Acceptance criteria

- [x] `async` and `await` lex as keywords.
- [x] `async fn` parses with `is_async: true`; plain `fn` still parses
      with `is_async: false`; `await expr` binds at the same precedence
      tier as unary `-`.
- [x] Calling an async function's call-expression type is `Task<T>`, not
      `T` directly; `await` on a `Task<T>` yields `T`; `await` on
      anything else is a positioned type error; `await
      time_sleep_ms(10)` type-checks as `Unit` after `import time`;
      missing-return / return-type-mismatch analysis is unaffected by
      `is_async` (checks the function's own declared type).
- [x] An async function's body does not execute at all if the call is
      never awaited — verified with a function that would error if it
      ran (`1 / 0`), called but not awaited, program still succeeds.
- [x] A test measures real wall-clock elapsed time around `await
      time_sleep_ms(...)` (≥25ms for a 30ms sleep) — proof of a genuine
      suspend/resume through Tokio, not synchronous code in async
      syntax.
- [x] `aint run` on `examples/{hello,fibonacci,stdlib,showcase}.an`
      behaves identically to before this milestone, verified against
      the actual built binary — including `showcase.an`, which exposed
      a real stack-overflow regression along the way (see "Notable
      finding" below) and now passes cleanly.
- [x] `examples/async.an` runs correctly through the actual built
      binary: declares two async functions, awaits both, and calls one
      without awaiting to demonstrate the body never runs.
- [x] `cargo test --workspace` passes with no regressions: 130 tests
      total, up from 114 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Notable finding during implementation

Converting the interpreter's evaluation chain to `async fn` (necessary,
not optional — see `SPEC.md`) made every level of AINT-level recursion
cost far more Rust stack than before: roughly five mutually recursive
`async fn`s (`call` → `run_function` → `exec_block` → `exec_stmt` →
`eval_expr` → `call`) nest per level, instead of one plain function
call. `examples/showcase.an`'s Collatz(27) — 111 levels, which ran fine
under the milestone 06 synchronous interpreter — overflowed the default
thread stack once every eval step was async. Since AINT's only iteration
mechanism is recursion (there are no loops, and none are planned), this
isn't an edge case to shrug off — it's exactly the shape real AINT
programs take.

Fixed by running the interpreter on a dedicated OS thread with a 64 MiB
stack, in both the CLI (`crates/cli/src/main.rs`) and every test that
exercises a real program (`crates/runtime/tests/common/mod.rs`,
`interpreter.rs`'s own test helpers). `Interpreter` holds `Rc`, so it
can't be constructed outside and moved into a spawned thread — parsing,
type-checking, and running all had to move *inside* the spawned
closure, which only captures `Send` data (the source text). This is
documented as a `SPEC.md` design decision, not a footnote, since it's
now a standing property of how any AINT program has to be run.

## Explicitly out of scope

See `SPEC.md` — `tokio::spawn`, background/concurrent execution,
`parallel { }`, real network I/O, `Task<T>` as user-writable syntax, and
an "await outside async context" restriction are all deferred with
documented reasoning.

## Outcome

Satisfied by `crates/lexer/src/{token,lexer}.rs` (keywords),
`crates/ast/src/{ty,stmt,expr}.rs` (`Type::Task`, `is_async`,
`ExprKind::Await`), `crates/parser/src/parser.rs` (`async fn`/`await`
parsing), `crates/typechecker/src/{stdlib,checker,error}.rs` (`Task<T>`
wrapping and unwrapping), `crates/runtime/src/{value,stdlib,
interpreter,error}.rs` (the async interpreter, lazy `Task` values,
`time_sleep_ms`), `crates/cli/src/main.rs` (Tokio-driven, big-stack
thread), and `examples/async.an`. 130 tests total across the workspace,
all passing: 16 new/updated in `aint-typechecker` and `aint-runtime`
combined, 2 new parser tests, 2 new CLI/runtime integration tests for
`async.an`, plus the `showcase.an` regression caught and fixed.
