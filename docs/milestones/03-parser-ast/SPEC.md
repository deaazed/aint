# Milestone 03 — Parser + AST — spec

## Scope

Parse a token stream from `aint-lexer` into the AST, defined in
`aint-ast` since the parser, type checker, and later stages all need to
agree on its shape.

## In scope

**AST (`aint-ast`):**
- `Expr` / `ExprKind`: integer, float, string, and bool literals,
  identifiers, unary negation, binary operators, function calls.
- `Stmt` / `StmtKind`: `let`, `if`/`else`, expression-statements.
- `Block`, `Program`.
- Every `Expr`/`Stmt` carries a `Span`, same `{ kind, span }` shape as
  `aint-lexer::Token`, for consistency and so later diagnostics have a
  position to point at.

**Parser (`aint-parser`):**
- `let IDENT = expr`
- `if expr { ... } else { ... }`, with `else` optional
- Expression-statements (needed for calls like `print(message)`)
- Expressions with standard precedence, lowest to highest:
  `== !=` → `< >` → `+ -` → `* /` → unary `-` → call → primary
- Function calls: `callee(arg, arg, ...)`, any number of arguments
  (including zero), left-associative so `f()()` parses.
- Parenthesized expressions for overriding precedence; `(expr)` parses
  to the inner `Expr` directly — no `Grouping` AST node, since parens
  don't change program meaning, only parse order.
- `ParseError` wraps `LexError` (parsing a source string lexes it first)
  plus one `Unexpected { expected, found: Token }` variant, with a
  `.span()` accessor for whichever variant it is.

## Out of scope (later milestones)

- `fn` definitions and `return` — milestone 04, alongside the
  interpreter that will actually execute them. `fn`/`return` are already
  lexed as keywords but the parser doesn't use them yet.
- Types/type annotations on `let` or function signatures — milestone 05.
- `infer`, `tool`, `Distribution<T>`, `budget`, and all other AI-specific
  syntax — added only when the milestones that need them arrive. The
  `StmtKind`/`ExprKind` enums are designed so new variants slot in
  without restructuring existing ones (see `docs/ARCHITECTURE.md`'s
  note about not hardcoding AI syntax into early passes).
- Full statement-boundary disambiguation. Milestone 02 made newlines
  pure whitespace at the lexer level, which means (as in JavaScript
  without semicolons) an expression-statement immediately followed by a
  line starting with `(` is ambiguous — `x\n(y)` and `x(y)` are lexically
  identical. Every current example avoids this (each statement starts
  with a keyword or ends in something that can't be followed by `(`).
  Not solved here; if it becomes a real problem, the fix is making
  newlines significant at the lexer, not a parser-side hack.
- Parser error recovery / multi-error collection — same reasoning as the
  lexer's `SPEC.md`: fails fast on the first error.

## Design decisions

- `Expr`/`Stmt` are `{ kind, span }` structs wrapping a `*Kind` enum,
  mirroring `Token { kind, span }` from milestone 02, rather than giving
  every enum variant its own span field.
- Precedence is implemented as one recursive-descent function per level
  (`parse_equality` → `parse_comparison` → `parse_term` → `parse_factor`
  → `parse_unary` → `parse_call` → `parse_primary`), the standard
  approach (as in *Crafting Interpreters*) rather than a table-driven
  Pratt parser — plenty for AINT's current operator set, and simple to
  extend a level at a time later.
