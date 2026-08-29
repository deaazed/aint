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
```

`Task<T>`, `Inference<T>`, and `Tool<T>` are never written as source
syntax — the type checker computes them at a call-site's type when a
value isn't `await`-ed.

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
```

`else` is always followed by a block — there is no `else if` as its
own syntax; a chain is written as nested `if` inside the `else`
block. `condition` must be `Bool`.

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
```

Structurally identical to `infer`'s signature — also no body. Calling
it without `await` produces a `Tool<ReturnType>`; `await`-ing it runs
against `MockTool` (the only `tool` executor that exists — see §9,
"Known gaps"). A `tool` can be called two ways: directly, from AINT
code (`await my_tool(args)`), or requested by a model mid-inference
(only for a `tool` named in the calling `infer`'s effective
`available_tools`, per §4.6).

### 4.8 `import`

```
import module_name
```

Binds every native function a stdlib module provides into the current
scope. See §9 for the full module list. There is no way to `import`
another `.an` file — every `import` target is one of the fixed stdlib
module names. A user-authored multi-file program does not exist in
AINT today; see §11.

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
-expr                          (unary negation only)
left OP right                   (OP: + - * / == != < >)
callee(arg, arg, ...)
[expr, expr, ...]                (list literal)
expr[expr]                       (indexing)
await expr
```

Precedence, lowest to highest: `==`/`!=` < `<`/`>` < `+`/`-` <
`*`/`/` < unary `-`/`await` < calls/indexing/literals. All binary
operators are left-associative. Calls and indexing are freely
mixable and left-associative (`f()[0]`, `list[0]()`).

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
| `string` | `string_length`, `string_to_upper`, `string_to_lower`, `string_trim`, `string_contains`, `string_concat` |
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

- **No cross-file `import`.** Every AINT program is one file.
  (`docs/milestones/23-package-manager/SPEC.md`,
  `docs/milestones/25-real-application/SPEC.md`)
- **No `Option<T>`/`Distribution<T>` construction syntax** — only
  specific natives produce them. (§5,
  `docs/milestones/25-real-application/SPEC.md`)
- **No list concatenation or incremental list construction.**
  (§5, same)
- **No `Int`/`String` conversion.** (§9, same)
- **`aint test` cannot exercise a file with a blocking top-level
  statement.** (§10, same)
- **Tool calls have no real backend** — `MockTool` is the only one
  that has ever existed, live or in tests. (§7,
  `docs/milestones/11-typed-tools/SPEC.md`)
- **`aint fmt` doesn't preserve comments** — refuses rather than
  deletes them. (§2, `docs/milestones/24-language-tooling/SPEC.md`)
- **No LSP, autocomplete, or go-to-definition** — need real semantic
  indexing nothing in the pipeline exposes yet.
  (`docs/milestones/24-language-tooling/SPEC.md`)
- **No real package registry** — `aint add` only takes local paths.
  (`docs/milestones/23-package-manager/SPEC.md`)
- **`http_serve` handles one connection at a time**, by construction —
  real concurrency would need either `tokio::task::spawn_local` under
  a `LocalSet` or moving `Value` off `Rc`, neither attempted.
  (`docs/milestones/25-real-application/SPEC.md`)
