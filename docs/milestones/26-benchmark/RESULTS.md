# Milestone 26 — Results

See `SPEC.md` for methodology. Every number below came from actually
running both implementations on the same machine, in the same
session, back to back.

## Lines of code

| | AINT | Python |
|---|---|---|
| Application (server + worker) | 285 (`server.an` 229, `worker.an` 56) | 432 (`main.py` 150, `worker.py` 44, `ai.py` 118, `models.py` 69, `db.py` 51) |
| Tests | 62 (`priority_logic_test.an`) | 139 (`test_main.py`, 10 cases covering both the logic and the full HTTP lifecycle) |
| **Total** | **347** | **571** |
| Language-level code needed to make this possible at all | **832** (new Rust in `aint-runtime`/`aint-typechecker`/`aint-cli`, milestone 25) | **0** (FastAPI/Pydantic/bcrypt/openai/langgraph already existed) |

AINT's *application* code is shorter — 285 lines against Python's 432
— even though it hand-builds JSON field access, session lookup, and
list-filtering-by-recursion that Pydantic, SQLAlchemy-style queries,
and list comprehensions give Python for free. But that comparison is
incomplete on its own: AINT needed **832 new lines of interpreter/
stdlib Rust** (a real HTTP server, a database, password hashing,
logging — see `docs/milestones/25-real-application/SPEC.md`) to reach
the starting line Python's ecosystem was already standing at. Python
paid zero language-level cost; AINT paid 832 lines once, and gets to
reuse them for every future AINT application. Which framing is "the
real number" depends on whether the question is "cost to build this
one app" (AINT wins on application code, loses overall once the
one-time stdlib cost is counted) or "cost to build the *next* one"
(the 832 lines are sunk; AINT's application-code advantage stands on
its own after that).

## Memory (idle, after warm-up)

| | Working set |
|---|---|
| AINT (`--release`, single process) | **8.8 MB** |
| Python (`uvicorn` + FastAPI + Pydantic + LangGraph loaded) | **107.6 MB** |

**~12x.** Not a close comparison — a compiled, single-binary
interpreter against a Python interpreter plus a real dependency stack
(FastAPI, Starlette, Pydantic's compiled core, LangGraph, LangChain's
`langchain-core`, the `openai` SDK) all resident in memory before the
first request even arrives.

## Binary / dependency footprint

| | Size |
|---|---|
| AINT release binary (`aint.exe`), self-contained, zero runtime deps | **3.2 MB** |
| Python: interpreter + 81 installed transitive packages | **~139 MB** on disk |

The same story from a different angle: deploying AINT means shipping
one 3.2 MB file. Deploying the Python equivalent means shipping (or
building an image around) a Python interpreter and 81 packages'
worth of dependencies — FastAPI alone pulls in Starlette, Pydantic
pulls in `pydantic-core` (a compiled Rust extension, notably) and
`annotated-types`, and LangGraph pulls in `langchain-core`,
`langgraph-checkpoint`, `langgraph-sdk`, and `langsmith`.

## Latency

Two routes, chosen to separate "how slow is the actual cryptographic
work" from "how much overhead does the language/framework add":

| Route | AINT mean | Python mean | AINT p95 | Python p95 |
|---|---|---|---|---|
| `/login` (bcrypt-dominated) | 209.06 ms | 217.08 ms | 212.80 ms | 220.21 ms |
| `/tickets/list` (no crypto — DB read + JSON) | **1.15 ms** | **3.40 ms** | 1.57 ms | 4.30 ms |

On the bcrypt-dominated route, AINT and Python are within ~4% of each
other — expected and correct: `bcrypt`'s cost function is deliberately
slow regardless of which language calls it, and at the default work
factor it dominates total request time by two orders of magnitude
over everything else in the request path. Neither implementation's
own overhead is visible here; this row exists specifically to show
that.

On the route with no cryptography, the actual runtime/framework
overhead becomes visible: AINT is **~3x faster** (1.15 ms vs. 3.40 ms
mean). This reflects both a compiled interpreter vs. a dynamic one,
and `http_serve`'s deliberately minimal hand-rolled HTTP/1.1 parsing
(`docs/milestones/25-real-application/SPEC.md`) against FastAPI/
Starlette/Pydantic's considerably richer (and heavier) request-
handling and validation stack — richness that buys real things (see
"Testability," below) at a real latency cost.

## Failure handling

Both were checked against the actual failure this application has:
`classify_sentiment`/`classify_sentiment`'s tier lookup with nothing
real behind them.

**AINT**: an unconfigured `infer` call produces a typed
`RuntimeError::ModelError`, propagated automatically through
`http_serve`'s dispatch with no error-handling code required anywhere
in `server.an` — verified live in milestone 25's acceptance:
`model error: no mock response configured for `classify_sentiment``,
surfaced as an HTTP `500` with that exact message in the body.

