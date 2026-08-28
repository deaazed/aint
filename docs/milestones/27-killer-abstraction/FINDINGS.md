# Milestone 27 — Findings

See `SPEC.md` for the evidence this is built on.

## The question

`LANGUAGE_DESIGN.md`'s original thesis: deterministic computation and
probabilistic inference should be equally fundamental to the
language, expressed through typed inference (`infer`), typed
uncertainty (`Distribution<T>`), typed tools, and deterministic
testing. Milestone 26 was that thesis's actual test, and the result
was mixed — real wins (memory, binary size, non-crypto latency), real
losses (total line count once AINT's own one-time stdlib cost is
counted; single-suite testability), and a tie (crypto-dominated
latency, where neither language's own overhead was even visible).

That's not "the language hasn't found its abstraction" in the sense
of finding nothing — it's that **the abstraction the original thesis
named isn't the one the evidence actually supports.** Two of the four
things the thesis listed (typed inference, deterministic testing) are
matched or beaten by Python's current ecosystem. Two of them
(uncertainty as a type, and — not named in the original thesis at
all — effects/permissions/budget as compiler-and-runtime-enforced
constraints) are not.

## The finding

**AINT's killer abstraction is not "AI operations are easier to
write" — Python's ecosystem has converged on comparable ergonomics
for that specific claim, per milestone 26's own numbers. It's that
AINT makes a program's entire AI surface area statically checkable
and runtime-enforceable, the same way type safety already is,
instead of a set of conventions a team has to remember to apply.**

Concretely, three things a Python codebase cannot get from its type
checker or its language, no matter how disciplined the team is:

1. **`effects [pure]` proves a function cannot reach a model or a
   tool.** Not "the team agreed this function shouldn't call an LLM"
   — the compiler rejects the program if it does. `mypy`/`ruff`/any
   mainstream Python static analyzer has no concept of "this call
   reaches a network-backed model" as a checkable property.
2. **`permissions [...]` proves which tools a specific inference can
   reach, checked twice** — once statically (does this name a real,
   declared tool?), once at the point of execution (independent of
   what the model was actually offered). A LangGraph node that
   reaches for a tool outside its intended scope is caught in code
   review, if it's caught at all.
3. **`budget` is a resource ceiling the runtime enforces, not a
   decorator someone has to remember to add to every route.** Token
   count, model-call count, cost, and timeout are checked the same way
   for every `infer`/`tool` call in a program, because they're a
   property of the program, not of an individual call site.

`Distribution<T>` (milestone 10) is real and belongs alongside these
— genuinely first-class, genuinely something Python has no shared
idiom for — but it's a *type*, and the other three are *guarantees
about a whole program's behavior*. The guarantees are the sharper,
more defensible claim: a type you can work around by reaching for the
raw value; a compiler rejecting your program you cannot.

## Why this is a real, not a rhetorical, difference

The distinction matters because of *what kind of tool could close each
gap*. Typed inference and deterministic testing are gaps a good
library closes — and Pydantic, `instructor`, and `pytest`'s fixture
system have largely closed them already, which is exactly what
milestone 26 measured. Static effect checking, per-inference tool
authorization, and language-level resource budgets are not gaps a
library can close in a dynamically-typed language with no effect
system to hook into — they need the type checker and the runtime to
agree on what "this function's allowed behavior" means, which is a
language-design decision, not an import.

That is also, honestly, this project's biggest remaining gap relative
to its own thesis: none of `examples/customer_support/`,
`priority_logic_test.an`, or `benchmark/python/` actually *exercises*
this claim yet. `server.an` doesn't declare `effects [pure]` anywhere
that would matter, doesn't restrict `classify_sentiment`'s
`permissions`, and doesn't set a `budget`. The governance story is
real and already built (milestones 13, 17, 20) — the milestone-25
application just doesn't lean on it. That's worth stating plainly
rather than claiming a demonstration that doesn't exist yet.

## The update to `LANGUAGE_DESIGN.md`

The thesis section now reads governance-first, with uncertainty
alongside it, and typed inference/deterministic testing repositioned
as necessary infrastructure rather than the differentiator — see the
diff in `docs/LANGUAGE_DESIGN.md`. The document's own principles
(strong static typing, explicit uncertainty, typed tool validation,
observable runtime effects) don't change — they were already
consistent with this finding, just not framed around it. What changes
is which one gets named as *the* bet.
