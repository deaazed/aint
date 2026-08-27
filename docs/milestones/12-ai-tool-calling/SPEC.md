# Milestone 12 — AI tool calling

## Scope

`ROADMAP.md`:

> The model can request a tool call mid-inference; the runtime
> validates, executes, and feeds the result back. This is the actual
> foundation for agents — not a separate `agent` primitive.

This is where `infer` (08-10) and `tool` (11) actually connect. Before
this milestone, they were parallel, independent features that happened
to share a testing pattern. Now an `infer` call can, instead of
answering directly, ask the runtime to run a declared `tool` and hand
the result back — repeatedly, until it's ready to answer.

## Design decisions

**`Model::infer` returns an `InferenceOutcome`, not a bare `Value`,
now.** This is the one real breaking change this milestone makes to
already-shipped code (`Model`, `MockModel`, `InferenceRequest` —
milestones 08-10). It's unavoidable: a model has to be able to say
"call this tool" instead of "here's your answer," and there's no
honest way to express that inside a plain `Value`.

```
enum InferenceOutcome {
    Answer(Value),
    CallTool { tool: String, args: Vec<Value> },
}
```

**The interpreter drives a loop, not a single call.** `eval_inference`
now calls `self.model.infer(...)`; if the outcome is `Answer`, it
validates and returns exactly as before (milestones 09-10's schema
validation, untouched). If it's `CallTool`, the interpreter:

1. Looks up the tool by name in a runtime registry populated when
   `StmtKind::Tool` executes (`self.tools_registry`, the same shape as
   `self.enums` — a flat, non-lexically-scoped table, since tools are
   only ever declared at the top level in practice).
2. Validates argument **count and type** against the tool's declared
   signature. This is new: every argument validation so far has been
   the static type checker's job, because every call site was AINT
   source the checker could see. A model's tool-call request is a
   runtime string and a `Vec<Value>` the checker never saw — this is
   the "runtime validates arguments before execution" line from
   milestone 11's own roadmap text, which that milestone deliberately
   deferred here as the first case where it's actually meaningful.
3. Executes the tool via `MockTool`, validates the *result* against
   the tool's declared return type (reusing `validate_inference_result`
   again — still nothing model-specific in it).
4. Appends `{tool, args, result}` to a running `history`, and calls
   `self.model.infer(...)` again — this time with that history
   attached, so the model has the tool's answer to work from.

This repeats until the model answers. There's no cap on how many times
it can loop. See "explicitly out of scope" below for why that's a
deliberate omission, not an oversight.

**`InferenceRequest` grows two fields: `available_tools` and
`history`.** A model needs to know what it's allowed to call
(`available_tools: Vec<ToolSignature>`, derived from the same runtime
registry) and, on any call after the first, what's already happened in
this conversation (`history: Vec<ToolExchange>`). `MockModel` ignores
both — same as it already ignores `return_type`'s structured-output
framing — but the shape now exists for a real adapter (milestone 16)
to actually use.

**`MockModel` is scriptable, not just single-shot, and this had to
happen without breaking any of the 20-odd existing tests that call
`.mock(name, value)`.** Internally, `MockModel` now stores a queue of
`InferenceOutcome`s per function name (`scripts: HashMap<String,
RefCell<VecDeque<InferenceOutcome>>>`), popped one at a time on each
call to `infer`. `.mock(name, value)` becomes sugar for `.script(name,
vec![InferenceOutcome::Answer(value)])` — a length-one queue, popped
once. Every existing test calls its mocked function exactly once, so
this is behaviorally identical to the old "always return this value"
table it replaces. New tests use `.script(name, outcomes)` directly to
script a multi-step tool-calling conversation (`CallTool`, then
`Answer`, or longer).

**`ToolFn.params` changes from `Vec<String>` to `Vec<Type>`.** Param
*names* were always vestigial for `tool` (and `infer`) — there's no
body to bind them into, only an arity count was ever read. Now that
model-requested arguments need their *types* checked at runtime, the
types are what the interpreter actually needs to keep. This is a
narrow, directly-motivated change, not a speculative one.

## Explicitly out of scope

- Any cap on tool-calling loop iterations, token budget, or call
  count. Milestone 17 ("AI resource management") is explicitly where
  `budget { max_model_calls ... }` belongs; adding an ad-hoc limit here
  would mean redesigning it there. Today's only implementation
  (`MockModel`) can't loop forever by accident — a test would have to
  deliberately script an unterminated sequence — so there's no real
  hazard yet to guard against.
- Tools calling other tools. Only a model (`infer`) initiates tool
  calls; a `tool` itself stays a plain, non-recursive external
  capability.
- Effects, permissions (13, 20) — a tool call isn't checked against
  any policy yet beyond "does this tool exist and do the arguments
  match its schema."
- An `examples/*.an`, for the same reason as every AI-touching
  milestone since 08: no AINT-level way to configure `MockModel`/
  `MockTool` yet. This milestone's payoff is a genuinely convincing
  *test*, though — a scripted multi-step tool-calling exchange is
  about as close to demonstrating "the foundation for agents" as
  something can get without a real model in the loop.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
