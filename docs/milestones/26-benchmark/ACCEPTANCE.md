# Milestone 26 — Benchmark against the status quo — acceptance

## Scope

See `SPEC.md`. A real, running Python + FastAPI + Pydantic + LangGraph
equivalent of milestone 25's customer-support app
(`benchmark/python/`), measured against the AINT original across every
dimension `ROADMAP.md` names, with real numbers from actually running
both — not estimates.

## Acceptance criteria

- [x] `benchmark/python/` implements every route
      `examples/customer_support/server.an` has (`/register`,
      `/login`, `/tickets`, `/tickets/list`, `/tickets/resolve`),
      matching request/response shapes, using FastAPI, Pydantic,
      SQLite, `bcrypt`, the `openai` SDK, and a real `langgraph`
      `StateGraph` (classify -> conditionally look up account tier ->
      decide priority) — the exact stack `ROADMAP.md` names.
- [x] `benchmark/python/worker.py` mirrors `worker.an`'s background
      job draining, written as Python's own idiomatic perpetual poll
      loop (contrasted directly with why the AINT version can't be
      one — see `RESULTS.md`).
- [x] `benchmark/python/test_main.py`: 10 passing `pytest` cases
      covering register/duplicate-rejection/login/wrong-password/
      unauthenticated-access/empty-list (mirroring
      `crates/cli/tests/customer_support.rs`), all four sentiment/tier
      priority combinations (mirroring `priority_logic_test.an`), and
      a full create-list-resolve lifecycle test that AINT's own test
      suite has no single-mechanism equivalent for (see "Testability"
      in `RESULTS.md`) — verified by actually running the suite.
- [x] Lines of code measured directly (`wc -l`) for both, split into
      application/test/language-level-cost categories — not
      estimated.
- [x] Memory measured directly: both servers actually started
      (`aint` built `--release`, Python via `uvicorn`), each hit with
      warm-up requests, then real process working-set size read via
      `Get-Process`.
- [x] Binary/dependency footprint measured directly: `aint.exe`'s
      actual file size; the actual installed size and count of
      Python's transitive dependencies.
- [x] Latency measured directly: 200–300 real sequential HTTP
      requests against each real running server, for two routes
      chosen specifically to separate cryptographic cost from
      framework/runtime overhead — not a single aggregate number that
      would hide that distinction.
- [x] Failure handling compared against both implementations' actual
      code and actual observed behavior for the same real failure
      (an unconfigured/unimplemented AI-decision path) — not a
      hypothetical.
- [x] Observability compared directly: AINT's built-in `TraceRecord`
      (present in the language, but — an honest finding — unused by
      the demo app itself) against Python's `logging` plus the opt-in
      `langsmith` ecosystem tooling `langgraph` already pulls in.
- [x] Cost addressed analytically, with the reasoning for why it isn't
      measured directly stated plainly (neither implementation calls
      a real, paid model) rather than fabricated.
- [x] `docs/milestones/26-benchmark/RESULTS.md` presents every number
      with its own table and honest analysis — including findings
      that don't favor AINT (Python's shorter total line count once
      the one-time stdlib cost is included; Python's single-mechanism
      testability advantage; Python's safer default failure-handling
      behavior) alongside the ones that do (memory, binary size,
      non-crypto-route latency).
- [x] `benchmark/python/README.md` documents how to run and test the
      comparison stack, so the numbers in `RESULTS.md` are
      independently reproducible, not just asserted.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — a real paid LLM call,
load/concurrency benchmarking, and a from-scratch LangGraph deep dive.

## Outcome

Satisfied by `benchmark/python/` (`main.py`, `worker.py`, `ai.py`,
`models.py`, `db.py`, `test_main.py`, `requirements.txt`, `README.md`)
and `docs/milestones/26-benchmark/` (`SPEC.md`, `RESULTS.md`,
`ACCEPTANCE.md`). 10/10 Python tests passing; every quantitative claim
in `RESULTS.md` reproduced from an actual run on this machine in this
session (release-mode `aint`, real `uvicorn`-served FastAPI, real
sequential HTTP timing, real process memory readings).
