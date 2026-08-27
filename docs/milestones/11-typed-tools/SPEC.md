# Milestone 11 — Typed tools

## Scope

`ROADMAP.md`:

> `tool database.get_customer(id: String) -> Customer`. Name, input
> schema, output schema, effect, permissions, timeout. Runtime
> validates arguments before execution; a model cannot invoke a tool
> that doesn't exist.

Taken piece by piece against what's actually buildable right now:

- **Name, input schema, output schema** — this milestone's real scope.
- **`Customer`** — a struct/record return type. AINT has no
  struct/record types (only `enum`, since milestone 09); nothing in the
  roadmap has asked for them yet either. Examples and tests use
  existing types instead (`Int`/`Float`/`Bool`/`String`/`enum`/`List`/
  `Option`) — a tool returning `String` is exactly as real a
  demonstration as one returning a hypothetical `Customer`.
- **effect** — milestone 13 names the effect system explicitly
  (`pure`/`inference`/`tool`/`network`/`filesystem`) and is where it
  belongs. Not parsed here.
- **permissions** — milestone 20 ("Security model").
- **timeout** — milestone 17 ("AI resource management",
  `budget { ... timeout }`).
- **"a model cannot invoke a tool that doesn't exist"** — this is
  milestone 12's own line almost verbatim ("the model can request a
  tool call mid-inference"). There is no mechanism for a model to
  request anything yet — that's what 12 builds. What this milestone
  delivers instead: the *only* way AINT source can reference a tool
  today is by name, through the same identifier-resolution path every
  other call goes through, so an undeclared tool name is already a
  compile-time `UndefinedFunction`, not a runtime surprise. Milestone
  12 is where a *model's own* tool-call request gets the equivalent
  guarantee dynamically, against arbitrary runtime strings a type
  checker never saw.

## Design decisions

**`tool` mirrors `infer`'s shape exactly, as a separate declaration —
not a generalization of it.** Same reasoning as keeping `Inference<T>`
a distinct type from `Task<T>`: they're structurally identical today,
but tool-specific metadata (permissions, timeout, effect audit) and
inference-specific metadata (tokens, latency, tracing) will diverge
starting with milestones already on the roadmap. Merging them now to
avoid the duplication would mean un-merging them later, and would
touch `StmtKind::Infer` — closed, shipped, tested code from milestone
08 — for a milestone that doesn't need to change it. New
`StmtKind::Tool { name, params, return_type }`, new `Type::Tool<T>`,
new `Value::ToolFn`/`Value::ToolCall`, same shapes as their `infer`
counterparts.

**No dotted tool names.** `database.get_customer` is roadmap shorthand,
not new syntax — AINT has never had dotted access, in any milestone
(`string_length`, `Sentiment_Positive`). A tool is just named
`database_get_customer` in source, exactly like any other declaration;
the "module-scoped" reading is a naming convention, not a parser
feature. Since this needs no new lexing/parsing beyond what `infer`
already has, `parse_tool_statement` is a straight copy of
`parse_infer_statement`'s shape with a different keyword.

**Tools don't need a `Tool` trait — only `Model` did.** `Model` is
generic (`Interpreter<W, M: Model>`) because milestone 16 swaps in
*one* real implementation in place of `MockModel` for an entire
program. Tools don't work that way: a program can declare many tools,
each independently backed, and nothing on the roadmap asks for a
single swappable "the tool backend" the way there's a single model per
deployment. So `MockTool` is a concrete (non-generic) struct on
`Interpreter` — a name-keyed table, structurally identical to
`MockModel`'s, with its own constructor
(`Interpreter::with_output_model_and_tools`) and its own error variant
(`RuntimeError::ToolError`, distinct from `ModelError` — different
external system, different failure to name honestly). No `Interpreter`
generic-parameter growth.

**Tool calls are lazy and `await`-able, exactly like `infer`.**
Milestone 07 built the async foundation specifically because
"inference and tools ... are inherently asynchronous" — this is that
foundation's second, equally-motivated user. `await` now accepts
`Task<T>`, `Inference<T>`, or `Tool<T>`.

**No `effect`/`permissions`/`timeout` syntax yet, not even
unenforced.** Guessing at that syntax now risks designing it wrong
before the milestones that actually need it (13, 17, 20) have decided
what it has to express. A `tool` declaration today is exactly as
minimal as `infer` was at milestone 08: name, typed signature, no
body, nothing else.

## Explicitly out of scope

- Struct/record return types (`Customer`).
- `effect`/`permissions`/`timeout` declaration syntax.
- A model requesting a tool call at runtime by name (milestone 12) —
  this milestone only makes *statically-referenced* tool calls from
  AINT source real.
- Real tool backends — an actual database, HTTP client, filesystem
  access. `MockTool` remains the only implementation, same testing
  story as `MockModel` since milestone 08.
- An `examples/*.an` exercising a tool call end to end — same
  reasoning as `infer` in milestones 08-10: no AINT-level way to
  configure `MockTool` yet (that's milestone 15's job, alongside
  `infer` mocking), so `aint run` still can't get a real answer from a
  declared tool. Verified instead by runtime tests configuring
  `MockTool` directly, plus a CLI test proving the honest failure mode
  end to end, matching milestone 08's pattern exactly.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
