# Milestone 28 — Production language (1.0)

## Scope

`ROADMAP.md`:

> Native compilation path, optimized runtime, package ecosystem,
> stable specification, backward compatibility policy, security
> audit, performance work. This is 1.0.

Seven names. Three of them name things that aren't buildable, alone,
inside this repository: native compilation (LLVM integration is a
substantial compiler-backend project by itself), a package ecosystem
(a real registry is a hosted service, same reasoning
`docs/milestones/23-package-manager/SPEC.md` already gave for why
`aint add` only takes local paths), and a genuine security audit
(a credible audit is independent, external review — a self-review
pass by the same author who wrote the code is a different, narrower
thing, useful but not that). This milestone does the other four for
real — a stable specification, a backward compatibility policy, a
security *pass* (self-review, explicitly not an audit — see below),
and targeted performance work — and states the other three as
deferred with specific reasons, not silently skipped.

## What this milestone actually builds

### Stable specification

`docs/SPECIFICATION.md` — a single, current, accurate reference for
the whole language and runtime as implemented: lexical structure,
every statement and expression form, the type system, the execution
model (both executors), model backend configuration, the full error
taxonomy, every stdlib module, the testing model's real constraints,
and a consolidated "known gaps" section pointing at where each gap was
originally found.

This replaced, rather than supplemented, treating `docs/RUNTIME.md` as
current. `RUNTIME.md` was written before implementation began and was
never updated — it shows dotted tool names (`database.lookup_customer`,
rejected by the language entirely), a `model { }` block that was never
built, and a rich distribution-literal `mock` syntax milestone 15
explicitly didn't implement. Shipping a "1.0" specification without
addressing a doc that contradicts it outright would be worse than not
having consolidated one at all — `RUNTIME.md` now carries an explicit
header pointing at `SPECIFICATION.md` as the accurate reference, with
its historical design ideas kept, not deleted. `docs/ARCHITECTURE.md`
had the same problem in miniature (crate layout missing `aint-vm`/
`aint-package`/`aint-fmt`, a "what's deferred" list including things
that shipped in milestones 22–24) and was corrected the same way.

### Backward compatibility policy

`docs/COMPATIBILITY.md` — semver, what's covered (grammar, type-
checking rules, stdlib signatures, CLI surface, file formats, error
variant names) and what explicitly isn't (everything
`SPECIFICATION.md` §11 already lists as a gap has no existing behavior
to protect; internal Rust crate APIs were never a public surface —
every crate is `publish = false`), and a deprecation process. Also
honest about its own current limits: there's no compatibility test
corpus enforcing this mechanically yet.

### Security pass (not an audit)

A real, motivated self-review of the runtime's actual attack surface —
not a checklist exercise. Two real findings, both fixed:

1. **Path traversal in `db`** (`crates/runtime/src/db.rs`). `table`
   flowed straight into `base_dir.join(format!("{table}.jsonl"))` with
   no validation at all. `db_insert("../../../etc/cron.d/x", ...)`
   would have attempted to write outside `.aintdb` entirely. Not
   exploitable by `examples/customer_support/` today (every table name
   there is a hardcoded string literal), but the *primitive* had no
   guard, and a future AINT program that builds a table name from
   request input would have been vulnerable. Fixed: `valid_table_name`
   requires every character to be alphanumeric, `_`, or `-` —
   conservative allowlisting rather than blocklisting `..`, so it
   doesn't need to reason about every path-syntax edge case
   (`\` on Windows, encoded traversal sequences, absolute paths) one
   at a time. Five new tests in `db.rs` cover it directly, including
   that ordinary hyphenated/underscored table names still work.
2. **Full error detail leaking into `http_serve`'s `500` responses**
   (`crates/runtime/src/interpreter.rs`). `RuntimeError::Display` text
   — which can echo back request content, file paths, or internal
   state — was written straight into the response body. Found and
   named as an open gap in `docs/milestones/26-benchmark/RESULTS.md`'s
   "Failure handling" comparison (Python/FastAPI's default is to hide
   this); fixed here by logging the full error server-side
   (`eprintln!`) and sending a generic `"internal server error"` to
   the client — the same conservative default FastAPI/Starlette
   already apply.

Also checked, found clean: `bcrypt::verify` already fails closed
(`unwrap_or(false)`, not a panic) on a malformed hash; `HttpModel`'s
response parsing has no unwraps on network-sourced data (all
`Result`-based); `auth_generate_token` uses a real CSPRNG (`rand`'s
thread-local generator); SQL injection is structurally impossible
(there is no SQL in AINT — `benchmark/python/`'s SQLite queries were
separately checked to be parameterized, not string-formatted).

### Performance work

Quantified, not asserted: `fibonacci(30)` (a compute-heavy, shallow-
recursion benchmark — 1.3M calls), `--release`, best-of-three:

| Executor | Time |
|---|---|
| Tree-walking interpreter (`aint run`) | 3.93s |
| Bytecode VM (`aint run --vm`) | **0.29s** |

**~13x.** This is milestone 22's already-shipped work, measured
properly for the first time here. A second finding worth recording
directly: `examples/customer_support/worker.an` (deterministic-core
only — `db`, `json`, `log`, `collections`, `string`, no `infer`/
`tool`/`http`) runs correctly under `--vm` with *zero* changes,
because the VM's native-call dispatch resolves against the same
`stdlib::module_bindings` table the interpreter uses, generically —
milestone 25's `json`/`db`/`auth`/`log` natives, added three
milestones after the VM shipped, were automatically compilable
without the VM needing to know about them individually. The
generic-dispatch design decision from milestone 22 paid for itself
here, unprompted.

## Explicitly out of scope

- **Native compilation (LLVM or otherwise).** `ROADMAP.md` already
  said "still no LLVM" for milestone 22; nothing changed that
  calculus. A real native-codegen backend is a substantial project on
  its own, disproportionate to a slice of this milestone.
- **A real package registry.** No hosted service exists;
  `docs/milestones/23-package-manager/SPEC.md` already gave the same
  reasoning for `aint add` staying local-path-only. Unchanged here.
- **A genuine security audit.** What this milestone did is real (two
  actual vulnerabilities found and fixed) but is a self-review, not an
  audit — the same person who wrote the code reviewing it lacks the
  independence a credible audit needs. Named as a self-review
  throughout this document specifically so it isn't mistaken for one.
- **A compatibility test corpus.** `COMPATIBILITY.md`'s policy exists;
  the mechanical enforcement of it (frozen example programs + expected
  output, checked on every change) doesn't yet — stated directly in
  that document's own closing section.
- **A profiling-guided, broad performance-optimization pass.** One
  clear, already-shipped win was measured and documented (the VM); a
  systematic search for further hot spots would need profiling
  tooling (flamegraphs, `perf`) not practically usable in this
  environment, and wasn't attempted speculatively.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
