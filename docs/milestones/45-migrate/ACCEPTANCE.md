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
- [x] `--project` (mutually exclusive with `<path>`, requested directly
      after using the command for real): finds the nearest `aint.toml`
      walking up from the current directory and migrates everything
      under it — `aint_loader::find_package_root` made `pub` and reused
      as-is rather than a second implementation of "find the project
      root." A clear error when no `aint.toml` exists above the current
      directory; `\\?\`-prefixed verbatim paths (what `canonicalize`
      produces on Windows) stripped for display so `--project`'s
      narration looks like every other path `aint migrate` prints, not
      the one oddly-formatted one. Verified against the real
      `aint-website` project, run from a subdirectory with no manifest
      of its own — found the root correctly.
- [x] `--ai`: proposes a whole-file rewrite via `ChatClient`, but only
      where a real before/after behavioral oracle exists — the file's
      own test blocks, or (found by dogfooding against `aint-website`
      itself: `aint-loader` forbids a `test` block in any file reached
      through `import`, so a shared library file can never carry its
      own) a companion file in the batch that imports it and has tests.
      A file with no coverage anywhere in the batch is reported as
      skipped and the model is never called for it (verified directly:
      a test points `AINT_MODEL_URL` at a listener that never accepts a
      connection, which would hang the test forever if a call were ever
      attempted — it isn't, and the test finishes immediately).
- [x] An accepted AI proposal is verified by capturing test outcomes
      before and after — the changed file's own, *and every other file
      in the batch's* — requiring an exact match (test-for-test,
      pass/fail, error text) in addition to type-checking, for all of
      them, not just the one that changed. This whole-batch check is
      what makes it safe to `--ai`-migrate a file other files import:
      a proposal that changes a function's signature could otherwise
      pass its own file's check while silently breaking an importer.
      Verified with four scenarios against real mock servers: a
      behaviorally-identical same-file proposal (accepted); a
      well-typed but behaviorally-wrong same-file proposal (rejected,
      reverted); a library file with no tests of its own but a tested
      companion (still attempted, accepted); and — the critical case —
      a library-file proposal that keeps the same signature (so the
      library file's own check sees nothing wrong) but changes behavior
      in a way that breaks an *importing* file's test (rejected,
      reverted, the importer's own file named in the reason).
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
- [x] `crates/cli/tests/migrate.rs` (14 tests): deterministic rewrite
      through the real binary, an already-modern file reported
      unchanged and left untouched, `--check`'s no-write/non-zero-exit
      contract, `--ai` without `AINT_MODEL_URL` failing clearly, the
      no-coverage-anywhere skip (proven via the hanging-listener
      technique above), an accepted same-file AI proposal, a
      rejected-and-reverted same-file one, directory walking, a
      library file made eligible by a companion test file, a
      library-file proposal rejected specifically because it broke the
      companion file's test, `--project` finding a manifest from a
      subdirectory, `--project` failing clearly with none anywhere
      above, `--project`/`<path>` being mutually exclusive, and neither
      being given at all.
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
- **A file with no coverage anywhere in the batch (itself or a
  companion) can never receive AI-assisted modernization** — a
  deliberate safety boundary, not an oversight. Real dogfooding against
  `aint-website` found the first version of this boundary too narrow
  (it only checked the file itself, which — given `aint-loader`'s
  no-test-blocks-in-an-imported-file rule — meant a shared library file
  like `layout.an` could *never* qualify, companion test file or not)
  and fixed it before it shipped as the actual behavior; see the
  "companion file" and "whole-batch verification" acceptance criteria
  above.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `aint migrate` delivers "no regression" as a real,
verified guarantee rather than a hope: Tier 1 is behavior-preserving
by construction and re-checked before being kept; Tier 2 is only ever
applied where a genuine before/after oracle exists — the file's own
tests or a companion file's — and every file in the batch, not just the
one that changed, is discarded back to its pre-attempt state the
instant anything regresses. The claim was tested against this
project's own real codebase, not just synthetic examples, with zero
mismatches, and the cross-file gap was found and closed by actually
attempting to migrate a second real project (`aint-website`) before
calling this done, not by inspection. The quota problem that prompted
this milestone is addressed through legitimate resilience
(retry-with-backoff) and a design that doesn't require a live model to
deliver its core guarantee — not through any attempt to get around a
provider's actual usage limits.
