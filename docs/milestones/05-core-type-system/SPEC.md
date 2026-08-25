# Milestone 05 — Core type system — spec

## Scope

Make `fn` signatures typed, add a real static type checker
(`aint-typechecker`), and wire it into `aint run` so
`add("hello", true)`-style programs are rejected *before* anything
executes — not caught mid-run the way milestone 04's interpreter caught
type errors.

## In scope

**Types (`aint-ast`):** `Type::{Int, Float, Bool, String, Unit,
List(Box<Type>), Option(Box<Type>)}`. Parsed from identifier tokens in
type-annotation position (`Int`, `List`, ...) — these aren't lexer
keywords, just recognized names, so milestone 02's lexer needs no
changes. `List<T>`/`Option<T>` reuse the existing `<`/`>` tokens.

**Parser changes:** `fn` signatures now require types —
`fn name(param: Type, ...) -> Type { ... }`. Both the param types and
the return type are mandatory, no inference on signatures (`let` keeps
inferring from its initializer, unchanged). An identifier in
type-annotation position that isn't one of the seven recognized names is
a **parse** error (`expected a type`), not a type-check error — see
"Design decisions."

**Type checker (`aint-typechecker`):**
- Expression type inference for every `ExprKind` variant.
- Statement checking for `let` (infers and binds), `if`/`else` (condition
  must be `Bool`; each branch type-checks in its own scope, mirroring
  the interpreter's block scoping), `return`, expression-statements.
- Function calls: arity and per-argument type checking against the
  callee's declared signature. `print` is special-cased (see below).
- `return` type-checked against the enclosing function's declared return
  type; `return` outside any function is a compile-time error now, not
  just the runtime `ReturnOutsideFunction` from milestone 04.
- **Missing-return analysis**: if a function's return type isn't `Unit`,
  every path through its body must end in `return`. Basic reachability
  over `if`/`else` (an `if` without `else` never counts — the false path
  falls through), no loops to worry about since we don't have any yet.
- Top-level `fn` signatures are hoisted into scope in a pass before
  bodies are checked, so forward references and mutual/self recursion
  between top-level functions all type-check regardless of source order.
  Non-top-level (block-nested) `fn` declarations are *not* hoisted —
  they're bound sequentially like `let`, matching how nested `fn` was
  already the more natural/least-special reading in milestone 04.

**CLI:** `aint run` now runs `aint_typechecker::check_program` between
parsing and interpreting. A type error prints `path:line:col: message`
and the program **never reaches the interpreter** — verified by an
actual subprocess test asserting stdout is empty when type-checking
fails, not just that the exit code is non-zero.

**Updated `fn` syntax everywhere it appears:** `examples/fibonacci.an`,
and every test across `aint-parser`/`aint-runtime` that declares a `fn`,
now use typed signatures (`fn fibonacci(n: Int) -> Int`). `examples/hello.an`
is untouched — it never declares a function.

## Out of scope (later milestones)

- Actually constructing `List<T>`/`Option<T>` values — no list-literal
  syntax, no indexing, no `Some`/`None`. The type vocabulary exists and
  type-checks structurally (`fn f(x: List<Int>) -> Int` parses and
  checks fine), but nothing in the language can produce such a value
  yet. That's collections/stdlib work (milestone 06 territory), not
  this milestone's job — adding literal syntax now would be scope well
  beyond "core type system."
- Type annotations on `let` (`let x: Int = 42`). Locals stay inferred
  from their initializer; only function signatures require annotations.
- User-defined named types (structs, enums like the `enum Intent`
  examples in `docs/LANGUAGE_DESIGN.md`) — not until a milestone
  actually introduces custom type declarations.
- Generics beyond the two hardcoded `List<T>`/`Option<T>` wrappers — no
  general type-parameter mechanism yet.
- A general native-function type signature mechanism. `print` remains
  hardcoded (accepts exactly one argument of any type, returns `Unit`),
  matching how it's already hardcoded in the interpreter. See milestone
  04's `SPEC.md` for why a real registry waits for milestone 06.
- Multi-error collection — the type checker fails fast on the first
  error, same as every earlier pass.

## Design decisions

- **Unknown type names are a parse error, not a type-check error.**
  `parse_type()` is the only place that turns an identifier into a
  `Type`, and it already rejects anything that isn't one of the seven
  recognized names. By the time a `Type` value reaches the type checker,
  it's structurally guaranteed to be well-formed — there's no
  "`UnknownType`" case to handle there. Simpler than validating type
  names twice in two different passes.
- **`==`/`!=` require matching operand types statically**, stricter than
  the interpreter's own runtime behavior (which permissively returns
  `false` for a cross-type comparison — see milestone 04's `SPEC.md`).
  This isn't a contradiction: once type-checking is mandatory, the
  interpreter's permissive fallback only exists as defense in depth for
  a program the type checker already would have rejected. Comparing an
  `Int` to a `String` is essentially always a mistake and worth catching
  before the program runs, the same as `Int + String` already is.
- **Two-pass top-level checking** (hoist all top-level `fn` signatures,
  then check every statement) rather than a single sequential pass.
  Without it, a function could only call top-level functions declared
  *before* it in the file, which would make ordinary mutual recursion
  impossible and be a constant surprise. This is the standard behavior
  for top-level declarations in most languages with them, not an
  embellishment.
- **Type-checker scopes are a plain `Vec<HashMap<String, Binding>>`**,
  not `Rc<RefCell<_>>` like the interpreter's `Environment`. The checker
  is a single-pass tree walk with no need for a scope to outlive the
  call that created it (nothing here is a closure), so the extra
  shared-ownership machinery the interpreter needs would be pure
  overhead here.
