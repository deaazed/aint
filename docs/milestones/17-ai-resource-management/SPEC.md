# Milestone 17 — AI resource management

## Scope

`ROADMAP.md`:

> `budget { max_tokens max_model_calls max_cost timeout }` enforced by
> the runtime as a real resource constraint.

This is the payoff for a gap this project has named explicitly, twice,
and deferred on purpose: milestone 12's `eval_inference` loop has "no
cap on iterations... by design" with a note that "milestone 17 is
explicitly where `budget { max_model_calls ... }` belongs." This is
that milestone.

## What's actually enforceable today, and what isn't

Two of the four fields are honestly, meaningfully enforceable right
now: **`max_model_calls`** (every `Inference #N` the tracing
infrastructure from milestone 14 already counts) and **`timeout_ms`**
(a real wall-clock deadline around the inference conversation,
`tokio::time::timeout`).

The other two are not, and this milestone says so rather than faking
it: **`max_tokens`** and **`max_cost`** both depend on real token
counts, which — as milestone 14 documented — are `{0, 0}` for every
call today (`MockModel` has nothing to tokenize, and `HttpModel`
doesn't parse the `usage` field a real OpenAI-compatible response
actually carries; that's a real, separate piece of work, not done
here). Both fields parse, both are tracked, both are *checked* against
running totals with real, tested comparison logic — but with every
call reporting zero tokens, neither can actually fire against a real
`infer` call yet. This is stated as a limitation, not silently
shipped as if it were complete.

## Design decisions

**`budget { ... }` is a single, program-wide declaration** — not
scoped to one function or one `infer` call. `LANGUAGE_DESIGN.md`'s
sketch doesn't attach it to anything, and scoping it to individual
call sites would need new expression syntax (`await x() with budget
{...}`) that nothing else in the roadmap asks for. A program has at
most one `budget` block; a second one is a type error, not
last-write-wins — silently ignoring a duplicate declaration would be
worse than refusing to compile it.

**All four fields are optional; an omitted field means unlimited for
that dimension.** A `budget { max_model_calls = 3 }` block constrains
only that one axis.

**`timeout` becomes `timeout_ms`, an integer, not a duration
literal.** `LANGUAGE_DESIGN.md` writes `timeout = 10s`, which would
need a new lexical form (numbers with unit suffixes) that nothing else
in AINT's grammar has. `time_sleep_ms` (milestone 06) already
established plain-integer-milliseconds as this language's convention
for durations; `timeout_ms` matches it instead of inventing a second
one.

**Enforcement lives entirely in `eval_inference`.** `max_model_calls`
is checked immediately before each `self.model.infer(...)` call in the
tool-calling loop — the same call site tracing already wraps — and the
whole loop is wrapped in `tokio::time::timeout(...)` for `timeout_ms`.
Exceeding either produces `RuntimeError::BudgetExceeded`, distinct
from `ModelError`/`SchemaViolation`: this isn't the model failing to
answer or answering wrong, it's the runtime refusing to let the
conversation continue.

**No budget block means no enforcement — this is opt-in, the same
shape as `effects` (13).** Every program without a `budget` block
(every existing example, every existing test) keeps behaving exactly
as before.

## Explicitly out of scope

- Real token counting from `HttpModel` (a real, separate piece of
  work: parsing the `usage` field from an OpenAI-compatible response).
- Real cost tracking (depends on the above, plus a per-model
  price table this project has no reason to maintain yet).
- Per-call-site or per-function budgets.
- Any enforcement outside `eval_inference` — tool calls made directly
  from AINT source (not model-requested) aren't currently
  budget-constrained; only the model-facing loop is, since that's
  where an unbounded agentic conversation is actually a risk.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
