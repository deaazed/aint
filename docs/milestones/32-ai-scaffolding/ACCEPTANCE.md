# Milestone 32 — AI-assisted scaffolding — acceptance

## Scope

See `SPEC.md`. `aint scaffold "description" <path>`: a new CLI command
generating a starter AINT project from a plain-English description via
a new `aint-runtime::ChatClient`, always checked before being reported
as done — minus iterative refinement, multi-file generation, and
automatic retry, all named directly as out of scope.

## Acceptance criteria

- [x] New `aint-runtime::ChatClient` (`crates/runtime/src/chat.rs`):
      `new`/`with_api_key` (mirroring `HttpModel`'s own builder shape),
      `complete(system_prompt, user_prompt) -> Result<String, String>`
      — a plain two-message chat completion against any
      OpenAI-compatible endpoint, returning raw response text.
      Exported from `aint-runtime`'s public API.
- [x] Verified directly against a local mock server (no live network):
      a successful completion returns the response text; a non-success
      HTTP status is a clear error naming the status code.
- [x] New `Command::Scaffold` in `crates/cli/src/main.rs`:
      `aint scaffold "description" <path>`.
- [x] Requires `AINT_MODEL_URL`; a clear error (naming the variable),
      no project created, if it's unset — verified through the real
      binary.
- [x] Refuses to run if `<path>/aint.toml` already exists, the same
      rule `aint init` follows.
- [x] `SCAFFOLD_SYSTEM_PROMPT`: a condensed, accurate reference to
      AINT's real syntax (bindings, control flow, closures, cross-file
      imports, the stdlib module list, testing, and the specific
      known gaps a model would otherwise invent syntax around —
      `Option` construction, loops, dotted access).
- [x] `extract_source`: strips a language-tagged, plain, or absent code
      fence from the model's response — verified directly with 4 unit
      tests covering all three shapes plus surrounding whitespace.
- [x] Generated `main.an` is always run through the same
      `parse_and_check` gate `aint check` uses. A well-typed response
      is written and reported as success; a response that fails to
      type-check is *still written to disk* (for inspection) but the
      command exits non-zero with a clear "does not type-check"
      message — verified end to end through the real binary, with
      `AINT_MODEL_URL` pointed at a local mock server standing in for
      the model, for both the success and failure paths.
- [x] `cargo test --workspace` passes with no regressions: 427 tests
      total, up from 418 before this milestone (9 new: 2 `ChatClient`
      unit tests, 4 `extract_source` unit tests, 3 `aint scaffold`
      integration tests against the real binary).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Known, honestly-stated gaps

- **One-shot only** — no iterative refinement, no editing an existing
  project.
- **Always exactly one `main.an`** — no multi-file generation, despite
  milestone 29 making it possible in principle.
- **No automatic retry on a failed type-check** — the failure is
  reported and the file left as-is.
- **Never verified against a live LLM in this milestone** — only
  against a local mock server, matching how `HttpModel` itself has
  always been tested. The wire protocol is real and shared with `aint
  run`'s own `AINT_MODEL_URL` path, which *has* been used against real
  backends, but `aint scaffold` specifically has not.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied by `crates/runtime/src/chat.rs` (`ChatClient`, exported from
`aint-runtime`), `crates/cli/src/main.rs`'s new `Command::Scaffold`,
`scaffold`, `extract_source`, and `SCAFFOLD_SYSTEM_PROMPT`, and
`crates/cli/tests/scaffold.rs` (3 real-binary integration tests against
a local mock model server). 427 tests total across the workspace, all
passing.
