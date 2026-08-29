# AINT — Runtime

**Historical design sketch, written before implementation began —
kept for its structural ideas, not as a syntax reference.** Several
examples below don't match what was actually built: tool names here
use dotted syntax (`database.lookup_customer`), which the language
ended up rejecting entirely (`database_get_email`-style naming
instead); `mock` here shows a rich distribution-literal shape, while
the real `mock` (milestone 15) only accepts literals and
`EnumName_Variant` references; the `model { }` block and deployment
profiles (`fast`/`cheap`/`balanced`/...) were never built — model
selection is `AINT_MODEL_URL`/`AINT_MODEL_NAME` (milestone 25); there
is no `aint trace` subcommand. **`docs/SPECIFICATION.md` (milestone
28) is the accurate, current reference** — read that for how any of
this actually works today.

How inference, tools, tracing, and testing actually work at runtime.
Read `LANGUAGE_DESIGN.md` and `ARCHITECTURE.md` first.

## Typed inference

```
infer classify(message: Text) -> Distribution<Intent>
```

is not a function call that happens to hit an LLM. The compiler knows
the input and output types, so the runtime can request *structured*
output rather than free text and validate it before the program ever
sees a value:

```
Axiom type
     │
     ▼
JSON Schema
     │
     ▼
Model
     │
     ▼
JSON response
     │
     ▼
Schema validator
     │
     ▼
Distribution<Intent>
```

A model is never allowed to hand back arbitrary text where the program
expects a typed value.

## Distribution<T> at runtime

```rust
struct Distribution<T> {
    values: Vec<(T, f64)>,
}

struct Inference<T> {
    output: Distribution<T>,
    model: ModelId,
    tokens: Usage,
    latency: Duration,
    trace: TraceId,
}
```

Every inference result carries its own provenance (which model, how many
tokens, how long it took) rather than that information living only in a
side-channel log line somewhere.

## Tools

```
tool database.lookup_customer(id: String) -> Customer
```

Tools carry a name, an input schema, an output schema, an effect, and
optionally permissions and a timeout. The runtime validates arguments
*before* execution — a model cannot invent a call to a tool that doesn't
exist, and cannot call a real tool with arguments that don't type-check.

When a model decides mid-inference that it needs a tool:

```
question
   │
   ▼
inference
   │
   ├── final answer
   │
   └── tool call request
          │
          ▼
        validate against tool signature
          │
          ▼
        execute
          │
          ▼
        result fed back into inference
```

Tool execution always happens outside the model. The model can request a
call; it never performs one directly.

## Tracing is a runtime primitive, not a bolt-on

```
Inference #17
Model: mistral-small
Input tokens: 143
Output tokens: 31
Latency: 382ms
Output:
    refund: 0.93
    support: 0.04
    other: 0.03

Tool Call #18
database.get_customer
input: { id: "123" }
output: { premium: true }
latency: 12ms
```

Because the runtime knows what an inference or a tool call *is* (rather
than treating them as opaque function calls), this trace comes from the
execution model itself. `aint trace program.an` should not require the
program to have been instrumented by hand.

## Testing AI code deterministically

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

`aint test` runs entirely offline against `MockModel`. No test in the
suite should ever require a live model call to pass — that would make the
suite nondeterministic and expensive, which defeats the point of having
one.

## Model deployment is configuration, not code

```
model classifier {
    input  Text
    output Intent
}
```

```toml
[classifier]
provider = "vllm"
model = "mistral-small"
endpoint = "http://localhost:8000"
```

versus, with no source change:

```toml
[classifier]
provider = "openai"
model = "..."
```

## Inference location is not part of program semantics

```
infer classify(message: Text) -> Distribution<Intent>
```

means the same thing whether it runs on a developer's laptop with no
GPU, a self-hosted vLLM cluster, or a hosted provider. Developers should
not need to own a GPU any more than a Python developer needs to own the
Postgres server their code connects to. Performance, cost, and model
quality differ by deployment; the program's meaning does not.

Deployment profiles are how that gets tuned without touching source:

```
fast       — small/cheap model, low latency
cheap      — optimize for cost
balanced   — default tradeoff
accurate   — largest available model
private    — on-prem / self-hosted only
local      — developer's own machine, no network
```

## Resource budgets are enforced by the runtime

```
budget {
    max_tokens = 5000
    max_model_calls = 3
    max_cost = 0.02
    timeout = 10s
}
```

The runtime enforces these the same way it would enforce a stack limit —
this is a runtime resource, not an external thing you monitor after the
fact in a dashboard.

## Non-goals for the runtime, for now

- No automatic multi-model routing/escalation logic until the basic
  single-model path is solid (this is a later research milestone, not a
  v0.1 feature).
- No hosted "AINT Cloud" execution path in the open-source runtime. If a
  hosted product ever exists, it is an *optional* implementation of the
  same `Model` interface everything else uses — never a requirement to
  run an AINT program.
