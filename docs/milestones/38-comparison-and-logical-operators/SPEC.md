# Milestone 38 — Comparison and logical operators

## Scope

`<=`, `>=`, `!`, `&&`, and `||` have been absent since the lexer's
first milestone — reached for by reflex, missing since day one, not a
deliberate design decision anyone had to revisit later. Building a real
website on top of AINT found this directly: a boundary check that
should have been `index >= length - 1` (in a hand-rolled string-replace
helper — see milestone 39) had to be inverted and its branches swapped
to work with only `<`. See `ROADMAP.md`'s Phase 3 framing.

This milestone adds all five. `<=`/`>=`/`!` are mechanical — the same
shape every existing comparison/unary operator already has. `&&`/`||`
are not: they short-circuit, which is a real semantic commitment with
consequences through every layer of the pipeline, not just new syntax.

## What this milestone actually builds

**Two new comparisons**, `<=` and `>=`, alongside the existing `<`/`>`
— same operand types (`Int`/`Int` or `Float`/`Float`), same `Bool`
result, in the typechecker, tree-walking interpreter, and bytecode VM
alike. No parity gap: these need no short-circuiting, so they compile
and run under `aint run --vm` exactly like `<`/`>` already did.

**One new unary operator**, `!` (logical negation) — `Bool -> Bool`,
same shape as `-` (`Int`/`Float -> Int`/`Float`), added alongside it in
`parse_unary`. Also no VM parity gap.

**Two new binary operators that short-circuit**, `&&` and `||`:

```an
if x != 0 && (10 / x) > 1 { ... }   // (10 / x) never runs when x is 0
if cache_hit || (expensive_lookup()) { ... }
```

The right operand is **not evaluated at all** when the left side
already decides the result — not just an evaluation-order guarantee.
`eval_expr`'s `ExprKind::Binary` handling special-cases `BinaryOp::And`/
`BinaryOp::Or` *before* generically evaluating both operands (which is
what every other binary operator still does): it evaluates the left
operand, and only evaluates the right operand if the result isn't
already determined. The generic post-evaluation `eval_binary` helper
(which only ever receives two already-evaluated `Value`s) never
actually sees `And`/`Or` — they're unreachable there by construction,
marked with `unreachable!()` rather than silently handled wrong.

**Precedence**, loosest to tightest: `||` < `&&` < `==`/`!=` <
`<`/`>`/`<=`/`>=` < `+`/`-` < `*`/`/` < unary `-`/`!`/`await` <
calls/indexing/literals — the same order most C-family languages use.
Two new recursive-descent levels, `parse_or` and `parse_and`, sit above
`parse_equality`; `parse_expr` now starts at `parse_or` instead of
`parse_equality` directly.

## Design decisions

**`&&`/`||` are interpreter-only, same shape as closures (milestone
30) and if-expressions (milestone 37). The bytecode VM rejects them
explicitly, not silently.** Short-circuiting needs real conditional-
jump bytecode — the compiler would have to emit something closer to
what an `if` statement compiles to than what every other binary
operator does (evaluate both operands eagerly, then apply the op).
Compiling `&&`/`||` the same eager way every other operator works would
be a genuine, silent semantic difference from `aint run` — not a
missing feature but a wrong answer, reachable the moment a right
operand has any observable effect (an error like division-by-zero, not
just a `tool`/`infer` call, which AIR already excludes entirely).
Rejected outright instead, at IR lowering
(`LowerError::UnsupportedShortCircuit`), before AIR ever represents
one. `<=`/`>=`/`!` have no such issue and fully support the VM path —
`AirExpr::Binary`/`AirExpr::Unary` already carry `aint_ast::BinaryOp`/
`UnaryOp` directly, so the new variants "just work" once `eval_binary`/
`eval_unary` in `crates/vm/src/vm.rs` (which duplicates
`aint-runtime`'s own, same reasoning `aint-ir` already accepted for a
small, fixed operator set) have the new match arms filled in.

**Lexer**: `!` becomes a real standalone token (`TokenKind::Bang`) for
the first time — previously only `!=` was recognized as two characters
together, and a lone `!` fell through to `UnknownCharacter`. `&&`/`||`
are recognized as their doubled forms only; a lone `&` or `|` is still
`UnknownCharacter` — there's no bitwise AND/OR, and no reason to
tokenize a character that can never start a valid expression on its
own.

**Type checking doesn't need to know about short-circuiting.** Both
branches of `&&`/`||` are always type-checked regardless of what a
value at runtime would do — same as every other expression; type
checking has no notion of "this code might not run." `check_binary`
requires both operands `Bool`, yields `Bool`, reusing the existing
`TypeError::Mismatch` shape.

## Explicitly out of scope

- **The bytecode VM/IR compiler executing `&&`/`||` at all.** A
  documented gap, not attempted, matching closures and if-expressions.
  A future milestone that gives the VM real conditional-jump bytecode
  (which `if` statements would need too, if they ever compile to
  genuine branches instead of whatever the VM currently does) could
  revisit this.
- **Bitwise operators** (`&`, `|`, `^`, `~`, shifts). Not the gap this
  milestone's retrospective found — AINT has no fixed-width integer
  story to make bitwise operations meaningful yet.
- **Compound assignment** (`+=`, `&&=`, etc.) — there's no
  reassignment anywhere in the grammar; irrelevant until (if ever)
  that changes.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
