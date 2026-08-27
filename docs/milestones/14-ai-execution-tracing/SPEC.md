# Milestone 14 — AI execution tracing

## Scope

`ROADMAP.md`:

> `Inference #N` / `Tool Call #N` records built into the runtime:
> model, tokens, latency, output. Not a library you opt into — part of
> the execution model.

Every `infer` call and every `tool` call — whether invoked directly
from AINT source or requested by a model mid-inference (milestone 12)
— now produces a trace record automatically: which one it was, what it
was asked, how long it took, and what came back (including if it
failed). No `import tracing`, no wrapper function to remember to call.

## Design decisions

**Tracing wraps the actual call, unconditionally, at the one or two
places calls actually happen.** `eval_inference` is the only place
`self.model.infer(...)` is ever called; a single new
`call_tool_traced` helper is the only place `self.tools.call(...)` is
ever called, used by both the direct-call path (`eval_tool_call`,
milestone 11) and the model-requested path (`call_requested_tool`,
milestone 12). There's no separate "tracing layer" to remember to wire
up somewhere else — it's inline in the two functions that already own
every model/tool interaction, which is what "part of the execution
model" means concretely.

**A trace record is created on failure too, not just success.**
Latency and outcome (including the error) are recorded regardless of
whether the call succeeded — a trace that only existed for successful
calls would hide exactly the cases tracing matters most for.

**Separate, independent counters for `Inference #N` and `Tool Call
#N`**, exactly as the roadmap writes them — not one shared sequence.
Each starts at 1.

**Every model round trip in a tool-calling conversation gets its own
`Inference` record.** `eval_inference`'s loop (milestone 12) may call
`self.model.infer(...)` several times before the model answers; each
call is a distinct, separately-numbered trace entry, not folded into
one record for the whole conversation. This is what makes the trace
log actually useful for a multi-step agentic call — you can see the
model asked for a tool, got it, and asked again.

**Latency is real, measured wall-clock time around the call** —
`Instant::now()` before, `.elapsed()` after — not a placeholder. For
`MockModel`/`MockTool` this is necessarily tiny (there's no real
network round trip yet), but it's a genuine measurement, not a
hardcoded value, and the mechanism is identical to what a real model
adapter (milestone 16) will produce.

**Token usage is a real, typed field (`TokenUsage { prompt,
completion }`), always `{0, 0}` today.** `MockModel` has no tokens to
report — there's no text being tokenized. Rather than omit the field
(which would mean adding it later, breaking every trace consumer) or
fake a number, the field exists now with an honestly-zero value,
documented as a placeholder for milestone 16.

**The model identifier is hardcoded to `"mock"` for now, not a new
`Model` trait method.** Only one `Model` implementation exists.
Adding `Model::name(&self) -> &str` to the trait for a single
implementation to answer "mock" would be exactly the kind of
speculative generality this project avoids elsewhere — it's easy to
add when milestone 16 actually needs to distinguish real backends from
each other.

**Traces are queryable (`Interpreter::traces() -> Vec<TraceRecord>`),
not printed anywhere by default.** "Not a library you opt into" is
about *capture*, not about the CLI making every run of every program
noisier by default — nothing currently consumes a trace log
(`aint run` has no flag for it), so adding unrequested output would be
UX design without a UX need behind it yet. The mechanism exists and is
fully tested directly; presenting it is tooling territory (milestone
24) once there's an actual reason to.

## Explicitly out of scope

- Real token counts (16).
- A `Model` trait method for backend identification (deferred until
  there's more than one backend to distinguish).
- CLI output/formatting for trace logs (24, or whenever something
  needs to consume them).
- Persisting traces anywhere, or exposing them to AINT source itself —
  they're a Rust-level (and future-tooling-level) concern for now, the
  same way `RuntimeError` is.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
