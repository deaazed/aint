# Milestone 30 — Closures — acceptance

## Scope

See `SPEC.md`. `fn(...) -> T { ... }` lambda expressions, a closure's
type (`fn(Type, Type) -> Type`), calling a closure through any
expression (not just a bare name), a plain top-level `fn` decaying to a
closure value when referenced bare, and real reference-capture in the
interpreter — minus generics/structs/traits, `async` lambdas, and VM/IR
execution, all named directly as out of scope.

## Acceptance criteria

- [x] AST: `Type::Function(Vec<Type>, Box<Type>)`
      (`crates/ast/src/ty.rs`, with a `Display` impl printing
      `fn(T1, T2) -> R`) and `ExprKind::Lambda { params, return_type,
      body }` (`crates/ast/src/expr.rs`).
- [x] Parser: `fn(...) -> T { ... }` parses as an expression
      (`parse_lambda_expr`, hooked into `parse_primary`); `fn(Type,
      Type) -> Type` parses as a type (`parse_function_type`, hooked
      into `parse_type` ahead of its identifier-only path). Both
      covered by direct parser tests, including a function-typed
      parameter.
- [x] Typechecker: a lambda body is checked in its own scope (params
      bound, `current_return_type`/`current_effects` swapped exactly
      like a top-level `fn`'s, missing-return checked via the existing
      `definitely_returns`); a bare reference to a `Sync`-mode
      `Binding::Function` decays to `Type::Function`; `async fn`/
      `infer`/`tool`/`Polymorphic*` references are still rejected
      exactly as before.
- [x] `check_call` handles three call shapes now: the unchanged named-
      function fast path (identifier resolving to `Binding::Function`/
      `Polymorphic*`, all four `CallMode`s untouched), an identifier
      resolving to a closure-typed variable (`check_closure_call`,
      keeping `TypeError::NotAFunction` for a non-callable named
      variable), and any non-identifier callee (`check_call_to_value`,
      an index/immediately-invoked-lambda/nested-call result).
- [x] Calling a closure checks arity and argument types and rejects a
      call from inside any `effects [...]`-declared function — verified
      directly, including a higher-order case (closure passed as a
      parameter, called from inside a `pure` function, rejected).
- [x] Interpreter: `Function` gains `captured_env: Rc<RefCell<
      Environment>>`; a top-level `fn` still captures `globals` (byte-
      identical behavior to before this milestone); a lambda captures
      whatever scope is live where it's evaluated; `run_function` now
      parents the call frame to `function.captured_env`, not always
      `globals`. `Function`'s `PartialEq` is now manual (declaration
      only — name/params/body/`is_async` — since `Environment` isn't,
      and shouldn't be, comparable).
- [x] Escaping-closure capture verified directly: a closure returned
      from a function still sees that function's own (long-returned)
      local parameter correctly, for two independently-created closures
      with different captured values in the same test.
- [x] Closures stored in a `List` and called by index, and an
      immediately-invoked lambda, both verified directly at the
      interpreter level.
- [x] `aint-ir`/`aint-vm`: a lambda expression fails clearly at IR
      lowering (`LowerError::UnsupportedLambda`) rather than reaching
      AIR; calling a named closure-holding variable was already safe by
      construction (`CompileError::UndefinedName`, since the VM
      compiler's call-target table only ever contains real top-level
      functions/natives) — verified through the real binary against
      `examples/closures.an`, confirming non-zero exit, empty stdout,
      and a clear message either way.
- [x] `aint-fmt`: lambda expressions print back correctly (reuses the
      existing `params`/`block` printer helpers and `Type`'s `Display`
      impl); the fmt-test AST-equality helper covers `ExprKind::Lambda`
      structurally.
- [x] New example `examples/closures.an`: an escaping closure
      (`make_adder`), a closure passed as a higher-order argument
      (`apply_twice`), closures stored in a `List` and called by index,
      and a plain `fn` decaying to a closure value — verified through
      the real binary: `aint check`, `aint run` (exact stdout: `6 11 12
      6 25 42`), `aint test` (its one test block passes), and
      `aint run --vm` (fails clearly with `UnsupportedLambda`, non-zero
      exit, empty stdout).
- [x] `cargo test --workspace` passes with no regressions: 415 tests
      total, up from 399 before this milestone (16 new: 8 typechecker,
      3 interpreter, 3 CLI integration, 2 parser).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Known, honestly-stated gaps

- **The bytecode VM and IR compiler don't execute closures at all** —
  a lambda fails at lowering; a named closure-holding variable fails at
  VM compilation. Both documented parity gaps, not attempted here. See
  `SPEC.md`'s "Interpreter-only."
- **`async` lambdas don't exist** — every lambda is synchronous;
  closures don't interoperate with `Task<T>`/`Inference<T>`/`Tool<T>`
  at all.
- **No generics/structs/traits** — this milestone deliberately stayed
  the smallest lever; see `ROADMAP.md`'s Phase 2 framing for why.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by AST/parser/typechecker/interpreter changes across
`crates/ast`, `crates/parser`, `crates/typechecker`, `crates/runtime`,
one defensive `LowerError` variant in `crates/ir`, real lambda-printing
support in `crates/fmt`, and `examples/closures.an` verified end to end
through `aint check`/`run`/`test`/`run --vm`. 415 tests total across the
workspace, all passing.
