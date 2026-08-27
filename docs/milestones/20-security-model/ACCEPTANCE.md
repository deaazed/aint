# Milestone 20 — Security model — acceptance

## Scope

See `SPEC.md`. Tool authorization via a `permissions [...]` clause on
`infer` declarations — the one genuine, live security gap that exists
in the language today (a model can request any declared `tool`, with
no way to scope that per `infer` function). "Resource limits" is
already done (milestone 17's `budget`); sandboxing, filesystem/network
restrictions, and secret management are stated as blocked on
primitives that don't exist yet, not half-built.

## Acceptance criteria

- [x] A new `permissions` keyword (lexer) and `parse_permissions_clause`
      (parser): `infer name(params) -> Type permissions [tool_a,
      tool_b]`. Absent entirely means unrestricted — verified directly,
      and confirmed by every pre-existing test continuing to pass
      unmodified.
- [x] `StmtKind::Infer` gains `permissions: Option<Vec<String>>`.
      Parses a single tool name, multiple comma-separated names, and
      an explicitly empty `[]` (meaning no tools at all) — all three
      verified directly.
- [x] The type checker rejects a `permissions` name that isn't a
      declared `tool` — a typo, an `infer`, or a plain `fn` — via a new
      `TypeError::UnknownTool`, verified for all three cases.
- [x] A `permissions` clause can name a `tool` declared later in the
      same file, since tools are hoisted before any `check_stmt` runs
      — verified directly.
- [x] `InferenceFn`/`PendingInference` carry `permissions` through from
      declaration to the deferred call, exactly like `return_type`
      already does.
- [x] `available_tools()` became `available_tools_for(&permissions)`:
      filters what's offered to the model down to the permitted set
      when `Some`, unrestricted when `None` — the "what's offered"
      half of authorization.
- [x] A new `check_tool_permission` enforces the same allowlist against
      what the model *actually requests* (`InferenceOutcome::CallTool`),
      independent of what was offered — the "what's allowed to
      execute" half. A model requesting a tool outside `permissions`
      gets `RuntimeError::PermissionDenied`, verified directly, even
      when that tool is declared and configured elsewhere in the same
      program (proving the check isn't just "was it offered").
- [x] A permitted tool call still succeeds end to end (multi-step
      tool-calling conversation, same shape as milestone 12's), and an
      `infer` with no `permissions` clause can still call any declared
      tool — both verified directly, confirming milestone 12's
      existing behavior is unaffected by default.
- [x] `examples/security.an` (new): declares two tools, an `infer`
      restricted to one of them via `permissions`, and a `test`/`mock`
      block proving it still type-checks and runs normally when mocked
      with a direct answer. Passes via both `aint test` and `aint run`,
      verified through the real binary.
- [x] `crates/ir` is untouched — `AirStmt::Infer` doesn't carry
      `permissions` any more than it carries `return_type` today;
      nothing consumes AIR yet, so there's no execution path there to
      enforce anything against.
- [x] `cargo test --workspace` passes with no regressions: 306 tests
      total, up from 290 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Known, honestly-stated gap

`aint test`'s `mock` syntax (milestone 15) can only script a direct
answer for an `infer`, not an `InferenceOutcome::CallTool` — so a
model requesting a tool call at all, permitted or not, has no
AINT-source-level way to be simulated. `examples/security.an`
therefore only demonstrates the clause parsing/type-checking and that
a restricted `infer` behaves normally when mocked directly; the actual
enforcement behavior (`PermissionDenied` on an out-of-permission
request, and the permitted case succeeding through a real multi-step
tool-calling conversation) is verified at the Rust level, directly
against `Interpreter`/`MockModel`, the same way milestone 12's
original tool-calling tests are. Not a gap introduced by this
milestone — the same DSL limitation would block testing milestone 12's
tool-calling behavior through `aint test` too, and always has.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — sandboxing,
filesystem/network restrictions, secret management, per-argument tool
policy, and unifying `permissions` with `effects`, each with the
specific reason it's blocked rather than left unexplained.

## Outcome

Satisfied by: `crates/lexer/src/token.rs` (`Permissions` keyword),
`crates/ast/src/stmt.rs` (`StmtKind::Infer.permissions`),
`crates/parser/src/parser.rs` (`parse_permissions_clause`),
`crates/typechecker/src/checker.rs` and `error.rs`
(`TypeError::UnknownTool`, validation in the `Infer` arm of
`check_stmt`), `crates/runtime/src/value.rs`
(`InferenceFn`/`PendingInference.permissions`),
`crates/runtime/src/interpreter.rs` (`available_tools_for`,
`check_tool_permission`), `crates/runtime/src/error.rs`
(`RuntimeError::PermissionDenied`), and `examples/security.an`. 306
tests total across the workspace, all passing: 16 new, covering
clause parsing, type-check validation (unknown tool, wrong kind of
declaration, forward reference, empty list, absent clause), runtime
enforcement (permitted succeeds, unpermitted rejected even when
declared elsewhere, unrestricted-by-default preserved), and the new
example through both `aint test` and `aint run`.
