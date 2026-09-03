# AINT — Language Specification

**Version 1.0.** This is the accurate, current reference for AINT as
implemented — every construct below exists in the compiler and
runtime today, checked against the source of truth (the `aint-*`
crates) while writing this document, not transcribed from an earlier
design sketch. Where a feature is documented as unsupported or
restricted, that reflects code, not an oversight in this document.

Read `LANGUAGE_DESIGN.md` first for *why*; this document is *what*,
precisely. See `docs/COMPATIBILITY.md` for what parts of this are
guaranteed to keep working across versions.

## 1. Source files

- Extension `.an`, UTF-8 encoded.
- A program is a flat sequence of top-level statements — no `main`
  function, no module wrapper. Execution runs top to bottom.
- `//` starts a line comment, extending to the end of the line. **Not
  preserved by `aint fmt`** — see §11.

## 2. Lexical structure

**Literals**: integers (`42`), floats (require a decimal point with a
digit on both sides — `2.0`, not `2.`), strings (`"..."`, with `\"`,
`\\`, `\n`, `\t`, `\r` recognized as escapes; any other `\x` is kept
literally, not an error), booleans (`true`/`false`).

**Identifiers**: `[A-Za-z_][A-Za-z0-9_]*`. Capitalization is a
convention, not enforced by the grammar: types are conventionally
capitalized (`Int`, `MyEnum`), values are conventionally lowercase
(`my_variable`), enum variant references follow `EnumName_Variant`
(`Sentiment_Positive`) — see §4.5.

**Keywords** (reserved, cannot be used as identifiers): `let`, `fn`,
`return`, `if`, `else`, `true`, `false`, `import`, `async`, `await`,
`infer`, `enum`, `tool`, `effects`, `test`, `mock`, `assert`,
`budget`, `permissions`.

**Operators**: `+` `-` `*` `/` `==` `!=` `<` `>` `=` `->`. There is no
`<=`, `>=`, `!`, `&&`, or `||`, and none are planned — see
`CONTRIBUTING.md`'s design constraints. Boolean negation and
"otherwise" logic are expressed with `if`/`else`.

## 3. Types

```
Int, Float, Bool, String, Unit
List<T>
Option<T>
Task<T>              — the type of an unawaited async fn call
Inference<T>          — the type of an unawaited infer call
Tool<T>                — the type of an unawaited tool call
Enum(name)             — a user-declared enum, compared nominally
Distribution<T>         — T must be an enum
fn(T, T, ...) -> T      — a closure's type (milestone 30)
```

`Task<T>`, `Inference<T>`, and `Tool<T>` are never written as source
syntax — the type checker computes them at a call-site's type when a
value isn't `await`-ed. `fn(...) -> T` is the one type spelling
written as source that isn't a bare identifier; see §4.3.

Static, nominal typing throughout. No implicit numeric coercion
(`Int`/`Float` are distinct; no arithmetic mixes them). Equality
(`==`/`!=`) is defined for any two values of the same type; comparing
values of different types is a type error at compile time, except
that `==`/`!=` between two different enum types is also rejected.

## 4. Statements

### 4.1 `let`

```
let name = expr
```

No type annotation (inferred from `expr`). No reassignment — `let`
introduces a new binding; there is no `name = new_value` statement
anywhere in the grammar. This is load-bearing: it's what makes
milestone 19's AIR-level call deduplication provably sound (two
syntactically identical calls in a block are guaranteed to see the
same argument values), and part of what makes `aint-vm`'s locals-on-
the-stack design possible.

### 4.2 `if` / `else`

```
if condition { ... }
if condition { ... } else { ... }
if condition { ... } else if condition { ... } else { ... }
```

The statement form's `else` is optional, and — since milestone 37 —
`else if` is real syntax: sugar for `else { if ... }`, applied at
parse time, so an arbitrarily long `else if` chain is one flat
sequence in the source with no extra nesting, even though the AST
underneath is still nested `if`s inside `else`. `condition` must be
`Bool`.

`if`/`else` is also usable as an *expression* (milestone 37):

```
let x = if condition { value } else { value }
let x = if condition { value } else if condition { value } else { value }
```

