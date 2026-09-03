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

## 06 — Modules + standard library — done

`import math`, `import http`, `import json`. Enough of `io`, `math`,
`collections`, `string`, `json`, `time` to write real (non-AI) programs.
Not a hundred libraries — just enough.

## 07 — Async / concurrency — done

`async fn` / `await` on Tokio. Needed before inference and tools exist,
since both are inherently asynchronous.

## 08 — First AI primitive — done

`infer sentiment(text: String) -> Sentiment` and an `Inference<T>` type,
backed by a `Model` trait with a `MockModel` implementation from day
one — AI-touching code must be testable without a live model from this
milestone forward.

## 09 — Typed structured inference — done

`enum Sentiment { Positive Neutral Negative }` plus
`infer sentiment(text: String) -> Sentiment`, with the runtime generating
a structured-output request and validating the response against the
schema before it becomes an AINT value.

## 10 — Uncertainty — done

`Distribution<T>` with `probability()`, `argmax()`, `entropy()`,
`sample()`, `require_confidence()`. Decide, explicitly and in writing,
what "probability" means here — see `LANGUAGE_DESIGN.md`.

## 11 — Typed tools — done

`tool database.get_customer(id: String) -> Customer`. Name, input
schema, output schema, effect, permissions, timeout. Runtime validates
arguments before execution; a model cannot invoke a tool that doesn't
exist.

## 12 — AI tool calling — done

The model can request a tool call mid-inference; the runtime validates,
executes, and feeds the result back. This is the actual foundation for
agents — not a separate `agent` primitive.

## 13 — Effects — done

`pure`, `inference`, `tool`, `network`, `filesystem` as declared function
effects, checked by the compiler.

## 14 — AI execution tracing — done

`Inference #N` / `Tool Call #N` records built into the runtime: model,
tokens, latency, output. Not a library you opt into — part of the
execution model.

## 15 — Deterministic AI testing — done

`test { mock ... assert ... }` blocks. `aint test` must pass completely
offline.

## 16 — Model adapters — done

`Model` implementations beyond `Mock`: vLLM, OpenAI-compatible APIs,
Ollama. Source code never names a vendor; deployment config does.

## 17 — AI resource management — done

`budget { max_tokens max_model_calls max_cost timeout }` enforced by the
runtime as a real resource constraint.

## 18 — Compiler IR (AIR) — done

Once surface semantics have stabilized: typed AST -> AIR, with explicit
`INFER`, `TOOL_CALL`, `DISTRIBUTION`, `PROBABILITY` operations instead of
generic calls.

## 19 — Optimization — done

Inference caching, parallel inference, model routing, tool
parallelization, request batching, prompt caching, memoization — now
possible because AIR makes AI operations visible to the compiler.

## 20 — Security model — done

Permissions, sandboxing, filesystem/network restrictions, tool
authorization, secret management, resource limits. Non-optional once a
model can call tools.

## 21 — Memory model — done

Decide GC vs. reference counting vs. ownership vs. arena allocation, if
and when the Rust-managed runtime objects from earlier milestones stop
being sufficient. Don't invent this early.

## 22 — Bytecode VM — done

`AST -> AIR -> Bytecode -> AINT VM`, for startup time, execution speed,
sandboxing, and portability. Still no LLVM.

## 23 — Package manager — done

`aint init`, `aint add`, `axiom.toml`-equivalent manifest, lockfile,
dependency resolution, registry. Comes after the language itself works,
not before.

## 24 — Language tooling — done

`aint fmt`, `aint check`, LSP, editor extension, syntax highlighting,
autocomplete, go-to-definition, debugger.

## 25 — Real application — done

Build something non-trivial entirely in AINT — a customer support system
with an HTTP API, a database, auth, inference, tool calls, background
jobs, logging, and tests. If AINT can't comfortably build this, the
abstractions aren't right yet.

## 26 — Benchmark against the status quo — done

