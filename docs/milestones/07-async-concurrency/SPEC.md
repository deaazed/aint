# Milestone 07 — Async / concurrency — spec

## Scope

`async fn` and `await` as real language constructs, with the interpreter
genuinely driven by a Tokio runtime — not syntax that parses and
type-checks but secretly still runs everything synchronously. This is
pure plumbing: nothing in the language needs asynchrony yet, but
milestone 08 (`infer`) and milestone 11 (`tool`) will make real,
long-running network calls, and retrofitting async onto a synchronous
interpreter later would be far more disruptive than building it now
while there's nothing depending on the old shape.

## In scope

**Syntax:** `async fn name(params) -> Type { ... }` (new `async` keyword)
and `await expr` (new `await` keyword, prefix, same precedence tier as
unary `-`).

**Types:** `Type::Task(Box<Type>)` — not user-writable syntax anywhere
(`parse_type` still only recognizes the seven names from milestone 05);
it only ever appears as a type the checker computes for you. Calling an
`async fn` whose declared return type is `T` produces a value of type
`Task<T>`, not `T` directly. `await` on a `Task<T>` yields `T`; `await`
on anything else is a type error.

**Runtime semantics — tasks are lazy:** calling an async function does
**not** run its body. It captures the function and its (already
evaluated) arguments into a `Value::Task` and returns immediately.
Nothing happens until that task is `await`-ed, at which point the body
actually executes. An async call that's never awaited has no effect at
all — same as Rust's own `Future`s, which do nothing until polled. This
sidesteps needing real background/concurrent scheduling
(`tokio::spawn`) for a milestone that has no genuine concurrent workload
yet, while still being authentically async: the entire interpreter call
graph runs as `async fn` Rust code, under a real Tokio executor, and
`await` inside AINT code triggers a real `.await` in the interpreter.

**One genuinely asynchronous native function, to prove the plumbing
isn't fake:** `time_sleep_ms(ms: Int) -> Unit`, gated behind
`import time` alongside the existing `time_now_seconds`, implemented
with `tokio::time::sleep`. Without at least one operation that actually
suspends and resumes later, "on Tokio" would be an unverifiable claim —
a test measuring wall-clock elapsed time around an `await
time_sleep_ms(...)` call is what actually proves this milestone did
something real, not just add keywords.

**New example:** `examples/async.an`, exercising declaration, `await`,
composing two awaited calls, and the lazy/never-runs-if-unawaited
semantic explicitly.

## Out of scope (later milestones, or deliberately not attempted)

- **No `tokio::spawn`, no background/concurrent task execution.**
  Everything is driven by nested `.await` inside one `block_on` call on
  a **current-thread** Tokio runtime. This isn't a shortcut — see
  "Design decisions" for why it's the architecturally correct choice
  given the interpreter's data model, not a workaround to revisit later
  out of embarrassment.
- **No `parallel { }` block** for running independent work concurrently
  — that's milestone 19 (Optimization), and needs the lazy-task model
  this milestone builds, not the other way around.
- **No real async I/O** — no network calls, no file I/O. `time_sleep_ms`
  is the one deliberately real async primitive, chosen because it's the
  simplest possible thing that genuinely suspends.
- **`Task<T>` is never user-writable syntax.** No function today needs
  to accept or return a `Task<T>` as a *declared* type — it only shows
  up as the type of a call expression. Revisit if/when that stops being
  true.
- **No "await outside an async context" restriction.** Rust and
  (historically) JavaScript restrict `await` to `async fn` bodies; AINT
  does not enforce this. The entire top-level program already runs
  inside `Interpreter::run`, which is itself `async fn` now, so
  top-level `await` is simply consistent with that rather than a
  special case — and enforcing the restriction would mean tracking "am
  I inside an async fn" through type-checking for no correctness benefit
  at this stage. Modern JavaScript modules made the same call for
  similar reasons.

## Design decisions

- **A single-threaded (`current_thread`) Tokio runtime, no `LocalSet`,
  no `tokio::spawn`.** `Value` holds `Rc<RefCell<Environment>>` and
  `Rc<Function>` throughout — neither is `Send`. Rewriting the value
  model to `Arc`/`Mutex` to support real multi-threaded task spawning
  would be a large, invasive change with no workload yet to justify it
  (there is no I/O, no concurrent work, nothing to parallelize).
  `Runtime::block_on` has no `Send` requirement on its future — only
  `tokio::spawn` does — so running everything through nested `.await`
  inside one `block_on` call gets genuine async execution without
  touching the value model at all. If real concurrent scheduling is
  ever needed (milestone 19's `parallel { }`), that's the point to
  reconsider `Send`, not before.
- **Recursive `async fn` via the `async-recursion` crate
  (`#[async_recursion(?Send)]`).** `Interpreter::{run, exec_block,
  exec_stmt, eval_expr, call}` call each other in a cycle, and Rust
  rejects naive recursive `async fn` because the returned future's type
  would be infinitely sized. Hand-writing `Pin<Box<dyn Future>>` at
  every one of these call sites is exactly the kind of boilerplate this
  crate exists to eliminate, and `?Send` is its documented way to opt
  out of the `Send` bound it adds by default — which matches the
  `Rc`-based, single-threaded design above. This is the one new
  dependency added purely for ergonomics rather than capability
  (`tokio` itself is the capability); adding it is judged worth a small,
  focused, widely-used crate over ~30 lines of manual boxing repeated
  across five mutually recursive methods.
- **Existing (sync) tests changed as little as possible.** The two test
  helpers in `interpreter.rs` (`run_capturing`, `run_expect_err`) build
  a throwaway current-thread runtime and `block_on` internally, so all
  28 existing `#[test] fn` bodies stay completely unchanged — only the
  helpers they call were touched. The alternative (converting every
  test to `#[tokio::test] async fn` and adding `.await` at each call
  site) would have meant editing every existing test for a milestone
  whose actual job is adding two new keywords.
