# Milestone 26 — Benchmark against the status quo

## Scope

`ROADMAP.md`:

> Compare the milestone-25 application against the equivalent
> Python + Pydantic + an LLM SDK + LangGraph stack: lines of code,
> latency, memory, failure handling, testability, observability, cost.

A real, running Python equivalent (`benchmark/python/`) of
`examples/customer_support/` — same routes, same behavior, same
sentiment-then-tier priority decision — built with the stack the
roadmap names: FastAPI, Pydantic, the `openai` SDK, LangGraph. Every
number in `RESULTS.md` comes from actually running both, not
estimation.

## What was actually measured, and how

**Lines of code** — plain `wc -l` over each implementation, split into
application code and test code, plus (a category the roadmap doesn't
name but the comparison would be dishonest without) the *language-
level* code AINT had to gain to make the application possible at all:
milestone 25's stdlib additions (`json`/`db`/`auth`/`log`/`http`), a
one-time cost with no Python equivalent since its ecosystem already
had all of this.

**Memory** — process working-set size (`Get-Process`'s `WorkingSet64`)
for each server, idle after handling a few warm-up requests: AINT's
`--release` binary vs. `uvicorn` running the FastAPI app with
LangGraph/Pydantic/etc. already imported.

**Latency** — wall-clock time for 200–300 sequential HTTP requests
against each *running* server (`urllib`, timed per-request,
mean/median/p95 reported), for two different routes chosen
specifically to separate two different things being measured:

- `/login` — dominated by `bcrypt` password verification, which is
  deliberately slow (by design, to resist brute-forcing) regardless of
  which language calls it. This measures "does the language matter
  when real cryptographic work dominates."
- `/tickets/list` — a plain database read and JSON response, no
  cryptography at all. This isolates actual runtime/framework
  overhead.

**Failure handling** — read directly off both implementations' actual
code and actual behavior (not simulated): what happens when the
AI-touching path fails with nothing configured to answer it.

**Testability** — both test suites were actually run: `aint test` +
4 `mock`-driven cases (`priority_logic_test.an`) plus 2 Rust-level CLI
integration tests, vs. `pytest` + 10 cases (`test_main.py`) using
`TestClient` and `monkeypatch`.

**Observability** — compared directly: AINT's built-in `TraceRecord`
(milestone 14, `Inference #N`/`Tool Call #N`, automatic, no code
required) vs. Python's options (manual `logging` calls, the way this
benchmark's Python app does it, or an opt-in third-party service like
LangSmith, which `langgraph` pulls in as a transitive dependency and
integrates with natively).

**Cost** — not measurable directly: neither implementation calls a
real, paid model in this benchmark (both mock/stub the LLM call for
deterministic, offline testing — the same reasoning
`docs/milestones/25-real-application/SPEC.md` gives for why ticket
creation was tested via `aint test`/`mock` rather than live). Handled
analytically instead: per-token API cost is a property of the model
backend, not the calling language, so it's identical either way;
what *does* differ is measured directly under memory/binary-size,
below, as the actual infrastructure-cost proxy.

## Design decisions

**Python gets to use its real, idiomatic ecosystem, not a
constrained reimplementation of AINT's own primitives.** SQLite (via
the standard-library `sqlite3`, not a hand-rolled JSONL file) for
storage, real Pydantic models for validation, a real `bcrypt` package,
a real (if request-mocked) `openai` SDK call, a real `langgraph`
`StateGraph`. Making Python reimplement AINT's from-scratch JSONL
store would understate exactly the thing this benchmark exists to
show — how much a mature ecosystem gives you for free.

**Same behavior, not same internals.** Every route in
`benchmark/python/main.py` matches `server.an` route-for-route,
request/response-shape-for-shape, and was checked against the same
scenarios (register, duplicate rejection, login, wrong password,
unauthenticated access, empty ticket list, full create-list-resolve
lifecycle, and all four sentiment/tier priority combinations) — not
just "does it have the same route names."

**Debug vs. release matters, so `aint` was benchmarked as `--release`,
not the `cargo build` debug binary every other milestone's manual
testing has used.** An unfair debug-vs-optimized comparison would be
worse than not measuring latency at all.

## Explicitly out of scope

- **A real, paid LLM call.** See "Cost," above — deliberately not
  run, for the same reason `examples/customer_support/`'s own tests
  don't run one either.
- **Load/concurrency benchmarking.** `http_serve` handles one
  connection at a time by construction (`docs/milestones/25-real-
  application/SPEC.md`); `uvicorn` defaults to a single worker too in
  this setup. Comparing concurrent-request throughput would mostly
  measure whether either was configured for concurrency, not the
  languages themselves.
- **A from-scratch LangGraph deep-dive.** The graph here is
  deliberately minimal (two nodes: classify, then conditionally look
  up account tier) — enough to genuinely use the framework the
  roadmap names, not a showcase of everything LangGraph can do.

## Outcome

See `RESULTS.md` for the full numbers and analysis; summarized in
`ACCEPTANCE.md`.
