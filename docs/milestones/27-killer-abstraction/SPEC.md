# Milestone 27 — Find the killer abstraction

## Scope

`ROADMAP.md`:

> Not predetermined. After milestone 26, ask what AINT actually made
> dramatically easier — typed inference, uncertainty handling, AI
> workflows, model orchestration, or something not yet imagined. That
> answer becomes the language's real thesis statement, replacing the
> working hypothesis in `LANGUAGE_DESIGN.md` if it turns out to be
> different.

A synthesis milestone, not a code milestone — the deliverable is an
answer, backed by evidence from milestones 08 through 26, and an
update to `LANGUAGE_DESIGN.md`'s thesis if the evidence points
somewhere the original working hypothesis didn't.

`LANGUAGE_DESIGN.md` itself already names the test this milestone
has to pass honestly:

> If AINT isn't clearly better on that comparison, the language
> hasn't found its abstraction yet, and milestone 27 exists to go
> find it rather than to declare victory on a predetermined answer.

Milestone 26's actual result was not a clean win. That's the honest
starting point for this milestone, not a problem to paper over.

## Method

A pass over every milestone from 08 (first AI primitive) to 26
(benchmark), asking one question of each: **is the thing this
milestone built something Python's current ecosystem also has, or
something it genuinely doesn't?** Not "is it possible in Python" —
almost everything is possible in Python — but "does mainstream Python
tooling already give you this, or would you be hand-rolling it, every
time, per project, with no shared convention to lean on?"

The answer sorted cleanly into two groups.

### Matched or substantially matched by Python's current ecosystem

- **Typed/structured inference** (milestones 08–09). `infer classify(text:
  String) -> Sentiment` and Pydantic's `response_format=` structured
  outputs (used directly in `benchmark/python/ai.py`) do the same job
  in a comparable number of lines. Python's ecosystem caught up on
  this specifically since AINT's original thesis was written.
- **Typed tool schemas** (milestones 11–12). LangGraph/function-
  calling schemas are typed and validated too, via the same Pydantic
  models. Not a differentiator either way.
- **Vendor-neutral model selection** (milestone 16). LangChain's
  `ChatOpenAI`/`ChatAnthropic`/etc. share a common interface the same
  way `infer` never names a vendor in source. Both ecosystems solved
  this.
- **Deterministic offline testing of AI-touching code** (milestone
  15). The original thesis claimed this as a language-level win
  ("testable offline... the same way ordinary functions are"). The
  actual milestone 26 finding was the opposite of a clean win:
  `pytest` + `TestClient` + `monkeypatch` covered *more* ground (the
  AI-decision logic *and* the full HTTP lifecycle) in *one* suite,
  where AINT needed two separate mechanisms for a real, documented
  reason (`docs/milestones/25-real-application/SPEC.md`). Named
  directly because the original thesis document asserted this as a
  strength before the evidence existed to check it.

### Not matched — no equivalent, not a convention, not a library away

- **Uncertainty as a first-class type** (milestone 10).
  `Distribution<T>` with `probability()`/`argmax()`/`entropy()`/
  `sample()`/`require_confidence()` built into the type system. Python
  has no standard idiom for this at all — every project that needs it
  hand-rolls its own shape, differently, with no shared vocabulary a
  second developer can already read.
- **Static effect checking for AI operations** (milestone 13). `pure`
  cannot call something that reaches a model or a tool, checked by the
  compiler, unconditionally, for every function. No mainstream Python
  static-analysis tool checks "does this function call an LLM" as a
  first-class property the way it checks type correctness.
- **Per-inference tool authorization, checked twice** (milestone 20).
  `permissions [...]` on an `infer` declaration is validated at
  compile time (does this name a real tool?) and enforced again at
  the point of execution, independent of what was offered to the
  model. A LangGraph node reaching for a tool outside its intended
  scope is a code-review question, not something the type checker or
  runtime stops.
- **Resource budgets as a language construct** (milestone 17).
  `budget { max_tokens max_model_calls max_cost timeout_ms }` is
  runtime-enforced, program-wide, with no library to remember to wire
  in. Nothing in the LangGraph/LangChain stack has a comparable
  built-in primitive; token/cost ceilings are middleware you write
  yourself, per project.
- **Automatic execution tracing** (milestone 14). Every `infer`/`tool`
  call gets a trace record with zero application code — `TraceRecord`
  is part of the runtime, not a library. (Honestly weakened by
  milestone 25's own finding: the demo app never surfaces this
  anywhere in its HTTP API. The capability exists; the application
  built on top of it didn't use it.)

## What this sorts into

Everything in the first group is about **how easy an individual AI
call is to write**. Everything in the second group is about **what
the compiler and runtime will and won't let a whole program do with
AI**, unconditionally, without anyone having to remember to enforce
it. The first group is where Python's ecosystem has converged with
AINT's original bet; the second is where it hasn't, and there's no
sign it's about to — none of `effects`, `permissions`, or `budget`
map onto anything a linter or a decorator library bolts onto Python
after the fact, because they need the type checker and the runtime to
agree with each other, not a convention layered on top of a dynamic
language.

See `FINDINGS.md` for the actual thesis this produces, and the update
made to `docs/LANGUAGE_DESIGN.md`.

## Explicitly out of scope

- Any new code. This milestone changes documentation only.
- Re-running milestone 26's benchmark under different conditions —
  the finding here is built on the numbers already gathered, not new
  ones.
- Deciding whether `agent` should become a keyword — `LANGUAGE_DESIGN.md`
  already defers that explicitly to "after real agents have been built
  in the language and a pattern actually repeats," which hasn't
  happened yet (the customer-support app is a request/response system
  with one AI decision point, not an agent).

## Outcome

To be filled in `ACCEPTANCE.md` once `FINDINGS.md` and the
`LANGUAGE_DESIGN.md` update land.