Each branch here is exactly one expression, not a block of statements
— `let`/`return`/other statements aren't allowed inside `{ }` in this
position — and `else` is required, not optional, since both branches
must produce a value of the same type. `else if` works the same way as
the statement form: the parser recurses directly into another
`if`-expression for `else_value` rather than requiring `{ }` around
it. This form is a genuinely separate AST node
(`ExprKind::If`, distinct from `StmtKind::If`) reachable only from
expression position — a bare `if` at the start of a statement always
parses as the statement form above, so every program written before
milestone 37 is unaffected. See
`docs/milestones/37-conditional-expressions/SPEC.md`.

### 4.3 `fn` / `async fn`

```
fn name(param: Type, ...) -> ReturnType { ... }
async fn name(param: Type, ...) -> ReturnType { ... }
    effects [ word, word, ... ]
```

`effects` is optional; its five words are `pure`, `inference`, `tool`,
`network`, `filesystem`. `pure` must appear alone. No `effects` clause
means *untracked*, not *pure* — an untracked function is incompatible
with being called from any function that does declare `effects`. A
`pure` function cannot call anything whose own effects aren't a
subset of `pure` (i.e., nothing at all beyond other `pure` calls and
stdlib functions, which are effect-exempt). `network`/`filesystem`
are accepted and checked the same way `inference`/`tool` are, but
nothing in the stdlib is currently tagged with either — see §9.

Calling an `async fn` without `await` produces a `Task<ReturnType>`
and does not run the body; `await`-ing it runs the body to
completion. There is no way to run a `Task` in the background without
eventually awaiting it — see `docs/milestones/25-real-application/
SPEC.md`'s "background jobs" finding.

**Closures (milestone 30).** `fn(params) -> ReturnType { ... }` in
*expression* position — no name, no `async`, no `effects` clause — is a
lambda: a first-class function value.

```an
let add_one = fn(x: Int) -> Int {
    return x + 1
}
print(add_one(4))
```

A lambda is always synchronous and untracked (exactly like a top-level
`fn` with no `effects` clause) — it can't be called from any
`effects [...]`-declared function, and its own body isn't
effect-checked against whatever function it's called from. A plain,
synchronous, non-`infer`/`tool` top-level `fn`, referenced bare (not in
call position), decays to the same closure value a lambda would;
`async fn`/`infer`/`tool` references bare are still rejected, since
calling the result would need `Task<T>`/`Inference<T>`/`Tool<T>` to
interoperate with closures, which doesn't happen. A closure value can
be called through any expression — an index into a `List<fn(...)
-> T>`, an immediately-invoked lambda, another call's result — not
just a bare name.

Capture is by reference to the closure's defining scope, sound because
nothing in AINT ever mutates a binding after creation — see
`docs/milestones/30-closures/SPEC.md` for the full argument. Closures
are interpreter-only: `aint run --vm` fails clearly (not silently) on
a lambda expression or a call to a closure-holding variable — see §11.

### 4.4 `return`

`return expr` (or bare `return` is not valid — every path returning a
non-`Unit` type must provide a value). A function declared to return
`Unit` need not return on every path; every other return type must.

### 4.5 `enum`

```
enum Name { Variant1 Variant2 ... }
```

At least one variant required. Variant values aren't a separate
expression form — `EnumName_Variant` (e.g. `Sentiment_Positive`) is
plain `ExprKind::Identifier` syntax, resolved to the enum value at
compile time. No associated data on variants.

### 4.6 `infer`

```
infer name(param: Type, ...) -> ReturnType
infer name(param: Type, ...) -> ReturnType permissions [tool_name, ...]
```

A signature-only declaration — no body; the implementation is
external (a `Model`). Calling it without `await` produces an
`Inference<ReturnType>`; `await`-ing it sends the request to whatever
`Model` the running `Interpreter` was constructed with (`MockModel` by
default — see §8 — or `HttpModel` if `AINT_MODEL_URL` is set for
`aint run`).

