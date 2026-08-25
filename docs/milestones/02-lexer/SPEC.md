# Milestone 02 — Lexer — spec

## Scope

Tokenize AINT source text (`.an`) into a stream of tokens for the parser
(milestone 03), with real source positions from the start.

## In scope

- Literals: integers, floats, strings (with `\" \\ \n \t \r` escapes).
- Identifiers, Unicode-aware (`café`, `π`, `_x` all lex fine).
- Keywords: `let fn return if else true false`.
- Operators: `+ - * / = == != < >`.
- Punctuation: `( ) { } , : ->`.
- Line comments (`//`), skipped along with whitespace.
- 1-indexed line/column tracking on every token.
- Errors, each carrying a span: unterminated string, unknown character,
  malformed number literal (e.g. `1.2.3`).

## Out of scope (later milestones, or deliberately not needed yet)

- Statement/newline significance — a parser concern (milestone 03).
  Newlines are pure whitespace at the lexer level.
- Block comments, hex/exponent/underscore-separated numeric literals.
- Full escape-sequence validation — an unrecognized `\x` escape is kept
  literally rather than erroring.
- A number literal directly followed by identifier characters (`12abc`)
  is accepted as two adjacent tokens rather than flagged; not a case
  called out by this milestone.
- Multi-error collection / error recovery. Explicitly listed as a hard
  problem in `ROADMAP.md`; this lexer fails fast on the first error,
  matching `tokenize()`'s `Result<Vec<Token>, LexError>` signature.
- Any AI-specific syntax (`infer`, `tool`, `Distribution<T>`, `budget`,
  etc.) — added only when the parser milestones that need them arrive.

## Design decisions

- **`Position`/`Span` live in `aint-ast`**, not `aint-lexer`. They're
  generic source-location types the parser will need for AST nodes too,
  and `aint-ast` is the crate designated for shared, logic-free types.
  This gives `aint-lexer` its first real dependency (on `aint-ast`),
  consistent with the dependency direction in `docs/ARCHITECTURE.md`.
- **`Lexer` is an iterator** (`Iterator<Item = Result<Token, LexError>>`)
  ending in exactly one `Eof` token, plus a `tokenize()` convenience
  function that collects it into `Result<Vec<Token>, LexError>`. This
  keeps both a streaming and a simple all-at-once API available without
  two separate implementations.
- **A number followed by another `.`** (`1.2.3`) is treated as one
  malformed-number token spanning the whole run, rather than a valid
  float followed by an "unexpected character" error on the stray `.` —
  gives a more honest diagnostic for the obvious typo case.