**Python**: nothing in `main.py` wraps the `ai.decide_priority` call
in a `try`/`except`. An unhandled Python exception under FastAPI
becomes a generic `500 Internal Server Error` with **no detail** in
the response body by default (a deliberate FastAPI/Starlette security
default — don't leak internals to the client) — the developer has to
opt in to see anything more than that without reading server logs.

This cuts both ways, honestly: AINT's automatic full-detail
propagation is more convenient during development and gives a
genuinely more actionable error message *for free*; it is also,
unaddressed, an information-disclosure concern a production deployment
would need to guard against (`RuntimeError::Display` text going
straight into a `500` body is not something `http_serve` currently
redacts — see `crates/runtime/src/interpreter.rs`'s `http_serve`).
Python's default is the more conservative, more production-appropriate
one, at the cost of the developer having to do more work to get a
useful error message back during development. Neither is strictly
"better" — they're different defaults with different tradeoffs, and
AINT's isn't fixed here since it wasn't in this milestone's scope.

## Testability

**AINT**: two separate mechanisms, for a documented reason
(`docs/milestones/25-real-application/SPEC.md`'s "`aint test`'s
re-execute-every-non-test-statement design is fundamentally
incompatible with a file that also has a blocking top-level
statement"). The AI-decision logic is tested via `aint test`/`mock`
(4 cases, `priority_logic_test.an`, deliberately duplicating
`priority_for` out of `server.an` since neither cross-file `import`
nor "skip this statement during test setup" exists yet); the HTTP
surface is tested by spawning the real binary as a real process and
sending real requests (2 Rust-level integration tests in
`crates/cli/tests/`).

**Python**: one mechanism, one file. `pytest` + FastAPI's
`TestClient` + `monkeypatch` cover *both* the AI-decision logic
(monkeypatching `ai.classify_sentiment`/`ai.lookup_account_tier`,
mirroring AINT's `mock`) *and* the full HTTP lifecycle — register,
login, create a ticket, list it, resolve it — in the same 10-case
suite, in-process, no server process spawn needed at all
(`TestClient` drives the ASGI app directly).

This is a genuine, unambiguous Python advantage for this application
shape: general-purpose testing infrastructure (an in-process ASGI test
client, a monkeypatch fixture) composes with an HTTP server the way
AINT's test-block-per-file, re-execute-everything model currently
doesn't. AINT's `mock`/`assert`/`test` being real language keywords is
a genuine, distinctive strength for testing AI-touching *logic*
specifically (no test double library, no monkeypatching machinery
needed — see every earlier milestone from 08 onward); it does not yet
extend to testing a whole running program the way Python's ecosystem-
level tooling does.

## Observability

**AINT**: `TraceRecord` (milestone 14) is built into the runtime
itself — every `infer`/`tool` call automatically gets an `Inference
#N`/`Tool Call #N` record (model, tokens, latency, outcome), with zero
application code required. Notably unused by
`examples/customer_support/`: nothing in `server.an` exposes
`Interpreter::traces()` anywhere, so this capability exists at the
language level but isn't surfaced through the demo app's own HTTP
API — a real gap in the *application*, not the language.

**Python**: no equivalent built-in. This benchmark's `main.py`/
`worker.py` use plain `logging` calls (matching AINT's own `log`
module usage in `server.an`/`worker.an` — a fair, direct comparison at
that level). For AI-call-specific tracing, LangGraph's ecosystem offers
`langsmith` (installed as a transitive dependency here, not wired up)
— an opt-in third-party service, arguably *richer* than milestone 14's
tracing if adopted, but external and not part of the language the way
`TraceRecord` is part of AINT's.

## Cost

Not measured directly — see `SPEC.md` for why. Per-token API pricing
is a property of the model backend (OpenAI, a local vLLM/Ollama
deployment, etc.), identical regardless of which language calls it;
neither `HttpModel` (AINT) nor the `openai` SDK (Python) changes what
the backend charges per token. The measurable cost proxy is
infrastructure: the memory and binary-size numbers above translate
fairly directly into hosting cost at scale (smaller container images,
less RAM per instance, more instances per host) — a real, if modest,
advantage for AINT in a high-instance-count deployment, and not
something either implementation's own code controls.

## Summary

Nothing here is a clean sweep either direction:

- **Lines of code**: AINT's application code is shorter; counting the
  one-time stdlib cost it needed to get there, Python's total is
  shorter for *this one app*, but AINT's cost doesn't repeat for the
  next one.
- **Memory / binary size**: AINT wins clearly, ~12x and ~40x
  respectively — the most unambiguous numbers in this milestone.
- **Latency**: roughly even when cryptography dominates (as it will
  for any auth-touching route); AINT ~3x faster when it doesn't.
- **Failure handling**: different defaults, different tradeoffs —
  AINT's automatic detail is convenient but currently unguarded;
  Python's default is safer but requires more developer effort to get
  useful errors.
- **Testability**: Python's general-purpose ecosystem tooling
  currently covers more ground (logic *and* HTTP, one suite) than
  AINT's language-native-but-narrower `mock`/`test` plus a separate
  process-spawning integration test.
- **Observability**: AINT has more *built in*; Python's ecosystem has
  more *available*, opt-in.
- **Cost**: identical at the model-API level; AINT's infrastructure
  footprint is the real, measured advantage.