Compare the milestone-25 application against the equivalent
Python + Pydantic + an LLM SDK + LangGraph stack: lines of code,
latency, memory, failure handling, testability, observability, cost.

## 27 — Find the killer abstraction — done

Not predetermined. After milestone 26, ask what AINT actually made
dramatically easier — typed inference, uncertainty handling, AI
workflows, model orchestration, or something not yet imagined. That
answer becomes the language's real thesis statement, replacing the
working hypothesis in `LANGUAGE_DESIGN.md` if it turns out to be
different.

## 28 — Production language — done

Native compilation path, optimized runtime, package ecosystem, stable
specification, backward compatibility policy, security audit,
performance work. This is 1.0.

---

## Phase 2 — beyond 1.0

1.0 (milestones 0–28) proved the governance thesis on a single-file
application. What it didn't prove: that AINT is a comfortable language
to build a real, multi-file, growing project in — the thing a framework
or a genuine web application actually needs. Phase 2 targets that
directly: modularity, passing behavior around as a value, a web layer
that doesn't fight the language, and AI-assisted scaffolding, in that
order, each still gated the same way 0–28 were.

## 29 — Modularity — done

`import "path" as alias`: one AINT program spanning more than one file,
for the first time. A new `aint-loader` crate resolves the whole import
graph into one flat program before the type checker, interpreter, IR
compiler, or VM ever see it — none of those four crates change in any
way that matters. See `docs/milestones/29-modularity/SPEC.md`.

## 30 — Closures — done

Functions as values — passed as arguments, returned, stored in a
`List<T>`. The minimum lever needed to express strategy/observer/
dependency-injection-style patterns without generics or structs, which
stay out of scope. Interpreter-only; the bytecode VM and IR compiler
reject a closure explicitly rather than miscompiling one. See
`docs/milestones/30-closures/SPEC.md`.

## 31 — Web framework ergonomics — done

A route table, built entirely in AINT source on top of 29 and 30 — no
new framework-shaped stdlib surface, just one new primitive
(`string_split`) and a real, importable library
(`examples/router/router.an`) replacing the hand-nested `if`/`else`
pyramid every HTTP example had before this. See
`docs/milestones/31-web-ergonomics/SPEC.md`.

## 32 — AI-assisted scaffolding — done

`aint scaffold "description" <path>`: dogfoods AINT's own
model-adapter machinery (a new `ChatClient`, alongside `HttpModel`) to
generate a starter project from a plain-English description, always
run through the same check `aint check` uses before being reported as
done. See `docs/milestones/32-ai-scaffolding/SPEC.md`.

## 33 — Rebuild the language's own website — done

`examples/website/` rebuilt on 29–31 — one 635-line file with a
7-level-deep nested `if`/`else` router became nine files and a flat
route table, verified live the same way the original was. See
`docs/milestones/33-website-rebuild/SPEC.md`.

## 34 — Real tool execution — done

