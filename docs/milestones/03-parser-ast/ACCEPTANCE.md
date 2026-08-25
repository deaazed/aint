# Milestone 03 — Parser + AST — acceptance

## Scope

See `SPEC.md`. Parse a token stream into the AST defined in `aint-ast`.

## Acceptance criteria

- [x] `ExprKind` covers literals (int/float/string/bool), identifiers,
      unary negation, binary operators, calls; `StmtKind` covers `let`,
      `if`/`else`, expression-statements. Every node carries a `Span`.
- [x] `1 + 2 * 3` parses as `Add(1, Mul(2, 3))`, not `Mul(Add(1,2), 3)`.
- [x] `==`/`!=` bind looser than `<`/`>`, which bind looser than `+`/`-`,
      which bind looser than `*`/`/`.
- [x] Unary minus binds tighter than any binary operator
      (`-1 + 2` = `Add(Neg(1), 2)`).
- [x] `(expr)` overrides precedence and produces no extra AST node.
- [x] `let x = 42` parses to `StmtKind::Let`.
- [x] `if` with and without `else` both parse correctly.
- [x] A program is just `statement*` — no separator token required
      between statements.
- [x] `callee(arg, ...)` parses to `ExprKind::Call`, for zero, one, and
      multiple arguments; chained calls (`f()()`) work.
- [x] A missing `=` in a `let`, and an unclosed `(`, each produce a
      positioned `ParseError::Unexpected`.
- [x] A lex error (e.g. unterminated string) surfaces through
      `ParseError::Lex`, and `.span()` resolves for both variants.
- [x] `cargo test -p aint-parser` passes.
- [x] A fixture test parses the full `examples/hello.an` program and
      checks the resulting AST shape.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — `fn`/`return`, type annotations, AI-specific syntax,
full statement-boundary disambiguation, and multi-error recovery are all
deferred, most of them to milestone 04 or later.

## Outcome

Satisfied by `crates/ast/src/{expr,stmt}.rs`, `crates/parser/src/{error,
parser,lib}.rs`, and `crates/parser/tests/hello.rs`. 19 unit tests + 1
integration test in `aint-parser`, all passing; 17 + 1 from the lexer
still passing.
