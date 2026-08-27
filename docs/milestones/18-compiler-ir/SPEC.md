# Milestone 18 — Compiler IR (AIR)

## Scope

`ROADMAP.md`:

> Once surface semantics have stabilized: typed AST -> AIR, with
> explicit `INFER`, `TOOL_CALL`, `DISTRIBUTION`, `PROBABILITY`
> operations instead of generic calls.

Milestones 08-17 stabilized exactly the surface this describes —
`infer`, `tool`, `Distribution<T>`'s five operations, effects,
tracing, testing, real adapters, budgets. This milestone doesn't
change any of that surface; it adds a second, explicit representation
of it, lowered from the (already type-checked) AST.

## What this milestone is not

`aint-ir`'s own placeholder doc comment (written at project scaffolding)
already says AIR is "what lets the runtime cache, parallelize, and
route inference deliberately" — but that's milestone 19's job
("Optimization... now possible *because* AIR makes AI operations
visible"), and milestone 22's ("`AST -> AIR -> Bytecode -> AINT VM`").
This milestone builds the lowering step and the explicit
representation; it does not wire AIR into `aint run`, does not touch
the tree-walking interpreter that has executed every example and test
so far, and does not optimize anything. `crates/runtime` is untouched
by this milestone — the tree-walker remains how every AINT program
actually runs until milestone 22 replaces it, exactly as milestone 21
("Memory model") already anticipates by name ("don't invent this
early").

## Design decisions

**AIR is a parallel type set in `aint-ir`, not a generalization of
`aint-ast`'s types.** Making `StmtKind`/`ExprKind` generic over an
expression representation so AIR could reuse them would touch every
one of the five crates that already depend on `aint-ast` as it is
today. A separate `AirProgram`/`AirStmt`/`AirExpr` in `aint-ir`, built
by a lowering pass that only *reads* `aint-ast`, keeps this addition
contained to the one crate it belongs in — the same reasoning that
kept `Value` (runtime) and `Type` (type checker) as separate
representations from day one rather than one shared "the type of an
AINT thing" enum.

**Four explicit AI-operation node kinds, matching `ROADMAP.md`'s own
four names exactly:** `AirExpr::Infer`, `AirExpr::ToolCall`,
`AirExpr::Distribution` (covering `argmax`/`entropy`/`sample`/
`require_confidence`, tagged by which one), and `AirExpr::Probability`
(covering `distribution_probability` specifically, as its own node —
`ROADMAP.md` names it separately from `DISTRIBUTION`, not as one of
its cases, so AIR keeps them separately nameable too). Every other
call — a plain `fn`, an `async fn`, any stdlib function including
`option_is_some`/`option_unwrap` — lowers to a single generic
`AirExpr::Call`. `Option<T>`'s two operations aren't AI-specific (only
one of the things that can *produce* an `Option` is), and
`ROADMAP.md`'s own list doesn't name them — extending the "explicit
operation" treatment to them would be scope creep past what this
milestone actually asks for.

**Lowering re-derives "is this callee an `infer`/`tool`?" itself,
rather than consuming a result from `aint-typechecker`.** The type
checker's own resolution (`Binding`, `CallMode`, `EffectInfo`,
`DistributionOp`) is private to that crate and was never meant to be
exported — every stage in this pipeline has accepted a small amount of
duplicated logic in exchange for staying decoupled from its neighbor's
internals since milestone 06 (`stdlib.rs` exists in both
`aint-typechecker` and `aint-runtime` for exactly this reason). AIR's
lowering pass does its own minimal pre-pass over top-level `infer`/
`tool` declarations (mirroring, not sharing, the type checker's own
hoisting) and recognizes the five `distribution_*` names directly, the
same way the interpreter's own `NativeFunction` enum hardcodes them.

**Only top-level `infer`/`tool` declarations are recognized during
lowering.** A block-nested `infer`/`tool` declaration (legal syntax,
never actually used in any example or test) falls back to a generic
`AirExpr::Call` rather than being specially recognized — consistent
with how the type checker's own forward-reference hoisting is also
top-level-only. A documented limitation, not a silent gap: block-nested
`infer`/`tool` is themselves already an edge case nothing in this
codebase exercises.

**Declarations carry less than the AST does.** `AirStmt::Fn` keeps
parameter *names* (what a body needs to bind them) but not their
declared types or `effects` clause — both are type-checking-time
facts already fully spent by the time AIR exists, the same way the
interpreter's own `Function`/`InferenceFn`/`ToolFn` runtime values
already discard types where they're not needed for execution. AIR is
meant to be leaner than the AST it's lowered from, not a lossless
mirror of it.

**Verified entirely through `aint-ir`'s own tests — real AINT source
in, asserted AIR shape out — not through `aint run`.** Since nothing
consumes AIR yet, there's no end-to-end CLI behavior to test; the
correctness claim this milestone actually makes is "this call lowers
to this explicit node," which is exactly what a lowering-shape test
checks directly.

## Explicitly out of scope

- Wiring AIR into `aint run`/`aint test` or the tree-walking
  interpreter.
- Any optimization, caching, or scheduling (19).
- A bytecode form or VM (22).
- Struct/record lowering (no such AST node exists yet).
- Recognizing block-nested `infer`/`tool` declarations (see above).

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
