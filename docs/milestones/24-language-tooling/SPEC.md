# Milestone 24 — Language tooling

## Scope

`ROADMAP.md`:

> `aint fmt`, `aint check`, LSP, editor extension, syntax
> highlighting, autocomplete, go-to-definition, debugger.

Eight names. This milestone builds the four that are genuinely
independent, self-contained deliverables — `aint check`, `aint fmt`,
syntax highlighting, and a minimal, loadable (if unpublished) editor
extension shell to carry that highlighting — and states plainly why
the remaining four (LSP, autocomplete, go-to-definition, debugger) are
a distinct, larger effort rather than half-built here.

## What this milestone actually builds

**`aint check <file>`**: exposes the exact `parse_and_check` gate
`run`/`test` already share as its own subcommand — parse and
type-check, no execution, silent on success (matching `gofmt
-l`/`tsc --noEmit`'s convention that silence means "it's fine").
Genuinely small: the gate already existed, this only gives it a name.

**`aint fmt <file>` / `aint fmt --check <file>`**: a new crate,
`aint-fmt` — a canonical, deterministic pretty-printer,
`Program -> String`, plus the CLI command that parses, reformats, and
writes back in place (`--check` reports without writing, for CI,
matching `rustfmt --check`).

Two correctness properties are what actually matter for a formatter,
more than matching any particular hand-written style, and both are
tested against every real, shipped `.an` example, not just curated
snippets:

- **Idempotent**: `format(format(src)) == format(src)`.
- **AST-preserving**: re-parsing the formatted output produces the
  same AST (everything except source positions, which necessarily
  move) as parsing the original did — checked with a real,
  span-insensitive structural comparator walking both trees, not
  approximated by "it still parses."

Getting these right surfaced two real, non-obvious correctness traps,
both fixed before either property held:

- **Float formatting.** Rust's default `Display` for `2.0_f64` is
  `"2"` — printing that directly would re-lex as an `Integer`, not a
  `Float`, silently changing the program's types. Fixed by always
  emitting a decimal point.
- **String escaping.** Rust's `Debug` escaping for `String` also
  emits `\u{...}` for other control characters, which `aint-lexer`'s
  `lex_string` doesn't recognize as an escape at all (it would keep
  `\u` literally in the value). Fixed by writing an escaper that
  only uses exactly the five escapes the lexer actually understands
  (`"`, `\`, `\n`, `\t`, `\r`).

**Blank lines are preserved, not invented.** Early on, the printer
tried a rule-based heuristic ("blank line between every top-level
statement except consecutive `import`s") and it was visibly wrong —
`showcase.an`'s tightly-grouped `let`/`print` sequences don't read
that way in the author's own layout. The fix: compare each
statement's span line number against the previous one's; a source gap
of one or more blank lines becomes exactly one blank line in the
output, a gap of zero stays zero. This is what gofmt/rustfmt actually
do, for the same reason — a formatter that invents its own spacing
policy fights every author's intentional grouping instead of
respecting it.

**Known limitation, checked for and refused rather than silently
triggered**: `aint-lexer` discards `//` comments entirely (they never
become tokens, so they never reach the AST this formatter prints
from). Formatting a file containing one would silently delete it.
Instead, `format` scans the raw source first (correctly skipping `//`
that appears *inside* a string literal, mirroring the lexer's own
escape handling) and refuses outright, file untouched, with a message
pointing at this limitation, if it finds a real comment.
`examples/async.an` — the one shipped example with a real comment —
is used directly to verify this refusal.

**Syntax highlighting**: a TextMate grammar
(`editors/vscode/syntaxes/aint.tmLanguage.json`) covering every
keyword (`let`, `fn`, `if`/`else`, `return`, `import`, `async`/
`await`, `infer`, `tool`, `enum`, `effects`, `test`, `mock`, `assert`,
`budget`, `permissions`), built-in and generic types (`Int`, `Float`,
`Bool`, `String`, `Unit`, `List<T>`, `Option<T>`, `Task<T>`,
`Inference<T>`, `Distribution<T>`, `Tool<T>`), string/number/comment
literals, and operators — plus `language-configuration.json` (bracket
matching, comment toggling, auto-closing pairs) and a minimal
`package.json` that makes `editors/vscode/` a real, locally-loadable
VS Code extension (installable via "Install from VSIX" or VS Code's
Extension Development Host) even though it's never published to a
marketplace.

## Explicitly out of scope

- **LSP, autocomplete, go-to-definition.** All three need real
  semantic indexing — a symbol table over a parsed program that can
  answer "what is this identifier" and "where was it declared," which
  nothing in the compiler pipeline builds or exposes today (the type
  checker's own `scopes`/`Binding` machinery is internal and
  discarded once `check_program` returns). Beyond that, an LSP is a
  genuinely different execution model from every command this CLI has
  had so far — a long-running JSON-RPC server over stdio, not a
  one-shot parse-check-run-and-exit — and would need new,
  previously-unused dependencies (`lsp-server`/`lsp-types`, the same
  crates rust-analyzer itself is built on) rather than anything
  already in this workspace. `aint check` already delivers an LSP's
  *diagnostics* feature's actual value (fast, accurate error
  reporting) as a one-shot CLI command; wrapping that specific
  feature in the LSP protocol so it appears live in an editor is real,
  valuable, and substantial enough to be its own effort rather than
  bolted onto this milestone's remaining scope.
- **Publishing the editor extension.** `editors/vscode/` is a real,
  loadable extension folder, not a marketplace listing — no icon,
  README, publisher registration, versioning policy, or CI packaging
  pipeline. That's presentation and distribution work, not language
  tooling.
- **A debugger.** The Debug Adapter Protocol is a full second
  protocol implementation (breakpoints, stepping, variable
  inspection, call-stack reporting) on top of a running interpreter or
  VM — substantial, standalone engineering disproportionate to a
  slice of this milestone alongside everything else here.

## Design decisions

**`aint-fmt` is a new crate**, not CLI-only logic — matching every
other pipeline-adjacent concern in this workspace (`aint-ir`,
`aint-vm`, `aint-package`) getting its own crate so it's testable in
isolation from argument parsing.

**The formatter refuses rather than guesses or silently drops
content.** The comment limitation is real; the response to hitting it
is the same "fail clearly, never silently" instinct that's run through
every previous milestone (`CompileError::Unsupported` in 22, budget
enforcement in 17, and so on) — a formatter that quietly deletes a
user's comment on first use would be far worse than one that refuses
and explains why.

**Structural equality for the AST-preservation test is hand-written,
not derived.** `Stmt`/`Expr`'s derived `PartialEq` includes `span`,
which always differs after reformatting even when nothing else does;
a dedicated, span-insensitive recursive comparator (in the test file,
not the library — nothing at runtime needs this) is what actually
proves the property that matters.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
