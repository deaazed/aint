# Milestone 04 — Tree-walk interpreter — acceptance

## Scope

See `SPEC.md`. Make `aint run <file>` actually execute a program.

## Acceptance criteria

- [x] `fn add(a, b) { return a + b }` parses to `StmtKind::Fn` with the
      right name/params/body; zero-param `fn` parses; `return expr`
      parses to `StmtKind::Return`.
- [x] `Environment.get` falls through to the parent when a name isn't
      local; a child environment's definitions don't leak into its
      parent; shadowing works.
- [x] `let` binds a value visible to later statements in the same
      scope; a `let` inside an `if`-block isn't visible after the block
      ends; `if` with and without `else` both execute the right branch.
- [x] A non-`Bool` `if` condition is a positioned `RuntimeError`.
- [x] `examples/fibonacci.an` → `fibonacci(10) == 55`, via real
      recursion (each call gets a fresh environment).
- [x] Calling with the wrong number of arguments is a positioned
      `ArityMismatch`; `return` outside any function is a positioned
      `ReturnOutsideFunction`.
- [x] Undefined variable, not-callable, type mismatch (`1 + "x"`), and
      integer division by zero each produce a positioned `RuntimeError`.
- [x] `aint run examples/hello.an` prints `Hello, AINT!` and exits 0;
      `aint run examples/fibonacci.an` prints `55` and exits 0 —
      verified against the actual built binary, not just library tests.
- [x] A program with a lex/parse/runtime error prints
      `path:line:col: message` to stderr and exits non-zero — verified
      against the actual binary for both a parse error and a runtime
      type error.
- [x] `cargo test -p aint-runtime` passes; fixtures cover both example
      programs.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — type checking, modules/stdlib beyond `print`, real
closures, string concatenation, a general native-function registry, and
compile-time rejection of `return` outside a function are all deferred.

## Outcome

Satisfied by `crates/ast/src/stmt.rs` (`Fn`/`Return` additions),
`crates/parser/src/parser.rs` (`fn`/`return` parsing),
`crates/runtime/src/{value,environment,error,interpreter,lib}.rs`,
`crates/cli/src/main.rs` (`run` wired up), and `examples/fibonacci.an`.
61 tests total across the workspace, all passing: 18 new in
`aint-runtime` (14 unit in `interpreter.rs`, 4 in `environment.rs`) plus
2 new integration tests (`hello.rs`, `fibonacci.rs`), on top of what
milestones 02-03 already had.
