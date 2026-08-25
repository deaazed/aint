# AINT — Roadmap

This is a compiler project, not a script. Each milestone below should be
treated as a gate: it isn't done until its acceptance criteria pass, and
the next one doesn't start until it is. See `CONTRIBUTING.md` for the
workflow that enforces this.

When a milestone is actively being worked, it gets a folder at
`docs/milestones/NN-name/` with a `SPEC.md` (what this milestone covers
and doesn't), implementation notes as needed, and an `ACCEPTANCE.md`
(the checklist that has to be true before it's marked done). Folders are
created when a milestone starts, not all pre-created up front — milestone
00 and 01 already have theirs as a reference example.

Status legend: `done`, `in progress`, not marked = not started.

## 00 — Language thesis — done

Write down what AINT is and isn't before any code exists, so there's
something to be held to once things are moving fast. Delivered as
`docs/LANGUAGE_DESIGN.md`.

## 01 — Project bootstrap — done

Cargo workspace, crate skeleton, `aint --version` working. Delivered as
this repository's current state.

## 02 — Lexer — done

Tokenize `.an` source: identifiers, literals, keywords, operators,
punctuation. Real error positions from the start —
`file.an:4:17: expected expression`, not `syntax error`.

## 03 — Parser + AST — done

Parse `let`, expressions, `if`, function calls into an AST. Architecture
should tolerate the AI-specific syntax (`infer`, `tool`, `Distribution`)
added later without a rewrite — don't hardcode AI syntax into the lexer
or parser in a way that fights that.

## 04 — Tree-walk interpreter — done

`let`, arithmetic, functions, `return`, recursion (fibonacci-level
programs) running end to end: `aint run examples/hello.an` actually
prints something. No bytecode, no LLVM, no AI yet.

## 05 — Core type system — done

`Int`, `Float`, `Bool`, `String`, `Unit`, then `List<T>`, `Option<T>`.
Static, not dynamic — AI involvement later is not a reason to weaken
this.

## 06 — Modules + standard library

`import math`, `import http`, `import json`. Enough of `io`, `math`,
`collections`, `string`, `json`, `time` to write real (non-AI) programs.
Not a hundred libraries — just enough.

## 07 — Async / concurrency

`async fn` / `await` on Tokio. Needed before inference and tools exist,
since both are inherently asynchronous.

## 08 — First AI primitive

`infer sentiment(text: String) -> Sentiment` and an `Inference<T>` type,
backed by a `Model` trait with a `MockModel` implementation from day
one — AI-touching code must be testable without a live model from this
milestone forward.

## 09 — Typed structured inference

`enum Sentiment { Positive Neutral Negative }` plus
`infer sentiment(text: String) -> Sentiment`, with the runtime generating
a structured-output request and validating the response against the
schema before it becomes an AINT value.

## 10 — Uncertainty

`Distribution<T>` with `probability()`, `argmax()`, `entropy()`,
`sample()`, `require_confidence()`. Decide, explicitly and in writing,
what "probability" means here — see `LANGUAGE_DESIGN.md`.

## 11 — Typed tools

`tool database.get_customer(id: String) -> Customer`. Name, input
schema, output schema, effect, permissions, timeout. Runtime validates
arguments before execution; a model cannot invoke a tool that doesn't
exist.

## 12 — AI tool calling

The model can request a tool call mid-inference; the runtime validates,
executes, and feeds the result back. This is the actual foundation for
agents — not a separate `agent` primitive.

## 13 — Effects

`pure`, `inference`, `tool`, `network`, `filesystem` as declared function
effects, checked by the compiler.

## 14 — AI execution tracing

`Inference #N` / `Tool Call #N` records built into the runtime: model,
tokens, latency, output. Not a library you opt into — part of the
execution model.

## 15 — Deterministic AI testing

`test { mock ... assert ... }` blocks. `aint test` must pass completely
offline.

## 16 — Model adapters

`Model` implementations beyond `Mock`: vLLM, OpenAI-compatible APIs,
Ollama. Source code never names a vendor; deployment config does.

## 17 — AI resource management

`budget { max_tokens max_model_calls max_cost timeout }` enforced by the
runtime as a real resource constraint.

## 18 — Compiler IR (AIR)

Once surface semantics have stabilized: typed AST -> AIR, with explicit
`INFER`, `TOOL_CALL`, `DISTRIBUTION`, `PROBABILITY` operations instead of
generic calls.

## 19 — Optimization

Inference caching, parallel inference, model routing, tool
parallelization, request batching, prompt caching, memoization — now
possible because AIR makes AI operations visible to the compiler.

## 20 — Security model

Permissions, sandboxing, filesystem/network restrictions, tool
authorization, secret management, resource limits. Non-optional once a
model can call tools.

## 21 — Memory model

Decide GC vs. reference counting vs. ownership vs. arena allocation, if
and when the Rust-managed runtime objects from earlier milestones stop
being sufficient. Don't invent this early.

## 22 — Bytecode VM

`AST -> AIR -> Bytecode -> AINT VM`, for startup time, execution speed,
sandboxing, and portability. Still no LLVM.

## 23 — Package manager

`aint init`, `aint add`, `axiom.toml`-equivalent manifest, lockfile,
dependency resolution, registry. Comes after the language itself works,
not before.

## 24 — Language tooling

`aint fmt`, `aint check`, LSP, editor extension, syntax highlighting,
autocomplete, go-to-definition, debugger.

## 25 — Real application

Build something non-trivial entirely in AINT — a customer support system
with an HTTP API, a database, auth, inference, tool calls, background
jobs, logging, and tests. If AINT can't comfortably build this, the
abstractions aren't right yet.

## 26 — Benchmark against the status quo

Compare the milestone-25 application against the equivalent
Python + Pydantic + an LLM SDK + LangGraph stack: lines of code,
latency, memory, failure handling, testability, observability, cost.

## 27 — Find the killer abstraction

Not predetermined. After milestone 26, ask what AINT actually made
dramatically easier — typed inference, uncertainty handling, AI
workflows, model orchestration, or something not yet imagined. That
answer becomes the language's real thesis statement, replacing the
working hypothesis in `LANGUAGE_DESIGN.md` if it turns out to be
different.

## 28 — Production language

Native compilation path, optimized runtime, package ecosystem, stable
specification, backward compatibility policy, security audit,
performance work. This is 1.0.

---

## Known hard problems, by category

Worth keeping visible rather than discovering mid-milestone:

**Compiler:** ambiguous grammar, operator precedence, error recovery,
source-location tracking through every pass, type inference, generics,
recursive types, module cycles.

**Runtime:** environments, stack frames, closures, async execution,
cancellation, concurrency, memory management, error propagation.

**AI:** model nondeterminism, malformed structured output, hallucinated
tool calls, schema validation, retries, timeouts, model failures, model
version drift, probability calibration, token accounting, streaming,
context limits, prompt injection, tool security.

**Language design (the actual research):** What exactly is
`Inference<T>`? What is `Distribution<T>`? Does inference have side
effects? Is it cacheable? Is it deterministic at temperature 0? What does
`==` mean for an AI value? Can an inference result be a loop condition?
How does cancellation work for an in-flight inference? Who controls
model selection — the program or the deployment? What belongs in the
compiler versus the runtime?