Not part of the original Phase 2 plan — added after confirming
`MockTool` was still the only tool executor that has ever existed,
undermining the language's own governance pitch. `tool name(params) ->
Type { body }` now runs for real, whether called directly or requested
by a model, with an explicit `mock` always taking precedence. See
`docs/milestones/34-real-tools/SPEC.md`.

## 35 — Installer — done

Getting `aint` has meant `cargo build` since the project existed. A
release workflow now builds real binaries on a tagged push, and
`install.sh`/`install.ps1` fetch them — no cargo, no new hosting,
everything served by GitHub. See
`docs/milestones/35-installer/SPEC.md`.

## 36 — Package dependencies over git — done

`aint add`'s local-path-only limitation, resolved the same way Go
modules do it: a dependency can name a git URL, no hosted registry
required. Also closes a gap named in both milestone 23's and milestone
29's own specs: `aint-package` and `aint-loader` had been disconnected
the entire time — a resolved dependency was never actually
`import`-able. A bare-name `import "name" as alias` now resolves
through `aint.lock` to that package's `lib.an`. See
`docs/milestones/36-git-dependencies/SPEC.md`.

## Phase 3 — ergonomics, from dogfooding

Phase 2 (29–36) made AINT capable of real, multi-file, dependency-
having programs. Building a full website on top of that — first inside
this repo as `examples/website/`, then again for real as its own
standalone `aint-website` project — didn't find a capability gap. It
found that every page was still hand-nested `string_concat` chains,
because there is no way to compute one of two values and return it
once. Phase 3 fixes what that dogfooding actually cost, in the order it
actually cost time, not by guessing. (One real bug came out of the same
effort too — `HttpModel` never told a model what an enum's variants
actually were, so live structured-output classification failed outright
against Mistral. That was a correctness bug, not an ergonomics gap, and
is already fixed — see the `[F]` commit bumping to 0.1.1, not a Phase 3
milestone.)

## 37 — Conditional expressions — done

`if`/`else` becomes usable as an expression, not just a statement:
`let x = if cond { a } else { b }`, both branches required and
type-matched when used this way — statement-position `if` (`else`
still optional) is unchanged. `else if` comes along as sugar for
`else { if ... }`, since it's nearly free once the grammar supports
this. The single highest-leverage fix `aint-website` found: a
four-variant label function needed three levels of nested `if`/`else`,
and a two-branch page handler had to duplicate its entire page-wrapping
call in both branches rather than compute a value once. Interpreter-only
— the bytecode VM rejects the expression form explicitly, same shape
as closures. See
`docs/milestones/37-conditional-expressions/SPEC.md`.

## 38 — Missing comparison and logical operators — done

`<=`, `>=`, `!`, `&&`, `||` — reached for by reflex, absent since day
one. `&&`/`||` genuinely short-circuit — the right operand isn't
evaluated at all once the left side already decides the result, proven
with tests that would crash or print unexpectedly if it didn't.
Directly hit writing `aint-website`'s own HTML-escaping helper: a
boundary check that should have been `index >= length - 1` had to be
inverted, and its branches swapped, to work with only `<`. `<=`/`>=`/`!`
run under the bytecode VM with no parity gap; `&&`/`||` are rejected
there explicitly (short-circuiting would need real conditional-jump
bytecode), the same shape as closures and if-expressions. See
`docs/milestones/38-comparison-and-logical-operators/SPEC.md`.

## 39 — String stdlib: replace — done

`string_replace(s, target, replacement) -> String`, native rather than
something every program hand-rolls from `string_split` plus a
recursive join — needed for `aint-website`'s `escape_html`, the one
place that site puts real user input back into HTML. No VM parity gap
— a plain native call, resolved through the same shared table the
tree-walker and bytecode VM already both use. See
`docs/milestones/39-string-replace/SPEC.md`.

## 40 — URL/query percent-decoding

`router_query_param` (and `http` generally) returns a query value
exactly as it arrived on the wire — `%20`/`%3C`/etc. never get decoded,
and the stdlib has no hex or char-code primitives an AINT program could
use to write a decoder itself. Found testing `aint-website`'s `/try`
page: a message typed with an apostrophe or a space came back through
as literal percent-escapes. Scope this milestone's `SPEC.md` to decide
whether the fix is a native `string_url_decode`, decoding inside
`router_query_param` itself, or both.

## 41 — `aint run`/`aint test` load `.env` automatically

`aint` has never read a `.env` file — every real model call needs the
caller to export `AINT_MODEL_URL`/`AINT_MODEL_NAME`/`AINT_MODEL_API_KEY`
by hand first, which is why `aint-website` needed its own `run.ps1`
just to start the site with real credentials. Smaller than 37–40, and
arguably CLI ergonomics more than language design, but it's exactly the
kind of first-run friction that decides whether someone gets a live
demo working at all — worth doing while this phase is already looking
at what dogfooding actually cost.

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
