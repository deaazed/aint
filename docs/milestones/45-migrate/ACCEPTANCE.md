# Milestone 45 — `aint migrate` — acceptance

## Scope

See `SPEC.md`. Retry-with-backoff for the two AI HTTP clients, a new
`aint-migrate` crate (deterministic, provably behavior-preserving AST
rewrites), and `aint migrate` wiring both that and an AI-assisted,
test-verified tier into the CLI.

## Acceptance criteria

- [x] `HttpModel::send_with_retry` (`crates/runtime/src/http_model.rs`)
      and `ChatClient::send_with_retry` (`crates/runtime/src/chat.rs`):
      a `429`/`5xx` is retried up to 4 times with exponential backoff
      (honoring a numeric `Retry-After` header when present), a
      sustained failure still surfaces as a real error once retries are
      exhausted. Verified against real mock HTTP servers (a `429` then
      a real success; `Retry-After` timing measured directly; a
      sustained `429` across every retry attempt still failing) — not
      just unit-level logic checks.
- [x] `crates/migrate` (new): `migrate_deterministic(Program) ->
      (Program, Vec<Migration>)` — the if-return-else-return → return-
      if-expression rewrite (collapsing `else if` chains for free via
      bottom-up composition) and the four boolean-literal-comparison
      simplifications. 11 unit tests, including idempotency
      (`migrate(migrate(src)) == migrate(src)`) and recursion into
      lambda bodies.
- [x] `aint migrate <path> [--ai] [--check]`
      (`crates/cli/src/migrate_cmd.rs`, new): walks a file or directory,
      applies the deterministic tier always, verifies by re-parsing and
      re-typechecking through the real `aint-loader` pipeline before
      keeping a rewrite (reverting and reporting a hard
      `aint-migrate`-bug failure on the — never observed — chance that
      fails).
- [x] `--ai`: proposes a whole-file rewrite via `ChatClient`, but only
      for a file with at least one `test` block — the one case with a
      real before/after behavioral oracle. A file with no test blocks
      is reported as skipped and the model is never called for it
      (verified directly: a test points `AINT_MODEL_URL` at a listener
      that never accepts a connection, which would hang the test
      forever if a call were ever attempted — it isn't, and the test
      finishes immediately).
- [x] An accepted AI proposal is verified by capturing this file's own
      test outcomes before and after, requiring an exact match
      (test-for-test, pass/fail, error text) in addition to
      type-checking — verified with a real mock server returning a
      behaviorally-identical proposal (accepted) and, separately, one
      that's well-typed but behaviorally wrong (rejected, file reverted
      to exactly what it was before the attempt).
- [x] `--check`: reports what would change without writing anything,
      exits non-zero if anything would — the same convention `aint fmt
      --check` already established. Skips the AI tier entirely (stated
      directly, not silently): verifying a proposal requires writing
      it.
- [x] **The actual "no regression on an existing project" claim,
      tested directly against this repository's own `examples/`, not
      assumed**: the whole directory copied, migrated (22 of 36 files
      changed), then diffed against the untouched original —
      `aint run`'s stdout and exit code identical across every
      non-blocking file, `aint test`'s pass/fail summary identical
      across all 36 files, zero mismatches either way. See `SPEC.md`'s
      verification section for the exact methodology.
- [x] `crates/cli/tests/migrate.rs` (new, 8 tests): deterministic
      rewrite through the real binary, an already-modern file reported
      unchanged and left untouched, `--check`'s no-write/non-zero-exit
      contract, `--ai` without `AINT_MODEL_URL` failing clearly, the
      no-test-blocks skip (proven via the hanging-listener technique
      above), an accepted AI proposal, a rejected-and-reverted one, and
      directory walking.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **No third-party quota bypass was built, on purpose.** The request
  named Mistral's rate limit directly; the fix is retry-with-backoff
  (handles a transient blip) plus this milestone's design not
  depending on a live model at all for its core "no regression"
  guarantee (Tier 1) or for the parts of Tier 2 it can't verify (files
  with no tests, skipped rather than risked). A sustained `429` — the
  account's actual limit — is not something this milestone gets around,
  by design.
- **Only two deterministic patterns exist.** Real, but small and
  contained to extend later — see `SPEC.md`'s "Explicitly out of
  scope."
- **A file with no test blocks can never receive AI-assisted
  modernization**, even if it would clearly benefit (the `layout.an`-
  style hand-built-HTML case named back in milestone 44's own
  retrospective). This is a deliberate safety boundary, not an
  oversight — adding test coverage to such a file first, then
  `--ai`-migrating it, is the intended path.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `aint migrate` delivers "no regression" as a real,
verified guarantee rather than a hope: Tier 1 is behavior-preserving
by construction and re-checked before being kept; Tier 2 is only ever
applied where a genuine before/after oracle exists and is discarded
the instant it doesn't hold. The claim was tested against this
project's own real codebase, not just synthetic examples, with zero
mismatches. The quota problem that prompted this milestone is
addressed through legitimate resilience (retry-with-backoff) and a
design that doesn't require a live model to deliver its core
guarantee — not through any attempt to get around a provider's actual
usage limits.
