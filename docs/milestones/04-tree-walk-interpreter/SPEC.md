# Milestone 04 — Tree-walk interpreter — spec

## Scope

Make `aint run <file>` actually execute a program: `let`, arithmetic,
`fn` definitions, `return`, recursion. No bytecode, no LLVM, no AI.

## In scope

**Parser additions** (deferred here from milestone 03 on purpose):
- `fn` definitions: `fn name(param, param) { ... }` — **untyped**, no
  `: Type` on params and no `-> ReturnType`. Type annotations are
  milestone 05's job; parsing them now would just mean re-parsing this
  syntax again once the type checker needs to attach to it.
- `return expr` — always takes a value. A bare `return` (implicitly
  returning `Unit`) is deferred: distinguishing bare `return` from
  `return` followed by an expression on the next line runs into the same
  newline-insignificance ambiguity milestone 03's `SPEC.md` already
  flagged, and no current program needs it.

**AST additions** (`aint-ast`): `StmtKind::Fn { name, params, body }`,
`StmtKind::Return(Expr)`.

**Runtime (`aint-runtime`):**
- `Value`: `Int`, `Float`, `String`, `Bool`, `Unit`, `Function`, and
  `Native` (currently just `print`).
- `Environment`: a `HashMap` plus an optional parent, `Rc<RefCell<_>>`
  throughout. Each function call and each `if` branch gets a fresh child
  environment.
- Statement execution: `let`, `fn` (binds a `Function` value), `return`
  (propagates a `Flow::Return` signal up through block execution),
  `if`/`else`, expression-statements.
- Expression evaluation: all of milestone 03's `ExprKind` variants,
  arithmetic/comparison requiring both operands to be the same numeric
  type (`Int`+`Int` or `Float`+`Float` — no implicit coercion), `==`/`!=`
  working across any types (structural equality, `false` for mismatched
  variants), integer division by zero as a `RuntimeError`, recursive
  function calls with arity checking.
- `print`: the only builtin. Interpreter is generic over `W: Write`
  (`Interpreter<W = io::Stdout>` by default) specifically so tests can
  capture what it wrote via `Interpreter::with_output(Vec::new())` /
  `.into_output()`, instead of asserting against real stdout.
- `RuntimeError`, following the same shape as `ParseError`: an enum with
  a `.span()` accessor and a `Display` starting with `{line}:{col}: `,
  so the CLI's `path:line:col: message` format falls out for free.

**CLI:** `aint run <file>` now actually lexes, parses, and interprets,
printing `path:line:col: message` on any lex/parse/runtime error.

**New example:** `examples/fibonacci.an`, since "recursion" is explicitly
part of this milestone's deliverable and `hello.an` doesn't exercise it.

## Out of scope (later milestones)

- Type annotations / static type checking — milestone 05. Everything
  here is dynamically checked at runtime (e.g. `1 + "x"` is a
  `RuntimeError`, not a compile error).
- `import`/modules, standard library beyond `print` — milestone 06.
- Real closures (capturing local variables from an enclosing scope at
  the point a function is declared) — not needed yet since `fn` can only
  be declared as a statement, never as a nested expression/lambda. Every
  function call's environment parents directly to *globals*, not to
  whatever scope was active when the function was defined. This is
  correct today (there's no other lexical scope a top-level `fn` could
  capture), not a shortcut — it'll need revisiting once nested/anonymous
  functions exist.
- String concatenation via `+` — not exercised by any current example;
  `+` is numeric-only for now.
- A general native-function registry — `print` is hardcoded as
  `NativeFunction::Print`, matched directly in `Interpreter::call`.
  Worth generalizing once milestone 06 needs more than one builtin, not
  before.
- `return` outside a function is a `RuntimeError` (`ReturnOutsideFunction`),
  caught at runtime, not rejected at parse time or by a semantic-analysis
  pass — there isn't one yet.

## Design decisions

- `Interpreter<W: Write = io::Stdout>` rather than hardcoding
  `println!` or taking a `Box<dyn Write>`: generic-over-writer means the
  interpreter can own the writer and hand it back via `into_output()`
  after running, which is a much simpler way to capture output in tests
  than any shared-ownership (`Rc<RefCell<Vec<u8>>>`) wrapper would be.
- Control flow (`return`) is a `Flow` enum (`Normal` / `Return(Value)`)
  threaded through `exec_stmt`/`exec_block`'s return type, not a Rust
  exception/panic. Tree-walk interpreters in a language without
  exceptions need *some* explicit signal for early return; this is the
  standard approach and keeps `RuntimeError` reserved for actual errors.
- Arithmetic/comparison require matching operand types rather than
  coercing `Int` to `Float`. Simpler, and consistent with the "strong
  static typing" principle in `LANGUAGE_DESIGN.md` — the type checker
  arriving in milestone 05 should end up enforcing the same rule
  statically, not contradict what the interpreter already does.
