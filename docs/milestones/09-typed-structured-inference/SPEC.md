# Milestone 09 — Typed structured inference

## Scope

`ROADMAP.md`:

> `enum Sentiment { Positive Neutral Negative }` plus `infer
> sentiment(text: String) -> Sentiment`, with the runtime generating a
> structured-output request and validating the response against the
> schema before it becomes an AINT value.

This is AINT's first user-defined type. Every prior milestone's SPEC
says "no user-defined types yet" — this is where that ends, scoped
specifically to enums (not general structs/records, which aren't on
the roadmap at all yet).

Delivers:

- `enum Name { Variant1 Variant2 ... }` as a top-level declaration.
- `Type::Enum(String)`, compared nominally by name.
- Enum-variant values as ordinary AINT expressions (see naming below),
  usable anywhere a value is: `let`, `print`, `==`, function
  arguments, `infer` return types.
- Runtime schema validation: an `infer` call's response is checked
  against the declared enum before it becomes a usable AINT value, not
  trusted blindly.

## Design decisions

**Variant syntax is `EnumName_Variant`, not `EnumName.Variant`.** AINT
has no dotted access anywhere — the standard library already avoids it
(`string_length`, not `string.length`) specifically so one rule covers
both cases instead of two. `Sentiment_Positive` is ordinary
`ExprKind::Identifier` syntax; no new expression kind exists for it.
The type checker resolves it by defining `EnumName_Variant` as a bound
name of type `Enum(EnumName)` when it processes the `enum` declaration
— the same mechanism that already binds a `fn`'s name. The interpreter
mirrors this: executing an `enum` statement defines each
`EnumName_Variant` in the environment as a `Value::Enum`. No new AST
node, no new parser rule beyond the declaration itself.

**`parse_type` now accepts any identifier, not just the seven built-in
names.** Before this milestone, an unrecognized type name was a parse
error (`fn f(x: Frobnicate) -> Int` failed immediately). Enums make the
set of valid type names open-ended and not knowable by the parser,
which has no symbol table by design. So `parse_type`'s fallback now
produces `Type::Enum(name)` speculatively for any identifier, and the
type checker — which already carries this responsibility for
undefined variables, functions, and modules — rejects it if no such
enum was declared (`TypeError::UnknownType`). This moves one error
from parse time to check time; the equivalent parser test
(`errors_on_unknown_type_name`) is replaced by a type-checker test
covering the same case at its new, correct layer.

**Schema validation lives in the interpreter, not in `Model`.** Per
the roadmap wording — "the runtime ... validating the response" — this
is a property of every model, not something each `Model` implementation
re-implements. `MockModel` (and every future real adapter) can return
whatever `Value` it wants; the interpreter checks it against the
`infer` call's declared return type before the caller ever sees it.
For an `Enum` return type, that means: the returned value is actually
a `Value::Enum` of the right enum name, and its variant is one the
`enum` declaration actually lists. A model that "hallucinates" an
unlisted variant is a real, named failure mode
(`RuntimeError::SchemaViolation`), not a silently-wrong `Bool`-style
comparison that just happens to always come out `false`. This is
directly testable today: configure `MockModel` with a
`Value::Enum("Sentiment", "Bogus")` and assert the runtime rejects it
— no real model required, continuing milestone 08's testability
promise into structured output specifically.

**`InferenceRequest` now carries the expected return type.** This is
the "structured-output request" half of the roadmap line — once a real
model adapter exists (milestone 16), this is what it builds a
JSON-schema / structured-output request from. Adding the field now,
even though `MockModel` ignores it, keeps `InferenceRequest`'s shape
stable for that milestone instead of growing it again later.

**Non-enum `infer` results are not additionally validated here.**
`Bool`/`Int`/`Float`/`String` results already get caught by whatever
uses them (`if` requires a real `Bool`, arithmetic requires a real
`Int`/`Float`, and so on) — the failure mode that motivates this
milestone (a well-typed-in-Rust but semantically-invalid value sailing
through undetected) is specific to enums, where an unlisted variant
compares merely "not equal" instead of erroring. General schema
validation for compound/richer types is revisited if and when the
roadmap calls for it.

## Explicitly out of scope

- General structs/records — only enums exist as user-defined types.
- Pattern matching / `match` — variant values are only compared with
  `==`/`!=` for now, same as any other type.
- `Distribution<T>`, uncertainty, `probability()` (milestone 10).
- Real model backends generating actual structured-output requests
  over the wire (16) — the request shape exists now, nothing sends it.
- Duplicate enum names, duplicate variant names within one enum, or
  variant-name collisions across two different enums: undefined
  behavior for now, not this milestone's concern.
- An `examples/*.an` exercising `infer` end-to-end still doesn't exist,
  for the same reason as milestone 08 — `aint run` still has no way to
  configure a mock (15) or reach a real model (16). Enums *without*
  `infer` are fully usable through `aint run` today, though, and get a
  real example (`examples/enums.an`) to prove it.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
