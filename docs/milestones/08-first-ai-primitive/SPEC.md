# Milestone 08 — First AI primitive

## Scope

`ROADMAP.md` states this milestone as:

> `infer sentiment(text: String) -> Sentiment` and an `Inference<T>` type,
> backed by a `Model` trait with a `MockModel` implementation from day
> one — AI-touching code must be testable without a live model from this
> milestone forward.

Taken literally, that example can't be built yet: `Sentiment` is a
user-defined enum, and enums don't exist until milestone 09 ("Typed
structured inference"). The roadmap's own split is deliberate — 08 is
the primitive, 09 is structured typing on top of it — so this milestone
targets the same shape over the types that already exist (`Bool`,
`Int`, `Float`, `String`), e.g. `infer is_positive(text: String) ->
Bool`. Milestone 09 swaps in `enum` return types once enums exist; the
`infer` mechanism itself does not change.

This milestone delivers:

- `infer` as a new statement form: a **signature-only** function
  declaration, no body.
- `Type::Inference<T>`, the type of an unawaited `infer` call.
- A `Model` trait in `aint-runtime`.
- `MockModel`, the only implementation, configured entirely from Rust
  (no AINT-level mock syntax yet — that's milestone 15).
- `Interpreter` generic over the model it runs against, defaulting to
  `MockModel`.

## Design decisions

**`infer` has no body.** `fn`/`async fn` are AINT source implementing
its own logic; `infer` declares a *capability* whose implementation is
external (a model, later a real one). This mirrors `tool` (milestone
11), which the roadmap describes the same way: name and typed
signature, no body. Reusing `fn`'s shape (with a dummy/empty body)
was considered and rejected — it would suggest `infer` bodies are
meaningful AINT code, which they never are.

**`infer` is inherently async, with no `async` keyword of its own.**
Every inference is a real (eventually network) round trip; there's no
useful "synchronous infer" the way there's a useful synchronous `fn`.
Calling an `infer`-declared function returns `Inference<T>`, not `T`;
`await` unwraps it, exactly like `Task<T>` from milestone 07. This
reuses milestone 07's whole async foundation — the SPEC for that
milestone says outright it exists because "inference and tools ... are
inherently asynchronous." This is the payoff.

**`Inference<T>` is a distinct type from `Task<T>`, not an alias.**
They're both "a deferred value," but `Inference<T>` is where
inference-specific metadata attaches later — model id, token usage,
latency, trace id (milestone 14), and eventually the distribution
machinery (milestone 10). Keeping them separate types now avoids a
disruptive split later. `await` accepts either; nothing else treats
them as interchangeable.

**No AINT-level mock configuration yet.** The natural way to give an
`infer` call something to return without a live model is AINT source
like `mock sentiment { ... }` — but that's milestone 15's `test { mock
... }` block, and building a parallel, throwaway mechanism now would
just be discarded later. So for this milestone, `MockModel`'s canned
responses are configured only through its Rust API
(`MockModel::new().mock("name", value)`), which is what the test suite
uses. A plain `aint run` against a program that calls and awaits an
`infer` function with nothing configured gets a clear runtime error —
`model error: no mock response configured for `name`` — not a crash,
not a silent default. That gap (no way to get a real answer from
`aint run` yet) is real and is closed by milestone 16 (real model
adapters) and/or milestone 15 (AINT-level mocking).

**Consequently, no new `examples/*.an` file for this milestone.**
Every other milestone's example is a complete, successful program run
through the real CLI. An `infer`-using program can't be that yet
without either overselling capability (faking a "real" answer) or
shipping a program that's designed to fail. Both undercut the
example directory's own purpose. Milestone 08 is verified instead by
runtime unit tests (configuring `MockModel` directly), typechecker
tests, and one CLI-level integration test that asserts the exact,
honest failure message end to end through the real built binary — the
same rigor as every other example, pointed at the feature this
milestone actually delivers.

**`Model` uses static dispatch, not `dyn Model`.** Only one
implementation exists (`MockModel`); dynamic model selection between
multiple real backends is milestone 16's problem, once there's more
than one implementation to choose between. `Interpreter<W, M: Model =
MockModel>` keeps every existing call site
(`Interpreter::new()`, `Interpreter::with_output(...)`) compiling
unchanged. Native `async fn` in a generic (non-`dyn`) trait needs no
`async-trait` crate — Rust's own async-fn-in-traits (stable since
1.75) is sufficient here specifically because nothing ever needs a
`Box<dyn Model>`. `#[allow(async_fn_in_trait)]` is applied at the
trait definition with this reasoning in a comment, since rustc's
default lint assumes trait objects are coming.

## Explicitly out of scope

- `enum`/structured return types (milestone 09).
- `Distribution<T>`, `probability()`, `argmax()`, uncertainty (10).
- Real model backends — HTTP calls, vLLM/OpenAI/Ollama (16).
- AINT-level `mock`/`test` syntax (15).
- Tracing metadata on `Inference<T>` — model, tokens, latency (14).
- Tool calls, effects (11-13).

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
