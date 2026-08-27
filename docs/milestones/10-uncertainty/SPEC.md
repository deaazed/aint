# Milestone 10 — Uncertainty

## Scope

`ROADMAP.md`:

> `Distribution<T>` with `probability()`, `argmax()`, `entropy()`,
> `sample()`, `require_confidence()`. Decide, explicitly and in
> writing, what "probability" means here — see `LANGUAGE_DESIGN.md`.

## What "probability" means here (the required explicit decision)

`LANGUAGE_DESIGN.md` names the problem directly: "model token
probability, calibrated confidence, normalized model scores, and
empirical probability are not the same thing, and picking one silently
is a design bug, not a shortcut."

The decision: **AINT does not pick one.** A `Distribution<T>` carries
whatever numbers the `Model` that produced it reported — the language
makes no claim about their statistical meaning beyond two structural
guarantees it enforces on every `Distribution<T>` before it becomes a
usable value:

1. Every probability is in `[0.0, 1.0]`.
2. The probabilities sum to `1.0` (within `1e-6`).

That's it. Whether those numbers are raw softmax output, a
temperature-scaled score, a calibrated confidence, or something a
prompt asked the model to self-report is a property of *that model*,
not of the language. This is enforced the same way milestone 09
enforces "the response actually names a real enum variant" — as
runtime schema validation, not a type-level guarantee, because it's
checking something only the model's *actual answer* can violate.
Real model adapters (milestone 16) are responsible for being honest in
their own documentation about what numbers they hand `Distribution<T>`
— AINT gives them a well-defined structural contract to fill in, not a
semantic promise it can't back.

This is a real, deliberate scope boundary, not a deferral: nothing
later in the roadmap asks AINT to unify these into one meaning, and
doing so would just be picking one anyway.

## Design decisions

**`Distribution<T>` only exists over `enum` types, for now.**
Representing a distribution needs an enumerable, finite value set.
Every type in AINT that has one is an `enum` (milestone 09) — `Bool`
technically qualifies too, but supporting it would mean a second,
parallel `Value` representation instead of reusing enum infrastructure
end to end, for a case nothing in the roadmap asks for yet. Declaring
`Distribution<Int>` or `Distribution<String>` is a type error.

**No literal construction syntax — same reasoning as `Inference<T>`.**
A `Distribution<T>` value only ever comes from an `infer` function
declared to return one (`infer classify(text: String) ->
Distribution<Sentiment>`), validated by the runtime the same way an
`infer -> Enum` response is. `MockModel` remains the only source of
one before milestone 16, configured directly in Rust — same testing
story as every AI-touching feature since milestone 08.

**`probability`/`argmax`/`entropy`/`sample`/`require_confidence` are
free functions, not methods.** `LANGUAGE_DESIGN.md`'s own sketch writes
these as `.probability(x)` — but AINT has had no dotted access
anywhere, in any milestone, since before that sketch was written
(`string_length`, not `string.length`; `Sentiment_Positive`, not
`Sentiment.Positive`). Introducing dotted method syntax for exactly
one type would be a second rule for the same problem the codebase has
a consistent answer to already. These become `distribution_probability`,
`distribution_argmax`, `distribution_entropy`, `distribution_sample`,
and `distribution_require_confidence`, gated behind `import
distribution` like every other stdlib function — a deliberate,
documented departure from the sketch's exact syntax, not an oversight.

**`require_confidence` needed `Option<T>` to become real, not just
declared.** `Option<T>` has existed as a type since milestone 05 but
nothing has ever constructed one — this is the first. Since AINT has
no pattern matching yet, an `Option<T>` value needs *some* way to be
inspected from AINT source or `require_confidence` would be a dead
end. Two minimal functions cover it: `option_is_some(x) -> Bool` and
`option_unwrap(x) -> T` (a runtime error if `x` is `None` — the
caller's job is to check first). These live in their own `import
option` module, not bundled into `distribution`, since `Option<T>` is
a general type that other features will eventually produce too.

**`sample()` is genuinely random, and that's fine.** It's an explicit,
named operation on a distribution — the same kind of thing `random()`
is in any language — not a hidden source of nondeterminism in AI
behavior itself. Uses the `rand` crate (new dependency, `aint-runtime`
only). Milestone 15's deterministic-testing story is about not
requiring a *live model* to pass a test suite; a test that specifically
exercises `sample()` and needs a fixed outcome can construct a
degenerate distribution (one variant at probability `1.0`) — `sample`
is deterministic in that case by construction, no seeding needed yet.

**Polymorphism follows the `collections_length` precedent exactly.**
Milestone 06 already solved "one stdlib function, many element types"
with `Binding::PolymorphicListFunction`, special-cased in
`check_call`, with the *runtime* doing nothing generic at all — by the
time `stdlib::call` runs, `Value` is already dynamically typed, so
there's no Rust-level polymorphism to solve, only a typechecker-level
one. `distribution_*`/`option_*` reuse this shape
(`Binding::PolymorphicDistributionFunction`/`PolymorphicOptionFunction`)
rather than inventing something new, and `collections_length`'s own
code is untouched.

## Explicitly out of scope

- `Distribution<Bool>` or any non-enum `Distribution<T>`.
- Pattern matching / `match` (still milestone-less; `option_unwrap`
  and plain `if`/`==` are the only ways to work with `Option<T>` for
  now).
- Seeded/reproducible `sample()`.
- Real model backends actually producing calibrated distributions
  (16) — `MockModel` remains the only source, same as every AI
  primitive so far.
- An `examples/*.an` exercising any of this. Every `distribution_*`
  function takes a `Distribution<T>` as input, and the only way to
  produce one at all is an `infer` call answered by a real `Model` —
  there is no literal syntax for one (see above) and no AINT-level way
  to mock one yet. So this milestone inherits milestones 08 and 09's
  gap in full, with no partial workaround this time: unlike `enum`
  (usable standalone, which is why `examples/enums.an` exists),
  `Distribution<T>` has no way to exist in a running program at all
  until milestone 15 (mocking) or 16 (real models). Verified instead
  by runtime unit tests calling `distribution_*`/`option_*` directly
  against hand-built `Value::Distribution`/`Value::Option` values, plus
  the same schema-validation-through-`MockModel` tests milestone 09
  established.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
