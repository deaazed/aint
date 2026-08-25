# Milestone 06 — Modules + standard library — spec

## Scope

`import <module>` gating access to a small set of builtin functions, and
— because it was explicitly deferred here in milestone 05's `SPEC.md` —
minimal immutable list support: literals, indexing, and a length
function. Enough to write real (non-AI) programs involving numbers,
text, and simple list processing.

## In scope

**`import` statement:** `import math` / `import string` / `import time`
/ `import collections`. Parses to `StmtKind::Import(String)`, a new
`fn`-sibling statement kind (new `import` lexer keyword). Not hoisted —
sequential like `let`, so it has to appear before its functions are
used, same as any real import system.

**Lists:** `[e1, e2, ...]` literal syntax (`ExprKind::List(Vec<Expr>)`,
new `[`/`]` lexer tokens) and `list[index]` indexing
(`ExprKind::Index`), as a postfix operation alongside calls. Lists are
**immutable** — consistent with the rest of the language, which has no
mutation of any kind yet (no reassignment, only `let` bindings). No
push/pop/mutation syntax, because there's nothing to model it after.

**Standard library, gated behind `import`:**

```
math        sqrt, pow, floor, ceil, round, abs, min, max   (all Float)
string      length, to_upper, to_lower, trim, contains, concat
time        now_seconds                                     (-> Int)
collections length                                    (List<T> -> Int)
```

`print` is unchanged from milestone 04 — still global, no import
needed. See "Design decisions" for the `module_function` naming and why
`collections.length` needed different treatment than everything else.

**New example:** `examples/stdlib.an` — recursive list summation using
`collections_length` + indexing, `math_sqrt`, `string_to_upper`, and a
deterministic use of `time_now_seconds` (checking `> 0`, never printing
the actual timestamp, since that would make the example's output
different on every run and untestable).

## Out of scope (later milestones, or deliberately not attempted)

- **`io` beyond `print`.** Reading stdin would need the interpreter to
  also be generic over an input source, symmetric to how it's already
  generic over output (`Interpreter<W: Write>`) — worth doing when a
  program actually needs interactive input, not preemptively.
- **`json`.** Meaningful JSON needs more than primitives to serialize;
  revisit once records/structs exist.
- **`http`.** Real I/O, way outside a tree-walk interpreter milestone.
  It was only ever an illustrative `import` syntax example in
  `ROADMAP.md`'s milestone blurb, not a commitment.
- **`Option<T>` construction** (`Some`/`None`). Nothing in this
  milestone needs it; `List<T>` was the concrete, promised gap to close.
- **Mutable lists**, or any list method beyond indexing + length. No
  `push`, no slicing, no negative/from-the-end indexing.
- **General type-checked collection operations** (`map`, `filter`,
  `reduce`) — would want closures/first-class functions first, which
  don't exist.
- **Empty list literals.** `[]` has no way to infer its element type
  (there's no typed `let` yet to supply one — see milestone 05's
  `SPEC.md`), so it's a type error: "cannot infer the type of an empty
  list literal." Revisit once `let x: List<Int> = []`-style annotations
  exist.
- **Loops.** Not part of this milestone and not part of any milestone on
  the roadmap so far — AINT's only iteration mechanism is recursion.
  This is exactly why `collections_length` + indexing matter: without
  them, there'd be no way to process a list at all.

## Design decisions

- **No dotted module access (`math.sqrt(x)`).** The parser has no
  field/method-access expression yet, and adding one is a bigger,
  separate feature — `docs/LANGUAGE_DESIGN.md` already shows dotted
  *method* calls on `Distribution<T>` (`result.probability(x)`), which
  milestone 10 will need to add anyway. Until then, `import <module>`
  brings plain, flat function names into scope instead.
- **Stdlib functions are named `module_function`** (`math_sqrt`,
  `string_length`, `collections_length`, ...), not bare `sqrt`/`length`.
  Without module namespacing, `string.length` and `collections.length`
  would collide as a single flat `length` binding — the prefix is the
  simplest fix given the no-dotted-access constraint above, and reads
  clearly even if it's more C than Python.
- **`collections_length` is the one genuinely polymorphic stdlib
  function** (`List<T> -> Int` for *any* `T`), and the type checker has
  no generics mechanism. It's special-cased in `check_call` exactly like
  `print` already is — accept any `List<_>`, don't try to build a real
  generics system for one function.
- **Small duplication between the type checker's stdlib signature
  tables and the interpreter's native-function bindings** is accepted,
  not eliminated. A shared registry would either need logic in
  `aint-ast` (against that crate's "no logic" charter) or a new crate
  just for ~17 function entries — not worth it. This already matches
  how `print`'s arity is independently hardcoded in both crates today.
- **An out-of-range or negative list index is a `RuntimeError`, not a
  parse or type error** — it's inherently a runtime fact (depends on the
  list's actual length at that point in execution), the same category
  as division by zero.
