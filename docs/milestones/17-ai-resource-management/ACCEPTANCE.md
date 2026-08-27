# Milestone 17 — AI resource management — acceptance

## Scope

See `SPEC.md`. `budget { max_tokens max_model_calls max_cost
timeout_ms }`, a single program-wide resource ceiling, with
`max_model_calls` and `timeout_ms` genuinely enforced — the direct,
named payoff of milestone 12's documented gap ("no cap on iterations
... by design ... milestone 17 is explicitly where budget belongs").

## Acceptance criteria

- [x] `budget { field = literal ... }` parses; all four fields are
      optional; a second `budget` block in one program is a type
      error (`TypeError::DuplicateBudget`), not last-write-wins.
- [x] `timeout_ms` (a plain integer), not `timeout = 10s` — matches
      `time_sleep_ms`'s existing plain-milliseconds convention instead
      of inventing a duration-literal lexical form.
- [x] `max_model_calls` is checked immediately before every
      `self.model.infer(...)` call in `eval_inference`'s tool-calling
      loop; exceeding it produces `RuntimeError::BudgetExceeded`
      *before* the would-be-excess call happens. Verified with a
      scripted multi-step tool-calling conversation where the budget
      cuts it off one call short of answering.
- [x] `timeout_ms` wraps the whole inference conversation in
      `tokio::time::timeout`; verified against a genuinely slow custom
      `Model` test double (a real `tokio::time::sleep`, not a proxy
      for slowness) that a fast `MockModel` could never exercise.
- [x] `max_tokens`/`max_cost` have real, tested comparison logic
      (`record_model_call`) — verified with a test that pokes the
      accumulator fields directly, since every live call reports zero
      tokens today and nothing computes cost anywhere in this
      codebase. This limitation is stated in both `SPEC.md` and in a
      code comment at the point it matters, not left for someone to
      discover by testing it and finding nothing happens.
- [x] A `budget` block that sets only unrelated fields (e.g. just
      `max_tokens`) does not restrict `max_model_calls` at all —
      verified directly.
- [x] Every program without a `budget` block — every pre-17 test, plus
      one new direct case — is completely unaffected: opt-in, the same
      shape as `effects` (13).
- [x] `aint run`/`aint test`/`examples/*.an` are unaffected. No new
      example (budget enforcement has no new AINT-visible behavior
      beyond an error message; the existing `infer`/`tool` examples
      already can't run through `aint run` for reasons predating this
      milestone).
- [x] `cargo test --workspace` passes with no regressions: 269 tests
      total, up from 257 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — real token counting from `HttpModel`, real cost
tracking, per-call-site/per-function budgets, and budget enforcement
outside `eval_inference` (direct, non-model-requested tool calls
aren't budget-constrained).

## Outcome

Satisfied by `crates/lexer/src/token.rs` (`budget` keyword),
`crates/ast/src/stmt.rs` (`StmtKind::Budget`),
`crates/parser/src/parser.rs` (`parse_budget_statement` and its
literal helpers), `crates/typechecker/src/{checker,error}.rs`
(`has_budget`, `TypeError::DuplicateBudget`), and
`crates/runtime/src/{error,interpreter}.rs` (`Budget`,
`RuntimeError::BudgetExceeded`, the `timeout_ms` wrapper around
`eval_inference_loop`, `check_model_call_budget`,
`record_model_call`). 269 tests total across the workspace, all
passing: 4 new parser tests, 3 new typechecker tests, 5 new runtime
tests.