`permissions [...]` (optional) restricts which declared `tool`s this
inference's model conversation may request — every name must refer to
a declared `tool` (checked at compile time); omitting the clause means
unrestricted. Enforced twice at runtime: what's offered to the model
is filtered to the permitted set, and a request for anything outside
it is rejected independent of what was offered. See
`docs/milestones/20-security-model/SPEC.md`.

### 4.7 `tool`

```
tool name(param: Type, ...) -> ReturnType
tool name(param: Type, ...) -> ReturnType { body }
```

Signature structurally identical to `infer`'s. Calling it without
`await` produces a `Tool<ReturnType>`. A `tool` can be called two ways:
directly, from AINT code (`await my_tool(args)`), or requested by a
model mid-inference (only for a `tool` named in the calling `infer`'s
effective `available_tools`, per §4.6).

A body is optional (milestone 34). Without one, `await`-ing a tool
runs against `MockTool` — the only executor a signature-only tool has.
With one, `await`-ing it runs the body for real: ordinary AINT source,
type-checked exactly like a `fn` body, able to call anything a `fn`
can (stdlib functions included). An explicit `mock` for that tool name
still wins over a real body when one is configured — mocking a tool is
a statement that its real implementation shouldn't run for that test,
not a fallback only consulted when no real implementation exists.

### 4.8 `import`

```
import module_name
import "./path/to/file.an" as alias
import "package-name" as alias
```

The bare-identifier form binds every native function a stdlib module
provides into the current scope — see §9 for the full module list.

The string-literal form imports another `.an` file's declarations under
`alias_name`. A leading `./` or `../` (milestone 29) resolves relative
to the importing file's own directory. Anything else (milestone 36) is
a *package* import: resolved against the nearest `aint.toml`'s
`aint.lock` walking up from the entry file, importing that dependency's
`<path>/lib.an` — a package's library entry point, distinct from a
program's `main.an`, the same split Rust's `lib.rs`/`main.rs` draws. A
package import with no `aint.toml` above the entry file, an `aint.toml`
with no `aint.lock` next to it, or a name not present in `aint.lock`
each fail with their own specific, distinct error rather than one
generic "not found" — see `aint-loader::LoadError`'s
`NoPackageRoot`/`NoLockfile`/`UnknownPackage` variants.

Either form makes every top-level declaration in the imported file —
`fn`, `enum` (and its `EnumName_Variant` identifiers), `tool`, `infer`
— available under `alias_name`. Resolution happens entirely before
type-checking, in a separate `aint-loader` crate: the whole import
graph is flattened into one ordinary `Program` first, so the type
checker, interpreter, IR compiler, and VM never know a program came
from more than one file. A file reached through `import "..." as ...`
may only itself contain `fn`/`enum`/`tool`/`infer`/`import` at its top
level — no `let`, no `test`, no bare statement that would fire as a
side effect just because the file was imported. See
`docs/milestones/29-modularity/SPEC.md` and
`docs/milestones/36-git-dependencies/SPEC.md`.

### 4.9 `test` / `mock` / `assert` / `budget`

```
test "name" { ... }

mock function_name -> value

assert condition

budget {
    max_tokens = N
    max_model_calls = N
    max_cost = N.N
    timeout_ms = N
}
```

`test` blocks are inert during `aint run` — executed only by
`aint test`, each in its own fresh `Interpreter`, with every non-`Test`
top-level statement in the file re-run first (see the important
caveat in §10). `mock` is only valid inside a `test` block, and only
targets a declared `infer` or `tool`; its right-hand side is
restricted to a literal or an `EnumName_Variant` reference — not a
general expression (there is no source syntax for constructing a
`Distribution<T>` literal, an `Option<T>` value, or anything else more
structured). `assert` is a general statement, valid anywhere, not
test-only — a failing `assert` during `aint run` is a runtime error;
during `aint test`, it fails just that test. `budget` may appear at
most once per program; every field is optional (`None` = unlimited on
that axis); `max_tokens`/`max_cost` are checked but currently vacuous
in practice, since `TokenUsage` is always zero (no `Model`
implementation reports real usage yet) — see
`docs/milestones/17-ai-resource-management/SPEC.md`.

## 5. Expressions

