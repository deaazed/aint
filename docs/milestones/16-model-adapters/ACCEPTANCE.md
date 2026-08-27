# Milestone 16 — Model adapters — acceptance

## Scope

See `SPEC.md`. `HttpModel` — a real `Model` implementation reaching
any OpenAI-compatible chat completions endpoint (vLLM, Ollama, OpenAI
itself), with tool-calling and `Distribution<T>` support explicitly,
honestly deferred rather than half-built.

## Acceptance criteria

- [x] `HttpModel::new(base_url, model)` implements `Model`, doing a
      real `POST {base_url}/chat/completions` via `reqwest`. One type
      serves all three named vendors — the base URL is the only thing
      that changes, never source code, matching "source code never
      names a vendor" literally.
- [x] Requests are built from a type-directed natural-language prompt
      (`expected_shape`); responses are parsed back against the
      declared return type (`Bool`/`Int`/`Float`/`String`/`Enum`) —
      verified for each of those five cases.
- [x] An `Enum`-typed response is *not* validated against real variant
      names inside `HttpModel` — confirmed by a test whose mock server
      returns a value and asserting it comes back as the raw
      `Value::Enum`, unvalidated. That validation is milestone 09's
      schema-validation layer, which runs downstream in `Interpreter`
      on `HttpModel`'s output exactly as it already does for
      `MockModel`'s.
- [x] A `Distribution<T>`-returning request and a request with
      non-empty `available_tools` are both rejected immediately, with
      a clear `RuntimeError::ModelError`, before any network call is
      made — verified directly (pointed at an unroutable address to
      prove no request is attempted).
- [x] Every real failure mode produces a clear, positioned
      `ModelError`, not a panic or an opaque error: connection
      failure, non-2xx HTTP status, malformed JSON, and a response
      that doesn't parse against the expected type — all four
      verified against a hand-rolled local HTTP responder (a
      `TcpListener` reading one request and writing back a controlled
      response), not a mocking crate or a live vendor endpoint. No API
      key or network access was needed to write or run any of these
      tests.
- [x] `aint run`/`aint test` are completely unaffected — `HttpModel`
      is a new, additive type; nothing selects it by default, and no
      existing behavior changed.
- [x] `cargo test --workspace` passes with no regressions: 257 tests
      total, up from 248 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Notable finding during implementation

The obvious choice for `reqwest`'s TLS backend, `rustls-tls`, failed
to build on this machine: its `ring` dependency needs a working C
toolchain to compile its assembly routines, and `gcc.exe` wasn't
correctly configured for this `x86_64-pc-windows-gnu` target
(`ToolExecError` during `ring`'s build script). Switched to
`native-tls`, which uses the OS's own TLS stack (SChannel on Windows)
via the `schannel` crate — pure Rust bindings to a Windows API, no C
compiler required. Documented in `Cargo.toml` directly, not just here,
since it's a real constraint on this dependency choice, not an
arbitrary preference.

## Explicitly out of scope

See `SPEC.md` — tool-calling and `Distribution<T>` through
`HttpModel` (both with concrete reasons, not just "later"), a
deployment-config file format or CLI wiring to select `HttpModel`,
streaming/retries/timeouts (17), and any vendor-specific feature
beyond the shared OpenAI-compatible wire format.

## Outcome

Satisfied by `crates/runtime/src/http_model.rs` (new: `HttpModel`,
request/response types, prompt construction, response parsing) and
`crates/runtime/Cargo.toml` (`reqwest`, `serde`, `serde_json`). 257
tests total across the workspace, all passing: 9 new tests covering
five successful-parse cases, three real-failure-mode cases, and two
reject-before-networking cases.
