# Milestone 25 — Real application — acceptance

## Scope

See `SPEC.md`. Five new `aint-runtime` stdlib modules (`json`, `db`,
`auth`, `log`, `http`), an `AINT_MODEL_URL`-driven real-model option
for `aint run`, and a real customer-support application
(`examples/customer_support/`) built entirely in AINT on top of them —
plus an honest accounting of every real language gap discovered while
actually building it, which is the milestone's own explicit purpose.

## Acceptance criteria

- [x] `json_get(json, key) -> Option<String>` / `json_object(keys,
      values) -> String`: flat-object read/write over `serde_json`,
      verified directly (found/missing key, round-trip through
      `json_object` then `json_get`).
- [x] `db_insert`/`db_get`/`db_list`/`db_update`/`db_delete`: a real,
      file-backed (`.aintdb/<table>.jsonl`) store — 10 tests in
      `db.rs` covering insert/get round-trip, duplicate-insert
      rejection, update and delete (including of a missing id),
      listing, and table isolation, each against a real scratch
      directory (never a shared or process-global one — `db`'s
      functions take the base directory as an explicit parameter
      specifically so tests don't need `std::env::set_current_dir`,
      which would race across `cargo test`'s parallel threads).
- [x] `auth_hash_password`/`auth_verify_password` (real `bcrypt`
      hashing, not a hand-rolled scheme) and `auth_generate_token`
      (real randomness via `rand`): verified directly, including that
      two generated tokens differ and a wrong password is rejected.
- [x] `log_info`/`log_error`: verified to run without error through
      real AINT source.
- [x] `http_serve(port)`: a real HTTP/1.1 server over a raw
      `TcpListener` (no `hyper`/`axum` — see `SPEC.md` for why),
      dispatching every request to a `handle_request(method, path,
      body) -> String` the AINT program declares. Verified through
      the real built `aint` binary with a real TCP client: a request
      to a defined route gets the right body back, an unmatched path
      falls through to the program's own catch-all.
- [x] `aint run`'s `AINT_MODEL_URL` environment variable: unset keeps
      today's `MockModel` behavior exactly (verified by the whole
      existing test suite passing unmodified); set, `HttpModel`
      (milestone 16, never wired into the CLI before this) is used
      instead — the first time any AINT program has been able to
      reach a real model outside `aint test`.
- [x] `examples/customer_support/server.an` — real, unmodified,
      spawned as a real process and driven with real HTTP requests
      over a real socket in an isolated scratch directory: register,
      a duplicate-email rejection, login, a wrong-password rejection,
      an unauthenticated-request rejection, and listing tickets for a
      session with none yet — all verified end to end.
- [x] Ticket creation (the `infer`/`tool`-touching path) was verified
      live by hand against the running server, producing the exact,
      honest failure every other `infer` call in this project produces
      outside `aint test` — a clean `model error: no mock response
      configured for `classify_sentiment``, surfaced correctly through
      the HTTP layer as a `500` with the real error message, not a
      crash or hang.
- [x] `examples/customer_support/priority_logic_test.an` — the
      `infer`-then-`tool` priority decision (negative sentiment +
      premium tier -> high priority; every other combination -> normal
      priority; the tool is never called for a non-negative
      sentiment) verified deterministically offline via `aint test`
      and `mock`: 4/4 tests pass.
- [x] `examples/customer_support/worker.an` verified against a real,
      empty `.aintdb/jobs.jsonl` — drains cleanly, reports zero
      processed.
- [x] Both `.an` application files type-check cleanly via `aint check`.
- [x] `SPEC.md`'s "What building this actually found" documents six
      real, load-bearing gaps surfaced by writing this application,
      not invented in the abstract: no `Option`-construction syntax,
      no list concatenation/incremental construction, no `Int`/
      `String` conversion, no boolean negation/`<=`/`>=` (confirmed
      already known, but load-bearing here for the first time), `aint
      test`'s re-execute-every-statement design being incompatible
      with a file that also has a blocking entry point, and tool
      calls never having had a real backend in this project's history.
- [x] `cargo test --workspace` passes with no regressions: 380 tests
      total, up from 362 before this milestone (18 new: 10 in
      `aint-runtime`'s `db` module, 6 interpreter-level tests for
      `json`/`auth`/`log`, 1 CLI `http_serve` integration test, 1 CLI
      customer-support end-to-end integration test — the four
      `aint test` cases in `priority_logic_test.an` are exercised
      through the real binary, not `cargo test`, and counted
      separately above).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are
      clean across the whole workspace.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — a record/struct language
feature, concurrent request handling, concurrent-writer safety in
`db`, nested/structured JSON, and TLS.

## Outcome

Satisfied by: `crates/runtime/src/db.rs` (new), `stdlib.rs`/`value.rs`
additions (`json`/`db`/`auth`/`log` natives), `interpreter.rs`'s
`http_serve` implementation and hand-rolled HTTP/1.1 parsing,
`crates/typechecker/src/stdlib.rs`'s new module signatures,
`crates/cli/src/main.rs`'s `AINT_MODEL_URL` wiring, and
`examples/customer_support/` (`aint.toml`, `server.an`, `worker.an`,
`priority_logic_test.an`). 380 tests total across the workspace, all
passing, plus 4/4 `aint test` cases in the application itself and a
hand-verified live HTTP session covering every route including the
AI-touching one's honest failure mode.