```
Integer, Float, String, Bool literals
Identifier
-expr                            (unary negation)
!expr                            (unary Boolean negation, milestone 38)
left OP right                    (OP: + - * / == != < > <= >= && ||)
callee(arg, arg, ...)
[expr, expr, ...]                 (list literal)
expr[expr]                        (indexing)
await expr
fn(param: Type, ...) -> Type { ... }    (lambda, milestone 30 — see §4.3)
if condition { value } else { value }   (milestone 37 — see §4.2)
```

Precedence, lowest to highest: `||` < `&&` < `==`/`!=` < `<`/`>`/`<=`/
`>=` < `+`/`-` < `*`/`/` < unary `-`/`!`/`await` < calls/indexing/
literals. All binary operators are left-associative, including `&&`
and `||` — though associativity is moot for a pure Boolean-and/or pair
in practice. `&&` and `||` also **short-circuit** (milestone 38): the
right operand isn't evaluated at all once the left side already
decides the result — not just an evaluation-order guarantee, since a
right operand with an observable effect (a `tool`/`infer` call, or
simply code that would otherwise error, like `10 / x` when `x` is
statically unknown to be nonzero) genuinely never runs. Every other
binary operator always evaluates both operands. Calls and indexing are
freely mixable and left-associative (`f()[0]`, `list[0]()`).

