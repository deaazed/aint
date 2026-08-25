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
     ▼
   AIR               aint-ir         AINT Intermediate Representation
     │
     ▼
   Runtime           aint-runtime    execution
```

`AIR` (AINT Intermediate Representation) is introduced once the
language's surface semantics have stabilized (milestone 18), not before —
early milestones run directly off the typed AST through a tree-walk
interpreter. There is no LLVM step, no JIT, no bytecode VM, and no
garbage collector in the initial design. Those are milestone 20-22
concerns at the earliest, and only if the tree-walk interpreter's
limitations actually demand them.

## Crate layout

```
crates/
├── ast/            aint-ast          shared AST types, no logic
├── lexer/          aint-lexer        source text -> tokens
├── parser/         aint-parser       tokens -> AST
├── typechecker/    aint-typechecker  AST -> typed AST
├── ir/             aint-ir           typed AST -> AIR
├── runtime/        aint-runtime      interpreter, model adapters, tools
└── cli/            aint              the `aint` binary
```

Dependency direction only ever points down this list — `ast` depends on
nothing in this workspace, `cli` depends on everything. If a crate needs
something from a crate below it in the pipeline, that's a design smell
worth stopping on, not routing around.

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

## What's deliberately deferred

Don't build these until their milestone, even if it looks easy to add
early:

- Bytecode VM / native compilation (milestone 22)
- Package manager / registry (milestone 23)
- LSP / editor tooling (milestone 24)
- A dedicated memory model beyond "the Rust runtime owns it" (milestone 21)
- Agents as a language primitive (see `LANGUAGE_DESIGN.md`)

## Where things live

- `docs/LANGUAGE_DESIGN.md` — thesis, non-goals, type system sketch, design
  principles. The "why."
- `docs/ARCHITECTURE.md` (this file) — pipeline and crate layout. The
  "how it's structured."
- `docs/RUNTIME.md` — inference execution, tracing, testing, model
  deployment. The "how it runs."
- `ROADMAP.md` — the full milestone list.
- `docs/milestones/NN-name/` — created when a milestone is actively
  started, holding that milestone's `SPEC.md`, notes, and
  `ACCEPTANCE.md`.
