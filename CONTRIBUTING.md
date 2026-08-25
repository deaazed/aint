# Contributing

Read before touching code, in this order:

1. `docs/LANGUAGE_DESIGN.md` — the thesis, explicit non-goals, type
   system sketch, ten design principles. This is the document that wins
   if anything else seems to contradict it.
2. `docs/ARCHITECTURE.md` — compiler pipeline and crate layout.
3. `docs/RUNTIME.md` — how inference, tools, tracing, and testing work
   at runtime.
4. `ROADMAP.md` — the full milestone list and current status.

This is a real programming language, not a Python transpiler, a
prompt-templating DSL, a LangChain/LangGraph wrapper, a YAML workflow
engine, an agent-only language, or an LLM SDK with nicer syntax. If a
shortcut would turn AINT into one of those, it's the wrong shortcut.

## The milestone-gated workflow

`ROADMAP.md` lists milestones 00 through 28. Treat each as a gate, not a
suggestion:

**Before starting a milestone:**
- Re-read `docs/LANGUAGE_DESIGN.md` and `docs/ARCHITECTURE.md`.
- Inspect the existing code the milestone touches.
- Identify which crates are affected.
- Aim for the smallest implementation that satisfies the milestone, not
  the most complete one imaginable.
- If a milestone doesn't have a `docs/milestones/NN-name/` folder yet,
  create one with a `SPEC.md` describing what this milestone covers and
  explicitly doesn't, before writing implementation code. Use
  `docs/milestones/00-language-thesis/` and
  `docs/milestones/01-project-bootstrap/` as the reference shape.

**While implementing:**
- Don't implement future milestones prematurely, even when it's tempting
  because you're already in the file. A milestone-04 change that quietly
  does milestone-09 work is a scope violation, not a bonus.
- Don't add a language feature without tests for it.
- Don't silently change an already-established language semantic — if a
  change is genuinely needed, call it out explicitly rather than letting
  it slide in as a side effect.

**After implementing:**
- Add unit tests (in-crate) and integration tests (whole-program, under
  `tests/`, once there's an interpreter to run programs against).
- Run `cargo fmt`, `cargo clippy --workspace --all-targets -- -D
  warnings`, and `cargo test --workspace`. All three clean.
- Update the relevant living doc (`LANGUAGE_DESIGN.md`, `ARCHITECTURE.md`,
  or `RUNTIME.md`) if the change affects what they describe.
- Write (or update) `docs/milestones/NN-name/ACCEPTANCE.md` and check off
  what's actually true — don't check off criteria that aren't met yet.

## Engineering principles (from `docs/LANGUAGE_DESIGN.md`)

1. Strong static typing — AI involvement is never a reason to go dynamic.
2. Uncertainty is explicit, always, everywhere.
3. AI operations are represented in the AST and type system directly,
   never smuggled through a generic function-call node.
4. Tool calls are typed and validated before execution.
5. AI behavior must be testable with deterministic mocks — no test
   should ever require a live model call to pass.
6. Runtime effects must be observable; tracing is a primitive, not a
   library bolted on later.
7. Never hide inference behind an ordinary-looking function call.
8. Prefer simple semantics over a clever implementation.
9. No language feature lands without tests.
10. Established semantics don't change silently.

## Rust conventions

- Dependency direction only flows one way through the pipeline: `ast` ->
  `lexer` -> `parser` -> `typechecker` -> `ir` -> `runtime` -> `cli`. A
  crate never depends on something above it in that list.
- Tree-walk interpreter first. No LLVM, native compilation, JIT,
  bytecode VM, or garbage collector until the milestone that explicitly
  calls for it (22 and 21, respectively) — and only if the tree-walk
  interpreter's limits actually demand it.
- Keep the current crate count. Split a crate only when it's genuinely
  doing two unrelated jobs, not preemptively.
- `MockModel` (or the equivalent deterministic test double) is not
  optional scaffolding — it's how every milestone from 08 onward stays
  testable without a live model.

## Where things live

- `docs/LANGUAGE_DESIGN.md`, `docs/ARCHITECTURE.md`, `docs/RUNTIME.md` —
  living reference docs, updated as the project evolves.
- `ROADMAP.md` — milestone list and status.
- `docs/milestones/NN-name/` — per-milestone `SPEC.md` / notes /
  `ACCEPTANCE.md`, created when that milestone starts.
- `crates/` — the workspace: `ast`, `lexer`, `parser`, `typechecker`,
  `ir`, `runtime`, `cli` (binary name `aint`).
- `examples/` — sample `.an` programs, used as the target for later
  milestones' end-to-end tests.
- `tests/` — whole-program tests that run actual `.an` files through the
  real `aint` binary.