**No `Option<T>`/`Distribution<T>` construction syntax.** These types
have values only via specific stdlib natives
(`distribution_require_confidence`, `option_is_some`/`option_unwrap`
consume but don't construct, etc.) — there is no `Some(x)`/`None`
expression form. See §11.

**No list concatenation.** `+` is defined only for `Int`/`Float`.
There is no `List<T> + List<T>`, no mutating append — a `List<T>`
value, once constructed by a list literal, cannot grow. See §11.

## 6. Execution model

Two independent executors:

- **The tree-walking interpreter** (`aint-runtime`, `aint run`) — the
  only one that supports `infer`/`tool`/`async`/`await`. Uses real Rust
  recursion per AINT-level call, which is why deep AINT recursion
  needs a large dedicated thread stack (`aint run`'s CLI, and every
  `aint-runtime` integration test, use a 64 MiB thread).
- **The bytecode VM** (`aint-vm`, `aint run --vm`) — covers AINT's
  full deterministic core (arithmetic, `let`, `if`/`else`, recursion,
  lists, indexing, enums, `assert`, every synchronous stdlib native)
  and nothing else: `infer`/`tool`/`await`/`async fn`/
  `Distribution<T>` operations are rejected with a specific,
  named `CompileError` at compile time, not silently mis-executed.
  Call frames live on the heap (`Vec<Frame>`), not the Rust stack, so
  deep recursion runs on an ordinary thread. On a compute-heavy
  recursive benchmark (`fibonacci(30)`), measured directly for this
  document: **~13x faster** than the tree-walking interpreter (0.29s
  vs. 3.93s, best-of-three, `--release`). Any program using only the
  deterministic core — `examples/customer_support/worker.an`, for
  instance — runs correctly under `--vm` with no changes, since the
  VM's native-call dispatch resolves against the same stdlib table the
  interpreter uses, generically, not a hardcoded subset.

`aint run` uses the tree-walking interpreter by default; `--vm` opts
into the VM. `aint test` always uses the tree-walking interpreter (test
bodies may use `infer`/`tool`).

## 7. Model backends

`aint run` uses `MockModel` (nothing configured, so every `infer`
call fails clearly) unless the environment variable `AINT_MODEL_URL`
is set, in which case it uses `HttpModel` against that URL
(`AINT_MODEL_NAME` for the model name, `AINT_MODEL_API_KEY` for a
bearer token) — any OpenAI-compatible chat completions endpoint
(vLLM, Ollama, OpenAI itself). `HttpModel` does not support tool
calling or `Distribution<T>` requests — see
`docs/milestones/16-model-adapters/SPEC.md`. `aint test` always uses
`MockModel`, configured per test block by that block's own `mock`
statements. Tool execution (`await my_tool(...)`, from any context)
always uses `MockTool` — no real tool backend exists; see §9.

## 8. Errors

Two error families, both positioned (`file:line:column: message`)
except where noted:

- **`TypeError`** (`aint-typechecker`) — raised by `parse_and_check`,
  before any execution. Includes undefined names, arity/type
  mismatches, effect violations, unknown types, an `assert`/`mock`
  outside its valid context, a `permissions` name that isn't a
  declared `tool`, and a duplicate `budget` block.
- **`RuntimeError`** (`aint-runtime`) — raised during execution.
  Includes `ModelError` (nothing configured to answer an `infer`),
  `SchemaViolation` (a model's answer, or a model-requested tool
  call's arguments, didn't match the declared type),
  `PermissionDenied` (a tool request outside an `infer`'s
  `permissions`), `BudgetExceeded`, `AssertionFailed`, and the
  ordinary set (division by zero, index out of bounds, arity
  mismatch, undefined variable). VM-produced `RuntimeError`s use a
  placeholder span (`1:1`) instead of a real position — AIR carries no
  source positions; see `docs/milestones/22-bytecode-vm/SPEC.md`.

`aint check` runs the `TypeError` gate only, without executing.
`aint fmt` adds one more failure mode of its own: refusing to format
a file containing a `//` comment, rather than silently deleting it —
see §11.

## 9. Standard library

Gated behind `import <module>`; `print` is always available,
ungated.

| Module | Functions |
|---|---|
| `math` | `math_sqrt`, `math_pow`, `math_floor`, `math_ceil`, `math_round`, `math_abs`, `math_min`, `math_max` |
| `string` | `string_length`, `string_to_upper`, `string_to_lower`, `string_trim`, `string_contains`, `string_concat`, `string_split(s, sep) -> List<String>` (milestone 31), `string_replace(s, target, replacement) -> String` (milestone 39 — every occurrence; an empty `target` leaves `s` unchanged), `string_url_decode(s) -> String` (milestone 40 — strict RFC 3986 percent-decoding; `+` is left alone, compose `string_replace(s, "+", " ")` first for query-string decoding) |
| `time` | `time_now_seconds`, `time_sleep_ms` (async — the one async native before milestone 25) |
| `collections` | `collections_length` (polymorphic over `List<T>`) |
| `distribution` | `distribution_probability`, `distribution_argmax`, `distribution_entropy`, `distribution_sample`, `distribution_require_confidence` |
| `option` | `option_is_some`, `option_unwrap` |
| `json` | `json_get(json, key) -> Option<String>`, `json_object(keys, values) -> String` — flat objects only, string-valued fields, no nesting, no arrays |
| `db` | `db_insert`, `db_get -> Option<String>`, `db_list -> List<String>`, `db_update`, `db_delete` — file-backed, `.aintdb/<table>.jsonl`; table names are validated against `[A-Za-z0-9_-]+` (milestone 28's security pass — a real path-traversal vulnerability existed before this check) |
| `auth` | `auth_hash_password`/`auth_verify_password` (real `bcrypt`), `auth_generate_token` (real randomness) |
| `log` | `log_info`, `log_error` — timestamped lines to stderr |
| `http` | `http_serve(port)` (async) — a hand-rolled HTTP/1.1 server over a raw `TcpListener`, one connection at a time; dispatches every request to a program-defined `handle_request(method: String, path: String, body: String) -> String`; no router (see `docs/milestones/25-real-application/SPEC.md` for why) |

No `Int`/`String` conversion exists anywhere in the stdlib. `print`
accepts any value type (via `Display`), which is the only way to
render a non-`String` value without hand-writing a match over it.

## 10. Testing model

`aint test` gives each `test` block its own fresh `Interpreter`,
re-running every non-`Test` top-level statement first for isolation.
**This means a file cannot contain both its own `test` blocks and a
blocking top-level statement** (`await http_serve(...)`, most notably)
— the first test run would re-execute the server start and hang
forever. `examples/customer_support/`'s own split
(`priority_logic_test.an` duplicating logic out of `server.an`) exists
specifically because of this; see
`docs/milestones/25-real-application/SPEC.md`.

## 11. Known gaps

Stated here so they're findable in one place, not scattered across 20
milestones' `SPEC.md` files (though each is documented in full where
it was found):

- **No diamond imports.** `import "path" as alias` (milestone 29)
  resolves multi-file programs, but the same file may only be imported
  from exactly one place in the whole program — a second import of the
  same file anywhere else in the graph is a clear
  `aint-loader::LoadError::DuplicateImport`, not a silent second copy.
  (`docs/milestones/29-modularity/SPEC.md`)
- **Cross-file error positions are approximate.** An error inside code
  spliced in from an imported file is reported against the *entry*
  file's path, with a line/column that's actually relative to the
  imported file's own source — `Span` carries no file identity yet.
  (same)
- **Closures don't run under `aint run --vm`.** A lambda expression
  fails clearly at IR lowering; calling a closure-holding variable by
  name fails clearly at VM compilation (it was never in the VM's
  compile-time function table). Both documented parity gaps, not
  attempted. (§4.3, `docs/milestones/30-closures/SPEC.md`)
- **`if`/`else` used as an expression doesn't run under `aint run
  --vm`.** Fails clearly at IR lowering (`LowerError::UnsupportedIfExpr`),
  same reasoning and same shape as the closures gap above — the
  *statement* form of `if`/`else` is unaffected either way. (§4.2,
  `docs/milestones/37-conditional-expressions/SPEC.md`)
- **`&&`/`||` don't run under `aint run --vm`.** Short-circuit
  evaluation needs real conditional-jump bytecode, not the "evaluate
  both operands, then apply the operator" shape every other binary
  operator compiles to — fails clearly at IR lowering
  (`LowerError::UnsupportedShortCircuit`), same shape as the closures
  and if-expression gaps above. `<=`/`>=`/`!` have no such gap — they
  need no short-circuiting, so they run under the VM exactly like every
  other comparison/unary operator already did. (§5,
  `docs/milestones/38-comparison-and-logical-operators/SPEC.md`)
- **No generics, structs, or interfaces/traits.** Closures (milestone
  30) were deliberately the smallest lever for passing behavior around
  — these stay out of scope until real framework-building shows what's
  actually needed. (`ROADMAP.md`'s Phase 2 framing)
- **No `Option<T>`/`Distribution<T>` construction syntax** — only
  specific natives produce them. (§5,
  `docs/milestones/25-real-application/SPEC.md`)
- **No list concatenation or incremental list construction.**
  (§5, same)
- **No `Int`/`String` conversion.** (§9, same)
- **`aint test` cannot exercise a file with a blocking top-level
  statement.** (§10, same)
- **A tool's real implementation is always synchronous, and can't run
  under `aint run --vm`.** `await` — the only way to invoke a tool
  call at all — is unconditionally unsupported by the bytecode VM,
  same as every other AI-facing operation. (§4.7, §6,
  `docs/milestones/34-real-tools/SPEC.md`)
- **`aint fmt` doesn't preserve comments** — refuses rather than
  deletes them. (§2, `docs/milestones/24-language-tooling/SPEC.md`)
- **No LSP, autocomplete, or go-to-definition** — need real semantic
  indexing nothing in the pipeline exposes yet.
  (`docs/milestones/24-language-tooling/SPEC.md`)
- **No hosted package registry** — `aint add` takes a local path or a
  git URL (milestone 36), never a bare name looked up somewhere; there's
  still no server, database, or domain, and none is planned. A
  name → URL index (so `aint add some-lib` works without a real URL)
  is real, additive work, not attempted.
  (`docs/milestones/23-package-manager/SPEC.md`,
  `docs/milestones/36-git-dependencies/SPEC.md`)
- **`http_serve` handles one connection at a time**, by construction —
  real concurrency would need either `tokio::task::spawn_local` under
  a `LocalSet` or moving `Value` off `Rc`, neither attempted.
  (`docs/milestones/25-real-application/SPEC.md`)
- **`aint run`/`aint test` never read a `.env` file** — every real
  model call needs `AINT_MODEL_URL`/`AINT_MODEL_NAME`/
  `AINT_MODEL_API_KEY` exported by hand first. (`ROADMAP.md`'s Phase 3
  framing, milestone 41, not started)
