# Milestone 13 — Effects

## Scope

`ROADMAP.md`:

> `pure`, `inference`, `tool`, `network`, `filesystem` as declared
> function effects, checked by the compiler.

Delivers an optional `effects [ ... ]` clause on `fn`/`async fn`
declarations, and a static check: a function that declares its effects
can only call other functions whose effects are a subset of its own.

## Design decisions

**Effect checking is opt-in, not retroactive.** Every `fn` written
before this milestone — every existing example, every existing test —
has no `effects` clause. Making the *absence* of a clause mean `pure`
would silently break all of them (`print` alone is I/O; most existing
functions recurse and call `print`). So no clause means "effects
untracked," not "no effects" — the function is exempt from checking
both as caller and as callee. Only a function that explicitly opts in
by writing `effects [...]` gets checked. This is a real, deliberate
asymmetry with `LANGUAGE_DESIGN.md`'s framing (which reads as if
purity were the default) — but the alternative breaks the entire
existing test suite for a milestone that's supposed to be additive.

**Effect checking is sound where it applies: a checked function can
only call other checked functions.** If `f` declares `effects
[pure]`, it can't call `g` just because `g` *happens* to do nothing
dangerous — `g` has to have said so itself (`effects [pure]` too, or a
compatible declared set). Calling an *un*-annotated user function from
inside a checked one is a type error ("callee has no declared
effects"), not a silent pass. Without this, effect declarations would
be decorative — a `pure` function could always launder a side effect
through one undeclared helper.

**Stdlib and `print` calls are exempt from the check, in both
directions.** None of `math_*`/`string_*`/`collections_length`/
`distribution_*`/`option_*`/`time_*`/`print` can trigger `inference`
or `tool` — the only two effects anything in AINT can actually produce
right now. Retrofitting a hand-classified purity tag onto all 20-odd
stdlib functions (`time_sleep_ms` genuinely isn't pure, but doesn't
fit `inference`/`tool`/`network`/`filesystem` either) is real, separate
work with no payoff yet: nothing about *that* classification is
AI-specific, which is what this milestone and the roadmap's own
"Effects" framing are actually about. So every native call is treated
as compatible with any declared effect set. This is documented as a
scope boundary, not a soundness claim about stdlib purity in general.

**`infer` and `tool` declarations have an intrinsic, non-writable
effect — `effects [...]` syntax isn't extended to them.**
`LANGUAGE_DESIGN.md`'s sketch shows `effects [inference]` written on
an `infer` declaration, but an `infer` declaration is *always*
`inference`-effectful and a `tool` declaration is *always*
`tool`-effectful — there's no other value the clause could honestly
hold, so writing it would be pure redundancy with no way to say
anything false even by mistake. The type checker already knows this
intrinsically at every call site (it has to — that's what makes the
`pure`-calling-`infer` check meaningful at all). Extending the parser
to accept a clause that can only ever have one legal value, on two
already-shipped, bodyless declaration kinds, was judged not worth the
surface area. `fn`/`async fn` are the only kinds `effects` is
actually new, meaningful syntax for.

**`[pure]` must be alone.** `effects [pure, tool]` is rejected —
`pure` specifically means "no effects," so combining it with anything
is a contradiction, not a union. Every other combination
(`effects [inference, tool]`, for an agent-style function that might
do either) is a plain set.

**`network` and `filesystem` parse and are accepted, but check against
nothing yet.** They're real words in the roadmap's effect vocabulary,
and rejecting them at parse time would mean re-adding them later. But
no AINT primitive performs network or filesystem I/O — there's nothing
in the language yet that a `network`/`filesystem`-declaring function
could call that a `pure` one couldn't. They exist as accepted,
currently-vacuous vocabulary, not enforced constraints, and that's
stated outright rather than left to be discovered.

## Explicitly out of scope

- Effect classification for the standard library.
- Checking `network`/`filesystem` against anything (nothing produces
  those effects yet).
- Effect polymorphism / generic effects.
- An `examples/*.an` — effects are entirely a type-checking-time
  concept (nothing about them is visible at runtime, and the
  interpreter never reads them), so a working example doesn't
  demonstrate anything an existing example doesn't; verified instead
  by typechecker tests, which is where all the actual behavior lives.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
