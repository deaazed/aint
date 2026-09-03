# Milestone 37 — Conditional expressions

## Scope

`if`/`else` has been a statement since day one: it can run one of two
blocks, but it can't produce a value. Combined with no reassignment
anywhere in the grammar, that means "compute one of two values" has no
way to be expressed except duplicating the surrounding code in every
branch — every branch has to independently `return` (or otherwise
finish) whatever depends on the choice. Building a real website on top
of AINT (`examples/website/`, then the standalone `aint-website`
project) found this directly: a four-variant label function needed
three levels of nested `if`/`else`, and a two-branch page handler had
to duplicate its entire page-wrapping call in both branches rather than
compute a value once and return it. See `ROADMAP.md`'s Phase 3 framing.

This milestone adds `if`/`else` as an expression, and `else if` as
sugar for `else { if ... }` — both found costly in the same retrospective,
fixed together since the second is nearly free once the first exists.

## What this milestone actually builds

**`if`/`else` used as a value:**

```an
let x = if condition { then_value } else { else_value }
let label = if n < 0 { "negative" } else if n == 0 { "zero" } else { "positive" }
```

A new, separate AST node — `ExprKind::If { condition, then_value,
else_value }` — deliberately not a reuse of `StmtKind::If`. Each branch
is exactly **one expression**, not a `Block` of statements: no `let`,
no `return`, no sequencing inside `{ }` in this position. `else` is
**required**, not optional, since both branches must produce a value.
Both branches must type-check to the same `Type`, which becomes the
whole expression's type — checked the same way a lambda's declared
return type is checked against what its body actually returns, just
without a declared type to check against: the two branches check
each other.

**Where it's reachable**: only from expression position (`parse_primary`,
alongside `fn(...) -> T { ... }` lambdas) — a bare `if` at the start of
a statement is completely unaffected and always parses as the existing
`StmtKind::If` (`parse_statement` dispatches on the `if` keyword before
ever falling through to expression parsing). Every program written
before this milestone parses identically. The two forms differ in one
more way worth stating plainly: the statement form's `else` stays
optional (unchanged), the expression form's `else` is required.

**`else if`, for both forms, as parser-level sugar with no AST
footprint of its own:**

- Statement form: `parse_if_statement`, on seeing `else` followed by
  `if` rather than `{`, recurses into itself and wraps the result in a
  single-statement `Block` — i.e. `else if cond { ... }` desugars
  exactly to `else { if cond { ... } }`. `StmtKind::If`'s shape is
  completely unchanged; the typechecker, interpreter, IR lowering, and
  VM never know the difference. `aint fmt` recognizes this exact
  shape (a `Block` holding exactly one `If` statement and nothing else)
  and prints it back as `else if` rather than an extra level of `{ }`.
- Expression form: `parse_if_expr`, on the same lookahead, recurses
  directly into another `parse_if_expr` and uses the result as
  `else_value` — already an `Expr`, no wrapping needed. A whole
  `else if` chain is one flat `ExprKind::If` spine. `aint fmt` prints
  it flat for the same reason (checking whether `else_value` is itself
  `ExprKind::If`).

Both recurse arbitrarily, so an `else if` chain of any length works,
in either form.

## Design decisions

**Interpreter-only, same shape as closures (milestone 30). The
bytecode VM and IR compiler reject the expression form explicitly, not
silently.** `ExprKind::If` fails immediately at IR lowering
(`LowerError::UnsupportedIfExpr`) — there's no AIR node for it, the
same reasoning `LowerError::UnsupportedLambda` already established.
The *statement* form of `if`/`else` is completely unaffected either
way — it already lowers and runs under `aint run --vm` exactly as it
always has, `else if` included, since that's pure parser sugar with no
new AST shape for the VM path to not-support.

**Single-expression branches, not a block with a tail expression.**
A more general form — `if cond { let y = f(x); y + 1 } else { 0 }`,
where a branch is a `Block` and its last statement's expression becomes
the value — was considered and deliberately deferred. It would need
the typechecker and interpreter to understand "a block used as a
value" as its own concept (what if the block doesn't end in an
expression? what about `return`/`assert` appearing before the tail?),
real complexity for a case the actual motivating examples didn't need
— every real duplication found while building `aint-website` collapses
cleanly to a single expression per branch once inlined. This is the
same "smallest lever" reasoning closures were scoped with; a
block-with-tail-expression form is real, separate, additive work if a
future real program shows single-expression branches aren't enough.

**Type mismatch between branches reuses `TypeError::Mismatch`**, not a
new variant — consistent with how a lambda's body/return-type mismatch
and an `await`ed non-awaitable both already report through the same
generic mismatch error with a specific message, rather than the
type checker accumulating a bespoke error variant per shape of
mismatch.

## Explicitly out of scope

- **Block-with-tail-expression branches.** See "Design decisions"
  above — real, deferred, not attempted here.
- **The bytecode VM/IR compiler executing the expression form at
  all.** A documented gap, not attempted, matching closures.
- **`match`/pattern matching.** `if`/`else` (now usable as a value)
  stays the only branching construct — a separate, larger feature,
  not scoped here.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
