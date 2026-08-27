# Milestone 15 — Deterministic AI testing

## Scope

`ROADMAP.md`:

> `test { mock ... assert ... }` blocks. `aint test` must pass
> completely offline.

This closes a gap every AI-touching milestone since 08 has explicitly
carried forward: there has never been an AINT-level way to configure
what `MockModel`/`MockTool` return — only a Rust API, used by this
project's own test suite. `mock` is that missing piece, and `test`/
`assert` are what make it usable as an actual test.

## Design decisions

**`mock`'s value is a restricted expression, not
`LANGUAGE_DESIGN.md`'s distribution-literal sketch.** The sketch shows
`mock classify { refund: 0.96 ... }` — a whole probability
distribution written inline. Building literal syntax for
`Distribution<T>` values was explicitly ruled out in milestone 10
("no literal construction syntax — same reasoning as `Inference<T>`")
and revisiting that now, just for `mock` bodies, would mean two
different construction mechanisms for the same type. Scoped down
instead: `mock name -> value` configures a single canned answer for
an `infer` or `tool` declared as `name`, where `value` is a *literal or
enum-variant reference* (`42`, `"positive"`, `true`, `Sentiment_Positive`)
— evaluated by a small standalone evaluator, not the full interpreter
(see below). Mocking a `Distribution<T>`-returning `infer`, or
scripting a multi-step tool-calling conversation from AINT source, is
explicitly out of scope; both remain reachable from Rust tests via
`MockModel::script(...)` as before.

**`mock` values are evaluated by a tiny, standalone evaluator, not
`Interpreter::eval_expr`.** Building the `MockModel`/`MockTool` a test
needs happens *before* that test's `Interpreter` exists — there's a
real chicken-and-egg problem in using the interpreter's own expression
evaluation to compute the value that configures the interpreter. Since
`mock`'s values are deliberately restricted to literals and
`EnumName_Variant` references, evaluating them needs nothing from a
running interpreter — only the program's own `enum` declarations
(collected once, up front, the same way `enum` hoisting already works
in the type checker). This keeps `mock` evaluation total and
side-effect-free by construction, not just by convention.

**Each `test` block gets a completely fresh `Interpreter`.** Every
declaration in the file (`fn`/`infer`/`tool`/`enum`) is re-executed
into a new interpreter per test, rather than trying to share one
base environment across tests. This is real, deliberate redundancy —
re-registering a handful of bindings is cheap, and it guarantees one
test's `mock` configuration (or any state) can never leak into
another's. `aint test` on a file with N tests does the "load the
program" work N times; at today's program sizes that's not a real
cost, and it buys total test isolation for free.

**`mock` outside a `test` block is a type error, not a silent no-op.**
Configuring a mock in a plain `fn` or at top level would either do
nothing (confusing) or need `aint run` to build a fake model
(contradicting "offline testing is `aint test`'s job specifically").
The type checker tracks whether it's currently inside a `test` body
(same shape as `current_return_type`/`current_effects`) and rejects
`mock` anywhere else.

**`mock`'s target and value type are checked statically.** The target
must resolve to a declared `infer` or `tool`; the value's type must
equal that declaration's return type exactly. A test that mocks
`classify` (declared `-> Sentiment`) with `42` fails to compile, the
same way passing the wrong argument type to any function does.

**`assert` is a general statement, not test-block-only syntax** — it
type-checks its condition as `Bool` and, at runtime, produces a
positioned `RuntimeError::AssertionFailed` if false, wherever it
appears. Its distinctive behavior is entirely in how `aint test`
*handles* that error (catches it per-test and keeps going) versus how
`aint run` handles it (the program stops, same as any other runtime
error) — the statement itself doesn't need to know which context it's
in.

**A new CLI subcommand, `aint test <file>`, not a flag on `aint
run`.** Running a file and testing it are different operations with
different outputs (a program's real stdout vs. a pass/fail report) and
different semantics (one execution vs. N isolated ones) — conflating
them behind a flag would make both harder to reason about.

## Explicitly out of scope

- Mocking `Distribution<T>`-returning `infer` functions from AINT
  source.
- Scripting a multi-step tool-calling conversation (`mock` producing
  more than one canned response) from AINT source — `MockModel::script`
  remains Rust-only.
- Custom assertion messages (`assert cond, "message"`).
- Any expression beyond literals and `EnumName_Variant` references as
  a `mock` value (no arithmetic, no calls).
- Test discovery across multiple files, filtering by name, parallel
  test execution — `aint test <file>` runs every `test` block in that
  one file, in source order.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
