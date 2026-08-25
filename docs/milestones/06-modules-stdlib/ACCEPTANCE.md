# Milestone 06 — Modules + standard library — acceptance

## Scope

See `SPEC.md`. `import` gating stdlib access, plus minimal immutable
list literals/indexing to make `List<T>` (introduced as a type in
milestone 05) actually usable.

## Acceptance criteria

- [x] `import` lexes as a keyword; `[`/`]` lex correctly.
- [x] `import math` parses to `StmtKind::Import("math")`.
- [x] `[1, 2, 3]` parses to `ExprKind::List` with 3 elements; `list[0]`
      parses to `ExprKind::Index`; chained indexing (`list[0][1]`) and
      arbitrary index expressions (`list[i + 1]`) both parse; indexing
      and calls compose (`f()[0]`).
- [x] `[1, 2, 3]` infers `List<Int>`; an empty list literal, and a list
      with mismatched element types, are each positioned type errors;
      indexing a non-list, or indexing with a non-Int, are positioned
      type errors; `list[i]` infers the list's element type.
- [x] Calling a stdlib function before its module is imported is
      `UndefinedFunction`; after `import math`, `math_sqrt(4.0)`
      type-checks as `Float` and wrong argument type/count are still
      caught; `collections_length` works for `List<Int>`, `List<String>`,
      etc. without any generics mechanism; `import frobnicate` is a
      positioned `UnknownModule` error.
- [x] List literals evaluate to `Value::List`; indexing returns the
      right element; an out-of-range or negative index is a positioned
      `IndexOutOfBounds` naming both the index and the list's actual
      length.
- [x] Each of the 15 new stdlib functions (8 `math`, 6 `string`, 1
      `time`) computes the right result for at least one case;
      `collections_length` works for lists of any element type;
      importing an unknown module is a positioned `RuntimeError` too
      (defense in depth, matching the type checker's own check);
      `time_now_seconds` returns a plausible, current Unix timestamp.
- [x] `examples/stdlib.an` combines `import`, list literals, indexing,
      recursion (no loops exist, so this is how a list gets summed),
      and `math`/`string`/`collections`/`time` — runs correctly through
      the actual built `aint` binary, not just library tests.
- [x] `cargo test --workspace` passes with no regressions to any
      earlier milestone's tests.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — `io` beyond `print`, `json`, `http`, `Option<T>`
construction, mutable lists, general collection operations
(`map`/`filter`/`reduce`), empty list literals, and loops are all
deferred with documented reasoning.

## Outcome

Satisfied by `crates/lexer/src/{token,lexer}.rs` (`import` keyword,
bracket tokens), `crates/ast/src/{expr,stmt}.rs` (`ExprKind::{List,
Index}`, `StmtKind::Import`), `crates/parser/src/parser.rs`
(`parse_import_statement`, list literals, `parse_postfix` renamed from
`parse_call` to cover indexing too), `crates/typechecker/src/{stdlib,
checker,error}.rs` (new `stdlib.rs`), `crates/runtime/src/{stdlib,value,
interpreter,error}.rs` (new `stdlib.rs`), `crates/cli/src/main.rs`
(unchanged — `import`/lists needed no CLI wiring beyond what milestones
04-05 already built), and `examples/stdlib.an`. 112 tests total across
the workspace, all passing: 28 new/updated in `aint-typechecker`, 28 in
`aint-runtime` (10 new), 30 in `aint-parser` (6 new), 4 CLI subprocess
tests (1 new).

One bug caught and fixed along the way, worth noting since it's the
second time it's happened: a hand-written `>=` in a test/example
(`if i >= collections_length(xs)`) — AINT only has `<`/`>`, no
`<=`/`>=` (milestone 02's operator set never included them). Both
instances were rewritten with `if ... { } else { }` instead of adding
the missing operators, which stayed out of this milestone's scope.
