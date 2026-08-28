# Milestone 25 — Real application

## Scope

`ROADMAP.md`:

> Build something non-trivial entirely in AINT — a customer support
> system with an HTTP API, a database, auth, inference, tool calls,
> background jobs, logging, and tests. If AINT can't comfortably build
> this, the abstractions aren't right yet.

AINT had none of the infrastructure this needs before this milestone:
no HTTP server, no persistence, no auth, no logging. Given the choice
between building the app on top of fakes (tool-call stubs standing in
for a real server/database) or actually adding that infrastructure to
the language, this milestone takes the second, larger path — a real
HTTP server, a real file-backed database, and real password hashing,
all as new `aint-runtime` stdlib natives, then a real customer-support
application written entirely in AINT on top of them.

## What "background jobs" means, given AINT's actual architecture

Worth deciding explicitly before writing anything: AINT's runtime has
been single-threaded since milestone 07, deliberately (`Value`'s
`Rc`/`RefCell` aren't `Send`). In-process concurrent background tasks
(`tokio::task::spawn_local` under a `LocalSet`) were considered and
rejected — not because they're impossible, but because they'd be
retrofitting real concurrency onto an architecture that has
specifically never had it, for a demo app that doesn't need it.

**Background jobs here means a separate AINT program, run as its own
OS process, polling a shared database table.** The HTTP server writes
a job record to a `jobs` table instead of doing slow work inline;
a second `.an` program (`worker.an`) polls that table and processes
pending jobs. This is genuinely how a lot of real systems separate
web-request latency from background work — process-level separation,
not in-process concurrency — and it needs zero new concurrency
primitives: it's just the `db` module, used from two independent
programs. Consistent with the architecture rather than fighting it.

## New stdlib modules

All new natives live in `aint-runtime` (`crates/runtime/src/stdlib.rs`
for the synchronous ones, `interpreter.rs` for `http_serve`, which
needs to call back into the interpreter itself) — the same module
structure `math`/`string`/`time`/`collections` already established in
milestone 06, gated behind `import`.

### `json`

```
json_get(json: String, key: String) -> Option<String>
json_object(keys: List<String>, values: List<String>) -> String
```

Flat objects only — string-valued fields, no nesting, no arrays. AINT
has no record/struct type (a real gap, not fixed here — see
"Explicitly out of scope"), so a "ticket" or "user" is represented as
a flat JSON string, read and written field-by-field through these two
natives, backed by `serde_json` (already a dependency since milestone
16's `HttpModel`). `json_get` returning `Option<String>` reuses the
same reasoning `distribution_require_confidence` already established:
a real "maybe absent" case gets `Option<T>`, not a sentinel string.

### `db`

```
db_insert(table: String, id: String, json: String) -> Bool
db_get(table: String, id: String) -> Option<String>
db_list(table: String) -> List<String>
db_update(table: String, id: String, json: String) -> Bool
db_delete(table: String, id: String) -> Bool
```

File-backed, not an external database process — "embedded" in the
literal sense: each table is one newline-delimited-JSON file under
`.aintdb/<table>.jsonl` in the current working directory, each line an
object `{"id": ..., "data": ...}`. `db_insert`/`db_update`/`db_delete`
rewrite the whole file (simplest correct implementation; no concurrent
writer story is needed or built — see "Explicitly out of scope").
Chosen over embedding a real SQL engine (e.g. SQLite via `rusqlite`,
which needs a C toolchain — the exact class of problem `native-tls`
was already chosen over `rustls` to avoid in milestone 16) or a
key-value crate (`sled`, `redb`): a hand-rolled JSONL store needs one
already-present dependency (`serde_json`) and is trivially
inspectable (`cat .aintdb/tickets.jsonl`), which matters for a demo
whose whole point is being legible.

### `auth`

```
auth_hash_password(password: String) -> String
auth_verify_password(password: String, hash: String) -> Bool
auth_generate_token() -> String
```

Real password hashing (`bcrypt`, a well-established pure-Rust-callable
crate, not a hand-rolled scheme) — a demo app is still real code, and
"this milestone's fake auth" would be a strange thing to ship even in
an example. `auth_generate_token` produces an opaque random session
token (via `rand`, already a dependency since milestone 10's
`distribution_sample`) — session storage is just a `db` table mapping
token to user id, not a separate primitive.

### `log`

```
log_info(message: String) -> Unit
log_error(message: String) -> Unit
```

Writes a timestamped line to stderr. Deliberately not more than this —
levels beyond info/error, structured fields, or a log file target
aren't needed for what the demo app actually does.

### `http`

```
http_serve(port: Int) -> Unit
```

The one genuinely async new native, and the one that can't be a plain
`stdlib::call` — it needs to call back into the interpreter itself for
every request, so it's implemented directly on `Interpreter`, not in
the stateless `stdlib` module (the same reason `eval_inference`/
`eval_tool_call` are `Interpreter` methods rather than free
functions).

**No routing framework — a single, fixed dispatch convention.** The
AINT program defines exactly one function, `fn handle_request(method:
String, path: String, body: String) -> String`; `http_serve` looks it
up by that name and calls it once per request, using the return value
as the response body (`200 OK`, `Content-Type: application/json`).
Routing (matching `method`/`path` to the right logic) happens *inside*
`handle_request`, in ordinary AINT `if`/`else` — the same reason
milestone 22's bytecode VM leans on AINT having no loops to make
control-flow compilation tractable, this leans on AINT having no
string-splitting/regex primitives to make "no router needed" the
honest, not just simpler, choice: exact-path matching only, resource
identifiers passed via the request body or query string, never via a
`/tickets/:id`-shaped path segment.

**Hand-rolled HTTP/1.1 over a raw `TcpListener`, not `hyper`/`axum`.**
Real web frameworks want `Send + 'static` request-handling futures for
their executors; `Value`/`Interpreter` are `Rc`-based and deliberately
never `Send` (milestone 07, reaffirmed by milestone 21's memory-model
decision). Rather than fight that, `http_serve` reads a request line
and headers directly off the socket, reads exactly `Content-Length`
bytes of body if present, calls `handle_request`, and writes back a
`Content-Length`-framed response with `Connection: close` — enough to
be a real, `curl`-able HTTP/1.1 server, on the same single-threaded
runtime every other native already runs on. One request at a time,
by construction, not by an arbitrary choice — see "Explicitly out of
scope" for what real concurrency here would need.

## The application

`examples/customer_support/`:

- **`server.an`** — the HTTP API: register/log in users (`auth` +
  `db`), create/list/resolve support tickets, classify an incoming
  ticket's sentiment via `infer`, and — for a negative-sentiment
  ticket — call a `tool` to look up the customer's account tier before
  deciding priority. Routing is exact-path `if`/`else` inside
  `handle_request` (see "New stdlib modules: http," above).
- **`worker.an`** — the background half: drains the `jobs` table
  `server.an`'s ticket-creation path enqueues into, once per run (see
  "What 'background jobs' means," above, for why it isn't a poll
  loop).
- **`priority_logic_test.an`** — deterministic, offline `aint test`
  coverage for the `infer`-then-`tool` priority decision, via `mock`
  (see "What building this actually found" for why this duplicates
  rather than shares code with `server.an`).

`aint.toml` makes it a real package (milestone 23), even though every
`.an` file in it is still independent — AINT still has no cross-file
`import`, so "the app" is three programs sharing a database (and, for
the test file, sharing logic by necessary duplication), not one
program split across files.

## What building this actually found

This is the part of the milestone the roadmap's own framing asks for
directly — "if AINT can't comfortably build this, the abstractions
aren't right yet." Several real, load-bearing language gaps surfaced
only once real application code was written against them, not from
reading the spec:

- **No record/struct type, and no way to construct `Option<T>` from
  AINT source at all.** Named in advance (see "New stdlib modules:
  json"); confirmed while writing `server.an` — `find_user_by_email`
  and `authenticate` both want to return "found this, or nothing," and
  the only tool available is an empty-string sentinel, since `Some`/
  `None` have never been expressible as AINT syntax (every existing
  `Option` value has only ever come out of a native function, like
  `distribution_require_confidence`).
- **No list concatenation or incremental list construction of any
  kind.** `+` only ever worked on `Int`/`Float`; there is no
  `List<T> + List<T>`, no `push`, no way to build a new list one
  element at a time across recursive calls. This blocks the obvious
  way to write "filter this list and return the matches" as AINT code.
  The workaround used throughout `server.an` (`tickets_for_user`,
  `find_user_by_email`): recursively accumulate a **string** result via
  `string_concat` instead of a list — building the *response body*
  directly, never an intermediate `List<String>`. Real, correct, and a
  genuine constraint on what kind of data-shaping code AINT can
  express today.
- **No `Int`/`String` conversion.** `worker.an`'s job count is
  reported with `print` (which accepts any `Value`) specifically
  *because* there's no way to `string_concat` an `Int` into a message.
- **No boolean negation, `<=`, or `>=`.** Confirmed already in
  `CONTRIBUTING.md`'s design constraints, but this milestone is the
  first time it actually shaped control flow repeatedly — every
  "otherwise" branch in `server.an`/`worker.an` had to be a real
  `else`, and every off-by-one boundary (`index == length - 1`, not
  `index >= length`) had to be phrased in terms of `<`/`==`/`>` only.
- **`aint test`'s re-execute-every-non-test-statement design
  (milestone 15) is fundamentally incompatible with a file that also
  has a blocking top-level statement.** `run_tests` re-runs every
  non-`Test` top-level statement — including a bare `await
  http_serve(...)` — before each test body, to give every test an
  identical, isolated setup. That's the right design for what
  milestone 15 was built to cover (declarations, not an entry point
  that never returns), but it means `server.an` genuinely cannot
  contain both its own `test` blocks and its `await http_serve(8080)`
  entry point — the first test run would hang forever trying to
  re-execute the server start. The fix used here:
  `priority_logic_test.an` duplicates just the AI-decision logic
  (`priority_for`, the `infer`/`tool` declarations) into its own small,
  test-only file. A real fix would need either cross-file `import`
  (milestone 23's own named gap) so the logic isn't duplicated, or a
  way to mark a top-level statement as "`aint run`-only, skip during
  `aint test` setup" — neither built here.
- **Tool calls have never had a real backend — `MockTool` is the only
  one that has ever existed, in this project's entire history.**
  Not a new gap; true since milestone 11. This milestone's `tool
  database_get_account_tier` is exercised the same way every other
  `tool` in this codebase always has been: through `aint test` and
  `mock`, never live. Named explicitly here because this is the first
  milestone where "run it for real" was the actual goal, so the gap
  is worth surfacing loudly rather than letting the demo quietly avoid
  it.
- **`aint run` had no way to reach a real `Model` at all before this
  milestone**, even though `HttpModel` has existed since milestone 16
  — every `infer` call outside `aint test` failed with "no mock
  response configured," unconditionally. Fixed here: `AINT_MODEL_URL`
  (plus `AINT_MODEL_NAME`/`AINT_MODEL_API_KEY`) now lets `aint run`
  use `HttpModel` instead of the default unconfigured `MockModel` —
  small, genuinely useful, and the first time any AINT program has
  been able to run a live inference outside a test.

## Explicitly out of scope

- **A record/struct language feature.** JSON-string-as-record is a
  real, working substitute, not a permanent design position — see
  "New stdlib modules: json," above. Adding actual structs is
  language-design work on the scale of a milestone by itself, not
  something to retrofit as a side effect of needing ticket records.
- **Concurrent request handling.** `http_serve` handles exactly one
  connection at a time. Real concurrency here would mean either
  `tokio::task::spawn_local` under a `LocalSet` (rejected above, for
  the same single-threaded-architecture reasons background jobs
  were) or moving `Value` off `Rc` entirely (`Arc`/`Mutex`) — a
  reversal of milestone 21's just-made memory-model decision, not
  something to casually undo for one native function.
- **Concurrent-writer safety in `db`.** Each `db_*` call rewrites a
  whole file; two processes writing the same table at once can lose
  an update. Fine for a demo where `server.an` and `worker.an` write
  disjoint tables (`tickets`/`users`/`sessions` vs. `jobs`), not fine
  for a real multi-writer workload.
- **Nested/structured JSON**, arrays of objects, or numeric/boolean
  JSON field types beyond what's representable as a `String` — `json`
  stays flat, string-valued, by design (see above).
- **TLS.** `http_serve` is plain HTTP, matching "an embedded demo
  server," not "a production listener."

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
