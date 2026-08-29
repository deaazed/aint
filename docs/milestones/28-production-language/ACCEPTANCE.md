# Milestone 28 — Production language (1.0) — acceptance

## Scope

See `SPEC.md`. The codeable subset of "1.0": a stable specification,
a backward compatibility policy, a real (if self-review, not
external-audit) security pass with actual findings fixed, and
quantified performance work. Native compilation, a hosted registry,
and a formal external audit are named and deferred, not attempted.

## Acceptance criteria

- [x] `docs/SPECIFICATION.md`: covers lexical structure, every
      statement/expression form, the type system, both executors'
      exact scope, model backend configuration, the full error
      taxonomy, every stdlib module (including milestone 25's
      `json`/`db`/`auth`/`log`/`http` additions), the testing model's
      `aint-test`/blocking-statement incompatibility, and a
      consolidated, cross-referenced "known gaps" section — checked
      against the actual source while writing it, not transcribed
      from an earlier design document.
- [x] `docs/RUNTIME.md` and `docs/ARCHITECTURE.md`, both found stale
      during this milestone (dotted tool-call syntax the language
      never had, a `model {}` block never built, a crate layout
      missing three shipped crates, a "deferred" list including
      things that shipped), corrected or clearly marked historical
      rather than left silently contradicting the new specification.
- [x] `docs/COMPATIBILITY.md`: semver policy, an explicit covered/
      not-covered split (grounded in `SPECIFICATION.md`'s own "known
      gaps" — nothing not-yet-built is claimed as a stability
      guarantee), a deprecation process, and an honest statement of
      what the policy doesn't yet mechanically enforce.
- [x] Security pass: two real findings, both fixed and tested.
      Path traversal in `db`'s table-name handling — verified fixed
      with 5 new tests (`db.rs`), including that ordinary table names
      still work. Full `RuntimeError` detail leaking into
      `http_serve`'s `500` response bodies — fixed to log server-side
      and return a generic message to the client, verified by
      rebuilding and confirming the existing HTTP integration tests
      (which never asserted on the specific leaked text) still pass.
- [x] Security pass also checked, and found already correct: `bcrypt`
      verification fails closed on a malformed hash, `HttpModel`'s
      response parsing has no unwrap on network data, session tokens
      use a real CSPRNG, and SQL injection is structurally impossible
      (no SQL exists in AINT).
- [x] Performance work: `fibonacci(30)` measured directly,
      `--release`, best-of-three — tree-walking interpreter 3.93s,
      bytecode VM 0.29s, ~13x. A second, unprompted finding recorded
      directly: `examples/customer_support/worker.an` runs correctly
      under `aint run --vm` with zero changes, because the VM's
      native-call dispatch is generic against the shared stdlib table
      rather than hardcoded — verified by actually running it.
- [x] `cargo test --workspace` passes with no regressions: 385 tests
      total, up from 380 before this milestone (5 new: the `db`
      path-traversal tests).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are
      clean across the whole workspace.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — native compilation, a
real package registry, a genuine external security audit, a
mechanically-enforced compatibility test corpus, and a broad
profiling-guided performance pass.

## Outcome

Satisfied by: `docs/SPECIFICATION.md` (new), `docs/COMPATIBILITY.md`
(new), `docs/ARCHITECTURE.md` and `docs/RUNTIME.md` (corrected/marked
historical), `crates/runtime/src/db.rs` (`valid_table_name` + 5 new
tests), `crates/runtime/src/interpreter.rs` (`http_serve`'s error
response no longer leaks `RuntimeError` detail). 385 tests total
across the workspace, all passing, plus the directly-measured
fibonacci(30) interpreter-vs-VM comparison and the confirmed
zero-changes-needed `worker.an` run under `--vm`.
