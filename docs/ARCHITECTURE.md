# AINT — Architecture

How the compiler and toolchain are put together. Read `LANGUAGE_DESIGN.md`
first — this document is about structure, not about why.

## Source files

```
AINT source files:
    Extension:    .an
    Encoding:     UTF-8
    Entry point:  main.an
```

A project looks like:

```
my-project/
├── aint.toml
├── src/
│   ├── main.an
│   └── ...
└── tests/
    └── ...
```

## Compiler pipeline

```
Source (.an)
     │
     ▼
   Lexer            aint-lexer      tokens
     │
     ▼
   Parser           aint-parser     AST
     │
     ▼
   Type Checker      aint-typechecker    typed AST
     │
     ├──────────────────────────────┐
     ▼                              ▼
   Runtime (aint-runtime)      AIR (aint-ir)
   tree-walk interpreter,      explicit Infer/ToolCall/
   the only path that can      Distribution/Probability
   run infer/tool/async        ops; AIR-to-AIR dedup
                                     │
                                     ▼
                                Bytecode + VM (aint-vm)
                                deterministic core only —
                                no infer/tool/async
```

Two execution paths exist since milestone 22, on purpose, not as an
unfinished merge: the tree-walking `aint-runtime` interpreter is the
only one that can run `infer`/`tool`/`async` — `AIR`/`aint-vm` were
built once the language's surface semantics had stabilized (milestone
18) specifically for AINT's *deterministic* core (arithmetic, control
flow, recursion, stdlib natives), where a real stack-based bytecode
VM gives a large, measured speedup (roughly 13x on a compute-heavy
recursive benchmark — see
`docs/milestones/28-production-language/SPEC.md`) at the cost of not
supporting the AI-facing half of the language at all yet. `aint run`
uses the interpreter by default; `aint run --vm` opts into the VM and
fails clearly, at compile time, on anything outside its scope. There
is still no LLVM step, no JIT, and no garbage collector (see
`docs/milestones/21-memory-model/SPEC.md` for why reference counting
remains sufficient).

## Crate layout

```
crates/
├── ast/            aint-ast          shared AST types, no logic
├── lexer/          aint-lexer        source text -> tokens
├── parser/         aint-parser       tokens -> AST
├── typechecker/    aint-typechecker  AST -> typed AST
├── ir/             aint-ir           typed AST -> AIR, AIR-to-AIR optimization
├── runtime/        aint-runtime      tree-walk interpreter, model adapters, tools,
│                                     http/db/auth/log/json stdlib
├── vm/             aint-vm           AIR -> bytecode -> stack-based VM
│                                     (deterministic core only)
├── package/        aint-package      manifest, lockfile, local dependency resolution
├── fmt/            aint-fmt          canonical source formatter
└── cli/            aint              the `aint` binary
```

Dependency direction only ever points down this list — `ast` depends on
nothing in this workspace, `cli` depends on everything. If a crate needs
something from a crate below it in the pipeline, that's a design smell
worth stopping on, not routing around. `aint-vm` and `aint-fmt` both
depend on `aint-runtime`/`aint-ast` respectively for shared types
(`Value`/stdlib dispatch, AST nodes) but not on each other or on `cli`.

Keep this crate count. Don't split further until a crate is genuinely
doing two unrelated jobs — early over-fragmentation costs more than it
saves.

## Runtime execution model

The runtime dispatches across three executors that share one program
state:

```
                Runtime
                   │
      ┌────────────┼────────────┐
      ▼            ▼            ▼
Deterministic   Inference      Tools
  executor       executor     executor
      │            │            │
      │       Model adapter     │
      │            │            │
      │     ┌──────┴──────┐     │
      │     ▼             ▼     │
      │   local          API    │
      │  (vLLM/Ollama) (OpenAI/ │
      │                Mistral) │
      └────────────┬────────────┘
                   ▼
                Result
```

Models are accessed through one trait so real models, self-hosted
models, and deterministic mocks are interchangeable at the type level:

```rust
trait Model {
    async fn infer<T>(
        &self,
        request: InferenceRequest,
    ) -> Result<InferenceResult<T>>;
}
```

Implementations start with `MockModel` (needed from milestone 08 onward
so AI-touching code is testable from day one) and grow to cover vLLM,
Ollama, OpenAI-compatible APIs, and others as needed. The language never
references a vendor directly; deployment configuration selects the
adapter.

## Async

Inference and tool calls are inherently asynchronous — the runtime uses
Tokio under the hood (milestone 07), before any AI primitives are added,
so `infer` and `tool` calls have a real execution model to plug into
rather than being bolted onto a synchronous interpreter after the fact.

## What's still deliberately not built

- **Native compilation / LLVM.** Never scheduled — `ROADMAP.md`'s
  milestone 22 says "still no LLVM" explicitly, and milestone 28
  (production language) kept it out of the codeable subset it
  actually attempted; see
  `docs/milestones/28-production-language/SPEC.md`.
- **A real package registry.** `aint-package` (milestone 23) resolves
  local path dependencies only — no hosted service exists to resolve
  a bare name against.
- **An LSP / editor tooling beyond syntax highlighting.**
  `aint check`/`aint fmt` and a TextMate grammar exist (milestone 24);
  autocomplete, go-to-definition, and a debugger need real semantic
  indexing nothing in the pipeline exposes yet.
- **Cross-file `import`.** Every `import` still resolves to one of a
  fixed set of stdlib modules — no user-authored path or package can
  be imported from another `.an` file yet. A real, load-bearing gap,
  found and named directly in `docs/milestones/25-real-application/SPEC.md`.
- **Agents as a language primitive** (see `LANGUAGE_DESIGN.md`) — still
  deferred until a real agent pattern repeats.

## Where things live

- `docs/LANGUAGE_DESIGN.md` — thesis, non-goals, type system sketch, design
  principles. The "why."
- `docs/ARCHITECTURE.md` (this file) — pipeline and crate layout. The
  "how it's structured."
- `docs/RUNTIME.md` — inference execution, tracing, testing, model
  deployment. The "how it runs."
- `docs/SPECIFICATION.md` — the stable, versioned language reference
  (milestone 28). The "what, precisely."
- `docs/COMPATIBILITY.md` — what's guaranteed to keep working across
  versions, and what isn't yet (milestone 28).
- `ROADMAP.md` — the full milestone list.
- `docs/milestones/NN-name/` — created when a milestone is actively
  started, holding that milestone's `SPEC.md`, notes, and
  `ACCEPTANCE.md`.
