# Milestone 24 — Language tooling — acceptance

## Scope

See `SPEC.md`. `aint check`, `aint fmt` (new `aint-fmt` crate),
syntax highlighting (TextMate grammar), and a minimal, locally-
loadable VS Code extension shell. LSP/autocomplete/go-to-definition,
marketplace publishing, and a debugger are explicitly deferred with
named reasons, not half-built.

## Acceptance criteria

- [x] `aint check <file>` (CLI): parses and type-checks without
      running; silent and exit 0 on success, exit non-zero with the
      real parse/type error on failure. Verified through the real
      binary for both a well-typed program (`showcase.an`) and an
      ill-typed one.
- [x] New crate `aint-fmt`: `format_program(&Program) -> String`
      (`printer.rs`) and `format(&str) -> Result<String, FormatError>`
      (`lib.rs`, parses first).
- [x] Every binary operator/precedence level is handled correctly —
      parentheses are inserted exactly when needed to preserve the
      original AST on re-parse, and *not* inserted when they'd be
      redundant (e.g. `(n / d) * d` reformats to `n / d * d`, since
      `/` and `*` share precedence and are left-associative — both
      parse identically).
- [x] Two real formatting-correctness bugs were caught and fixed
      before either property held: float literals losing their
      decimal point (would re-lex as `Integer`), and string escaping
      using Rust's own `Debug` rules instead of exactly the five
      escapes `aint-lexer` recognizes.
- [x] Blank-line handling was corrected from an invented
      category-based rule (visibly wrong against `showcase.an`'s own
      layout) to source-position-based preservation, matching
      gofmt/rustfmt's own approach.
- [x] `format` refuses (not silently strips) any file containing a
      real `//` comment, correctly distinguishing a comment from `//`
      appearing inside a string literal (including one after an
      escaped quote) — verified directly, and against
      `examples/async.an`, the one shipped file with a real comment.
- [x] **Every** shipped example file without a comment
      (`enums.an`, `fibonacci.an`, `hello.an`, `security.an`,
      `showcase.an`, `stdlib.an`, `testing.an`) passes both
      correctness properties: idempotent formatting, and
      AST-identical (via a hand-written, span-insensitive structural
      comparator, not just "still parses") on re-parse of the
      formatted output.
- [x] `aint fmt`/`aint fmt --check` (CLI): writes in place (unless
      already canonical, in which case it's a no-op); `--check`
      reports and exits non-zero without writing. Both verified
      through the real binary, including that a refused (commented)
      file is left byte-for-byte untouched.
- [x] Syntax highlighting: `editors/vscode/syntaxes/aint.tmLanguage.json`
      (every keyword, built-in/generic type, string/number/comment
      literal, operator), `editors/vscode/language-configuration.json`
      (brackets, comment toggling, auto-closing pairs), and
      `editors/vscode/package.json` making the folder a real,
      locally-loadable (if unpublished) VS Code extension.
- [x] `cargo test --workspace` passes with no regressions: 362 tests
      total, up from 350 before this milestone (12 new: 5 unit tests
      and 2 whole-example-suite integration tests in `aint-fmt`, 5 CLI
      integration tests for `check`/`fmt`).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are
      clean across the whole workspace, including the new crate.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — LSP (and the autocomplete/
go-to-definition features that ride on it), publishing the editor
extension to a marketplace, and a debugger, each with the specific
reason it's a distinct, larger effort rather than unstarted.

## Outcome

Satisfied by the new `crates/fmt` crate (`printer.rs`, `lib.rs`,
`tests/examples.rs`), `crates/cli/src/main.rs`'s new `check`/`fmt`
functions and `Command::Check`/`Command::Fmt` variants, and
`editors/vscode/` (`package.json`, `language-configuration.json`,
`syntaxes/aint.tmLanguage.json`). 362 tests total across the
workspace, all passing: 12 new, covering formatter correctness
(idempotency and AST-preservation against every real example, comment
refusal including the string-literal-containing-`//` edge case) and
the real `aint check`/`aint fmt`/`aint fmt --check` CLI paths end to
end.
