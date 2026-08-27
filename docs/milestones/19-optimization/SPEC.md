# Milestone 19 — Optimization

## Scope

`ROADMAP.md`:

> Inference caching, parallel inference, model routing, tool
> parallelization, request batching, prompt caching, memoization —
> now possible because AIR makes AI operations visible to the
> compiler.

Seven names, but they aren't seven independent features. This
milestone implements the one that's both genuinely sound given AINT's
actual semantics and squarely what four of the seven names are really
describing, and states plainly why the rest aren't implementable yet
rather than half-building them.

## The one thing this milestone actually builds

**Deduplicating repeated, identical `infer`/`tool`/`Distribution<T>`
operations within a block** — an AIR-to-AIR transformation
(`aint_ir::optimize`). "Inference caching," "prompt caching," and
"memoization" are the same idea at slightly different granularities:
don't pay for the same call twice. "Model routing" and "request
batching" aren't optimizations over *one* call graph at all — they're
about *which* backend answers and *how many requests go out at once*,
neither of which this pass touches.

This is sound specifically *because* of a guarantee AINT has had since
before any of this existed: **there is no mutation or reassignment
anywhere in the language.** Two occurrences of the identical call
(same function, syntactically identical literal/identifier arguments)
within the same block are guaranteed to see the same values for those
arguments — nothing could have changed them in between. That's what
makes deduplication provably safe here in a way it wouldn't
automatically be in a language with mutable variables.

The transformation: walking a block's statements in order, the first
occurrence of a cacheable call is left alone (if it's a bare
expression statement with no name, it's hoisted into a synthesized
`let` so later duplicates have something to reference); every later
statement whose value is the *same* call is rewritten to reference
that first result by name instead of calling again. Each block is
optimized independently and starts a fresh cache — a call inside an
`if`'s `then` branch is never treated as equivalent to one in the
`else` branch, since at most one of them ever actually runs.

**What counts as "the identical call," conservatively:** the function/
tool/distribution-op name, plus arguments that are each *either* a
literal or a bare identifier — nothing containing a nested call.
`classify("great")` matches another `classify("great")`;
`classify(review)` matches another `classify(review)` (same bound
name, and it can't have changed); `classify(format_review(review))`
is never treated as cacheable at all, because nothing here attempts to
prove `format_review` itself has no side effects.

## What's explicitly not built, and why

- **Parallel inference / tool parallelization** — need concurrent
  execution. Milestone 07 deliberately built a *single-threaded*
  Tokio runtime with no `tokio::spawn` and no `parallel { }`,
  documented at the time as an explicit, load-bearing architectural
  choice (`Value`/`Environment` use `Rc`/`RefCell`, not `Send`).
  Actually running two inferences concurrently needs that decision
  revisited first — real, separate work, not something an AIR pass
  can retrofit.
- **Model routing** — needs more than one model reachable from a
  single program. `Interpreter<W, M: Model>` has exactly one `model`
  field; there is no per-`infer`-call model selection anywhere in the
  language or runtime. A deployment-config mechanism would need to
  exist first (itself deferred in milestone 16, adjacent to milestone
  23's manifest work).
- **Request batching** — needs a real backend that supports batched
  requests and a runtime that accumulates calls before flushing them.
  `HttpModel` (16) sends one request per call, synchronously; nothing
  in the current execution model defers or groups calls at all.

None of these are "not started" so much as "blocked on a specific,
named prerequisite that isn't built" — stated here so the gap is
findable, not discovered by someone looking for a batching flag that
doesn't exist.

## Design decisions

**`optimize` is a separate, explicitly-invoked function, not baked
into `lower`.** Nothing consumes AIR yet at all (milestone 18); making
optimization automatic on every lowering would optimize a
representation nothing reads. `aint_ir::optimize(&AirProgram) ->
(AirProgram, OptimizationStats)` is a second, independent stage a
caller opts into — the same relationship a real compiler's `-O` flag
has to parsing.

**Verified the same way lowering was: real AINT source in, asserted
AIR shape out.** A program with two identical `classify("great")`
calls in one block optimizes to one real call and one reference; two
calls with *different* arguments both survive untouched; a call
inside one `if` branch and an identical one in the other both survive
(different blocks, no shared cache).

## Explicitly out of scope

- Everything in "What's explicitly not built, and why," above.
- Cross-block or cross-function deduplication (only within one
  block).
- Deduplicating anything whose arguments contain a nested call.
- Any change to `crates/runtime` — `aint run`/`aint test` are
  unaffected; this operates entirely on AIR, which nothing executes.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
