# Milestone 02 — Lexer — acceptance

## Scope

See `SPEC.md`. Tokenize `.an` source into a stream of tokens with real
positions, for the parser (milestone 03) to consume.

## Acceptance criteria

- [x] `let x = 42` produces `LET IDENTIFIER(x) EQUAL INTEGER(42)`.
- [x] Keywords (`let fn return if else true false`) are never lexed as
      identifiers.
- [x] Unit tests cover each literal kind (integer, float, string,
      identifier) and each keyword.
- [x] Multi-character operators (`==`, `!=`, `->`) are lexed as one
      token, not split.
- [x] Unit tests cover every operator and punctuation token.
- [x] Malformed number, unterminated string, and unknown character each
      produce a positioned (`LexError` with a `Span`) error, with test
      cases for all three.
- [x] `//` line comments are skipped.
- [x] Token spans after a comment are still correct.
- [x] Non-ASCII identifiers (`café`, `π`) lex correctly.
- [x] `cargo test -p aint-lexer` passes.
- [x] A fixture test tokenizes the full `examples/hello.an` program.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace, not just the lexer crate.

## Explicitly out of scope

See `SPEC.md`'s "out of scope" section — newline significance, block
comments, extended numeric literal syntax, full escape validation,
multi-error recovery, and any AI-specific syntax are all deferred to
later milestones.

## Outcome

Satisfied by `crates/lexer/` (`token.rs`, `error.rs`, `lexer.rs`,
`lib.rs`), `crates/lexer/tests/hello.rs`, and `Position`/`Span` added to
`crates/ast/src/span.rs`. 17 unit tests + 1 integration test, all
passing.
