# Milestone 45 — `aint migrate`

## Scope

Requested directly: a command that migrates an AINT project to the
latest syntax, with AI in the loop, and with no regression — plus a
real answer to the rate-limit wall milestone 44's own dogfooding hit
against the live Mistral endpoint (`429 Too Many Requests`, repeatedly,
while verifying `infer -> Node`).

Two genuinely separate asks, handled separately:

1. **The quota question.** Not solved by evading Mistral's rate
   limiting — key rotation, spoofing, or anything else designed to get
   around a paid service's usage controls was explicitly declined.
   Solved instead by (a) retry-with-backoff on a transient `429`/`5xx`
   in both `HttpModel` (used by `infer`) and `ChatClient` (used by
   `scaffold` and this milestone's AI tier) — a sustained `429`, the
   account's real limit, still fails once retries are exhausted, and
   (b) a "no regression" design for `migrate` that doesn't *need* AI to
   deliver its core guarantee at all — see below.
2. **`aint migrate`.** Two tiers, because "no regression" is a strong
   claim and the two tiers can back it with genuinely different levels
   of confidence.

## Tier 1 — deterministic, no AI, no quota dependency

`crates/migrate` (`aint_migrate::migrate_deterministic`): a small,
fixed set of AST rewrites, each chosen because it's **provably
behavior-preserving by construction**, not just empirically tested:

- `if cond { return a } else { return b }` → `return if cond { a }
  else { b }` (adopting milestone 37's if-expression syntax). Applied
  bottom-up, so an `else if` chain collapses into one flat
  `ExprKind::If` spine for free — see the crate's own doc comment for
  why.
- `x == true` / `x == false` / `x != true` / `x != false` simplified to
  `x` / `!x` / `!x` / `x` (adopting milestone 38's `!`), literal on
  either side.

Both rewrites keep every function's signature untouched — only a
body's internal shape changes — which is *why* per-file, independent
re-verification (below) is sufficient even in a multi-file project:
call-site type-checking only ever depends on a callee's declared
signature, never its body, so a callee's migration can't change how
any caller type-checks.

**Verified again anyway, not just trusted**: the CLI (`aint migrate`,
`crates/cli/src/migrate_cmd.rs`) re-parses and re-typechecks the
rewritten file (through the real `aint-loader` pipeline, so cross-file
imports resolve exactly as `aint check` would) before ever keeping it.
On the — never expected — chance the transform itself produced
something broken, the file is reverted to its original content and the
run fails loudly as a `aint-migrate` bug report, not a silent bad
write.

Deliberately *not* attempted deterministically: anything requiring
understanding of intent (a hand-rolled `replace` helper becoming
`string_replace`; hand-built HTML strings becoming `Node` literals).
No mechanical pattern match can tell "this string-concat chain
happens to look like it's building `<div>` tags" from "this one
genuinely is markup" — that's Tier 2's job.

## Tier 2 — AI-assisted (`--ai`), verified before ever kept

Asks the configured model (`AINT_MODEL_URL`, required) to propose a
modernized rewrite of a whole file — the kind of broader restructuring
Tier 1 can't safely automate. The verification bar is the load-bearing
design decision here:

**Only ever attempted where a real behavioral oracle exists.** A file's
own tests, re-run before and after a proposed rewrite and required to
match byte-for-byte (same test names, same pass/fail, same error text
on failure), is that oracle — a genuine regression check, not a hope.
Type-checking alone can't catch a subtly-wrong-but-well-typed rewrite,
and actually *running* an arbitrary file to compare output risks
hanging forever on a blocking top-level statement (`await
http_serve(...)`) — a real, live risk this design refuses to take.

**The oracle doesn't have to live in the file being migrated.**
`aint-loader` forbids a `test` block in any file reached through
`import "..." as ...`, so a shared library file (the most common,
highest-value migration target — `layout.an`-shaped files exist
specifically because a project split its markup-building helpers out
for reuse) can *never* carry its own test block. Real coverage for one
lives in a companion file that imports it and tests it externally — the
same shape `examples/router/router_test.an`/`examples/
customer_support/priority_logic_test.an` already use in this project.
`aint migrate` recognizes this: a file with no test blocks of its own
is still eligible if some *other* file in the batch imports it and has
test blocks. A file with no coverage anywhere in the batch is reported
as skipped, not silently left alone with no explanation, and — since
there's nothing to verify against — the model is never even called for
it, which also means it never spends a quota-limited request on a file
`migrate` was never going to be able to trust anyway.

**Verification is whole-batch, not just the one file that changed** —
the second load-bearing correction found by actually dogfooding this
against a real multi-file project (`aint-website`). A proposal could
change a function's signature, pass its *own* file's type-check (or
even its own tests, if it happens to have none), and silently break
every importer elsewhere in the project — a single-file check can't see
that. So before any run starts, `aint migrate --ai` captures a baseline
(type-check status, and test outcomes where they exist) for *every*
file in the batch, once, from each file's pre-migration content. Then,
for a file that qualifies:

1. Ask the model for a modernized rewrite of the whole file
   (`MIGRATE_SYSTEM_PROMPT`: the current grammar, including `Node`
   literals, explicit instructions to preserve behavior exactly and
   only reach for a rewrite where the older code is clearly expressing
   something the newer syntax says more directly).
2. If the proposal doesn't parse: reject, file unchanged.
3. Otherwise, write it, then re-verify: type-check the changed file
   (same loader pipeline), re-capture its own test outcomes and compare
   against its baseline, *and* re-verify every other file in the batch
   against its own baseline the same way.
4. Any mismatch anywhere — the changed file no longer type-checks or
   its own tests differ, or any *other* file in the batch now fails to
   type-check or has different test outcomes — reverts the changed file
   to what it was immediately before this attempt and reports why,
   naming the broken file if it wasn't the one being migrated: nothing
   is ever left half-migrated or silently kept in a state nobody
   checked.
5. Only a proposal that clears every gate, for every file in the batch,
   is kept.

Tier 1 doesn't need any of this — it never changes a signature, so
per-file, independent verification is already sufficient (see Tier 1's
own reasoning above). This machinery exists specifically because Tier
2's rewrite isn't provably safe by construction the way Tier 1's is.

## CLI

```
aint migrate <path>              # deterministic only
aint migrate <path> --ai         # + AI-assisted, for files with tests
aint migrate <path> --check      # report without writing; --ai is
                                  # skipped (verifying a proposal
                                  # requires writing it)
```

`<path>` is a `.an` file or a directory, walked recursively
(`target`/`.git`/hidden directories excluded). Narrated per file
(`==> scanning ...`, then what changed or why it didn't), a summary
line at the end, `--check` exiting non-zero when anything would
change — the same convention `aint fmt --check` already established.

## Verification methodology

Beyond `crates/migrate`'s own unit tests (idempotency; every rewrite
and its inverse; recursion into lambda/tool/test bodies) and
`crates/cli/tests/migrate.rs`'s integration tests against the real
binary (deterministic rewrite, `--check`, missing `AINT_MODEL_URL`, a
file with no tests never calling a mock server that would otherwise
hang the test, an accepted AI proposal, a rejected-and-reverted one,
directory walking) — the actual "no regression on an existing project"
claim was tested directly against this repository's own `examples/`:

1. Copied the entire directory, ran `aint migrate --check` (22 of 36
   files had a Tier 1 pattern), then applied it for real.
2. Every file re-typechecked cleanly (one pre-existing, unrelated
   failure: `package_import/app/main.an` needs `aint add` run first —
   present before and after, not a regression).
3. `aint run`'s stdout and exit code, diffed byte-for-byte between the
   original and migrated copies across every file that doesn't block on
   `http_serve`: zero mismatches.
4. `aint test`'s summary (test count, pass/fail), diffed the same way
   across all 36 files: zero mismatches.

This is the actual evidence behind "no regression," not an assumption:
every real example in the language's own repository, migrated, still
runs and tests identically.

## Explicitly out of scope

- **Migrating anything Tier 1 can't prove safe and Tier 2 can't verify
  against tests.** A file with no test coverage and no Tier 1 match is
  left exactly as it is — `aint migrate` never guesses.
- **A registry of migration rules beyond the two Tier 1 patterns.**
  Adding a third deterministic pattern is a small, contained addition
  later; not attempted speculatively here.
- **Bypassing/evading a real model provider's rate limiting or quota.**
  See the quota section above — retry-with-backoff and (recommended,
  not built here — already possible today) pointing `AINT_MODEL_URL`
  at a local model are the legitimate answers.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
