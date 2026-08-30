# Milestone 32 — AI-assisted scaffolding

## Scope

`infer`/`tool` have always been *runtime* AI operations — a `Model`
answers a structured request inside a running AINT program. Nothing
before this milestone let AI help *write* an AINT program in the first
place, despite the model-adapter machinery (`HttpModel`,
`AINT_MODEL_URL`) already existing since milestone 16/25. This
milestone dogfoods that machinery at the tooling layer: `aint scaffold
"description" <path>` generates a starter project from a plain-English
description, checked before it's ever shown as done.

## What this milestone actually builds

**`ChatClient`, a new type in `aint-runtime`** (`crates/runtime/src/
chat.rs`) — a plain, unstructured chat-completion call to any
OpenAI-compatible endpoint. Deliberately *not* a reuse of `Model`/
`HttpModel`: those are shaped entirely around `infer`'s calling
convention (a structured `InferenceRequest` — function name, typed
args, a declared return type used to build a JSON-schema-shaped
prompt and parse a type-directed response). Generating free-form
source code is a genuinely different request shape — a system prompt,
a user prompt, one string back — not a special case of it.
`ChatClient` duplicates `http_model.rs`'s tiny `ChatRequest`/
`ChatMessage`/`ChatResponse` structs rather than sharing them, the
same small-duplication call this codebase already makes for the
typechecker/runtime stdlib signature tables.

**`aint scaffold "description" <path>`** (`crates/cli/src/main.rs`):

1. Requires `AINT_MODEL_URL` — there's nothing to scaffold without a
   real model, so this is a hard requirement, not an optional
   enhancement the way it is for `aint run`.
2. Refuses to run if `<path>/aint.toml` already exists — the same rule
   `aint init` follows; this creates a new project, it doesn't edit one.
3. Sends a fixed system prompt (`SCAFFOLD_SYSTEM_PROMPT`) — a
   condensed, accurate reference to AINT's actual syntax (bindings,
   control flow, closures, cross-file imports, the stdlib module list,
   testing) written from what real `.an` files in this repository
   actually do, not a guess — plus the user's own description as the
   user prompt.
4. Extracts AINT source from the response (`extract_source`), stripping
   a ` ```an `/` ```aint `/plain ` ``` ` code fence if the model wrapped
   its answer in one, which most do even when asked not to.
5. Writes `<path>/aint.toml` (via the existing `aint-package::Manifest`,
   same as `aint init`) and `<path>/main.an`.
6. Runs the generated `main.an` through exactly the same gate `aint
   check` uses (`parse_and_check` — `aint-loader` then
   `aint-typechecker`). **The file is written to disk either way** — a
   program that fails to type-check is left there to inspect and fix,
   not discarded — but the command reports success only when it
   actually type-checks, and exits non-zero with a clear "does not
   type-check" message otherwise. Never silently hands over code that
   doesn't type-check as if it were done.

## Design decisions

**One-shot only.** `aint scaffold` creates a new project; it doesn't
edit an existing one, doesn't refine its own output, doesn't hold a
conversation. Iterative refinement is real, separate future work, not
attempted here.

**No live network dependency in `cargo test`.** `ChatClient` is tested
against a hand-rolled local mock server (the same technique
`http_model.rs`'s own tests already use) — proving the real wire
protocol works, not a model of it. `aint scaffold`'s own behavior
(writing files, running the check gate, reporting success/failure
accurately) is verified the same way, through the real binary, with
`AINT_MODEL_URL` pointed at a local mock server — not a live LLM.

**The system prompt lives in the CLI, not generated from
`docs/SPECIFICATION.md`.** That document is the exhaustive reference;
turning it into a prompt automatically would either bloat every
request or require its own summarization step. `SCAFFOLD_SYSTEM_PROMPT`
is deliberately the smallest version that still keeps a model from
inventing syntax AINT doesn't have (loops, `Option` construction,
dotted access) — maintained by hand, alongside the language itself.

## Explicitly out of scope

- **Iterative/chat-style refinement** or editing an existing project —
  v1 is one-shot scaffold-a-new-project only.
- **Generating multi-file projects.** Always exactly one `main.an` —
  milestone 29's `import "path" as alias` means a generated project
  *could* reasonably span files, but teaching the prompt and the
  writer to do that is separate, real work.
- **Retrying automatically on a failed type-check.** The failure is
  reported and the file is left as-is; re-running the model
  automatically (with the error fed back) is a natural follow-up, not
  attempted here.
- **Any change to `Model`/`HttpModel`/`infer` itself.** `ChatClient` is
  entirely new, additive surface.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
