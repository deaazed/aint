# Milestone 19 — Optimization — acceptance

## Scope

See `SPEC.md`. One real, sound AIR-to-AIR optimization —
deduplicating repeated `infer`/`tool`/`Distribution<T>` operations
within a block — covering three of `ROADMAP.md`'s seven names
("inference caching," "prompt caching," "memoization," which are the
same idea), with the other four stated as blocked on specific, named
architectural prerequisites rather than left unexplained.

## Acceptance criteria

- [x] `aint_ir::optimize(&AirProgram) -> (AirProgram, OptimizationStats)`
      is a separate, explicitly-invoked stage — not folded into
      `lower`, matching how nothing consumes AIR at all yet.
- [x] Two identical calls (same function, identical literal arguments)
      within one block: the second is rewritten to reference the
      first's result by name; `OptimizationStats::eliminated` reports
      `1`. Verified for `infer`, `tool`, and a `Distribution<T>`
      operation (`distribution_argmax`) — the same mechanism applies
      uniformly to all three of `ROADMAP.md`'s named AI-operation node
      kinds.
- [x] Two identical calls with identical *identifier* arguments (not
      just literals) are also deduplicated — verified directly, with
      the soundness argument (AINT has no mutation/reassignment
      anywhere, so a bound name can't have changed between the two
      call sites) stated in both `SPEC.md` and the code itself.
- [x] Two calls with *different* arguments both survive untouched —
      verified directly.
- [x] Two identical calls in different branches of the same `if` both
      survive — each block starts a fresh cache, verified directly,
      confirming blocks are never conflated even when their contents
      are textually identical.
- [x] A call with a *nested call* as an argument is never treated as
      cacheable, even against an identical-looking duplicate —
      verified directly (nothing here attempts to prove the nested
      call has no side effects, so it's conservatively excluded
      entirely).
- [x] A bare expression-statement call (no `let`) is hoisted into a
      synthesized `let` on its first occurrence so a later duplicate
      has something to reference — verified directly, checking both
      the hoisted binding's shape and the second occurrence's
      rewritten form.
- [x] A program with nothing to deduplicate passes through unchanged
      (`eliminated == 0`, same statement count) — verified directly.
- [x] `crates/runtime` is untouched — `aint run`/`aint test` are
      unaffected, since nothing consumes AIR.
- [x] `cargo test --workspace` passes with no regressions: 290 tests
      total, up from 281 before this milestone (9 new, all in
      `aint-ir`).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md`'s "What's explicitly not built, and why" — parallel
inference and tool parallelization (blocked on milestone 07's
deliberate single-threaded-runtime decision), model routing (blocked
on there being only one `model` field per `Interpreter`, no
per-call-site selection anywhere), and request batching (blocked on
`HttpModel` sending one synchronous request per call, with nothing in
the execution model that defers or groups calls). Also: cross-block or
cross-function deduplication, and deduplicating anything with a
nested-call argument.

## Outcome

Satisfied by `crates/ir/src/optimize.rs` (new: `optimize`,
`OptimizationStats`, the per-block cache-key-based deduplication) and
`crates/ir/src/lib.rs` (re-exports). 290 tests total across the
workspace, all passing: 9 new tests in `aint-ir` covering literal-arg
and identifier-arg deduplication across all three AI-operation kinds,
the different-arguments and different-branches non-deduplication
cases, the nested-call-argument exclusion, statement hoisting, and the
no-op case.
