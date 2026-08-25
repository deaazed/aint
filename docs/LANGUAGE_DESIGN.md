# AINT — Language Design

This is the reference document for what AINT is, what it deliberately is
not, and the principles every later decision has to answer to. Read this
before implementing any milestone. If a milestone's implementation seems
to contradict this document, the document wins — raise it and fix the
plan before writing code.

## The thesis

Traditional programming languages give you exactly one kind of value:
one you know for certain. `x = 10` means `x` is 10, full stop. AI
systems don't work that way — a classifier doesn't know your customer
wants a refund, it's 94% confident they do. Every language currently
bolts that uncertainty on top as an API response you parse by hand.

AINT's bet is that this is a language-design gap, not just tooling debt:

> **Deterministic computation and probabilistic inference should be
> equally fundamental to the language — not one implemented on top of
> the other.**

Concretely, that means:

- Calling a model is a real language construct (`infer`), not a library
  function that happens to make a network request.
- The result of an inference is explicitly typed as uncertain
  (`Distribution<T>`), and the compiler will not let you treat it as a
  plain `T` without an explicit decision about how you're resolving the
  uncertainty.
- Tools an AI can call are typed and validated the same way regular
  function arguments are — a model cannot invoke a capability that
  doesn't exist or pass it malformed input.
- AI behavior is testable offline and deterministically, the same way
  ordinary functions are. A test suite that requires live model calls to
  pass is a bug in the language, not an acceptable cost of doing AI.

## What AINT is not

Stated explicitly because it's easy to drift toward the path of least
resistance mid-milestone without a clear line to check against:

- **Not** a Python transpiler.
- **Not** a prompt-templating DSL.
- **Not** a LangChain/LangGraph replacement bolted onto a scripting
  language.
- **Not** a YAML/config-driven workflow engine wearing a syntax.
- **Not** an agent-only language — agents should fall out of
  `infer` + `tool` + state + control flow, not be a privileged primitive.
- **Not** an LLM SDK with nicer syntax sugar around `chat.completions`.
- **Not** married to today's vendor APIs. If the pitch only makes sense
  while "LLM" means "OpenAI-shaped chat API," the design is wrong.

If a proposed feature is easier to justify by one of the bullets above
than by the thesis, don't build it yet.

## The four kinds of computation

```
deterministic computation      — functions, data transforms, control flow
inference                      — model calls, classification, generation
tools                          — database calls, HTTP, filesystem, APIs
uncertainty                    — Distribution<T>, probability, confidence
```

The important invariant: these are never silently interchangeable.

```
String       ≠  Inferred<String>
Intent       ≠  Distribution<Intent>
ToolResult   ≠  Inference<ToolResult>
```

Crossing from an uncertain value to a certain one is always a decision
the program makes explicitly (`.probability(x)`, `.argmax()`,
`.require_confidence(p)`), never an implicit coercion.

## Sketch of the type system

```
Primitive
    String, Int, Float, Bool, Bytes, Unit

AI-native
    Distribution<T>          — a probability distribution over T
    Inference<T>              — an in-flight or completed inference producing T
    Embedding

Infrastructure
    Tool<TInput, TOutput>
    Model<TInput, TOutput>

Collections
    List<T>, Map<K, V>, Option<T>
```

`Distribution<T>` is expected to carry at least:

```
probability(value: T) -> Float
argmax() -> T
entropy() -> Float
sample() -> T
require_confidence(threshold: Float) -> Option<T>
```

What "probability" *means* is one of the open research questions this
project has to answer honestly rather than assume: model token
probability, calibrated confidence, normalized model scores, and
empirical probability are not the same thing, and picking one silently
is a design bug, not a shortcut.

## Effects

A small, explicit effect system so the compiler (and a reader) knows
what a function can do without reading its body:

```
pure          — no side effects
inference     — calls a model
tool          — calls a typed tool
network       — arbitrary network access
filesystem    — arbitrary filesystem access
```

```
pure fn calculate_total(order: Order) -> Money { ... }

fn get_customer(id: String) -> Customer
    effects [tool]

infer classify(text: String) -> Distribution<Intent>
    effects [inference]
```

A `pure` function cannot call something with a wider effect. This is
what makes it possible to reason about what a piece of business logic
is actually allowed to do, even once AI is involved.

## Runtime resources are language-level constraints, not afterthoughts

```
budget {
    max_tokens = 5000
    max_model_calls = 3
    max_cost = 0.02
    timeout = 10s
}
```

Cost, latency, token usage, and tool-call count are treated as runtime
resources the same way memory or time are in other languages — not
something you discover after the fact in a billing dashboard.

## Models are a deployment detail, not a language detail

```
infer classify(message: Text) -> Distribution<Intent>
```

never names a vendor. Which model runs it, and where, is deployment
configuration:

```toml
[classifier]
provider = "vllm"
model = "mistral-small"
endpoint = "http://localhost:8000"
```

This should hold whether inference runs on a developer's laptop with no
GPU, a self-hosted vLLM cluster, or a hosted API. Inference location is
not part of program semantics — the same principle SQL applies to where
a database physically lives.

## Testability is not optional

```
test "refund classification" {
    mock classify {
        refund: 0.96
        support: 0.02
        other: 0.02
    }

    assert respond("I want my money back", "123")
        == "I'll help you process your refund."
}
```

`aint test` must be able to run completely offline, with zero live model
calls, and get a deterministic result. If a change makes that harder, the
change is wrong, not the constraint.

## Where agents fit

Agents are not a milestone-one concept. An agent is what you get when you
combine inference, tools, state, and control flow — the language should
make building one out of those primitives natural, rather than defining
`agent` as a magic top-level construct that hides how it works. Whether
`agent` ever becomes its own keyword is a decision to make after real
agents have been built in the language and a pattern actually repeats.

## Design principles for implementation

1. Strong static typing. AI involvement is not an excuse to go dynamic.
2. Uncertainty is explicit, always, everywhere.
3. AI operations are represented in the AST and type system directly —
   never smuggled through a generic function-call node.
4. Tool calls are typed and validated before execution.
5. AI behavior must be testable with deterministic mocks.
6. Runtime effects must be observable (tracing is a primitive, not a
   bolt-on library).
7. Never hide inference behind an ordinary-looking function call.
8. Prefer simple semantics over a clever implementation.
9. No language feature lands without tests.
10. Established semantics don't change silently — a breaking change to
    how something already works is a decision, documented as one.

## How to know if this is working

The honest test, revisited at milestone 25-27: build something real
(a customer support system with an HTTP API, a database, auth,
inference, tool calls, background jobs, and tests) entirely in AINT, and
compare it against the equivalent Python + Pydantic + an LLM SDK +
LangGraph stack on lines of code, latency, testability, and how it fails.
If AINT isn't clearly better on that comparison, the language hasn't
found its abstraction yet, and milestone 27 exists to go find it rather
than to declare victory on a predetermined answer.
