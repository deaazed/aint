# Milestone 52 — static asset serving (foundation) — acceptance

## Scope

See `SPEC.md`. A second, additive native, `http_serve_assets(port: Int,
asset_root: String) -> Unit`, serving a real file off disk for a `GET`
request resolving safely under a declared directory, before falling
through to `handle_request` unchanged. First filesystem-serving
capability in the runtime.

## Acceptance criteria

- [x] `NativeFunction::HttpServeAssets` added alongside `HttpServe`
      (`crates/runtime/src/value.rs`) — same `is_async` treatment, own
      `name()` entry, bound in both `crates/runtime/src/stdlib.rs`'s
      `module_bindings` and `crates/typechecker/src/stdlib.rs`'s
      module table (`async_sig("http_serve_assets", [Int, String],
      Unit)`) — the exact two-table convention every prior stdlib
      addition already follows.
- [x] `http_serve`'s own signature and every existing caller (including
      `aint-website`'s `main.an`) are completely unaffected —
      `http_serve(port)` stays exactly one argument; nothing about it
      changed except its own `async fn http_serve` now takes an
      internal `asset_root: Option<&Path>` (always `None` from the
      original native, `Some` only from `http_serve_assets`).
- [x] `safe_asset_path` (`crates/runtime/src/interpreter.rs`) —
      mirrors `db.rs`'s `valid_table_name`'s "deliberately conservative
      rather than merely rejecting `..`" posture, adapted from a flat
      name to a hierarchical path: every segment must be letters/
      digits/`_`/`-`/`.` only, and not exactly `.` or `..`. A
      structural guarantee (a path built only from validated segments
      can never contain a `..` component), not a canonicalize-and-
      compare check. Verified directly, including percent-encoded and
      backslash-based traversal attempts, both rejected for free since
      neither `%` nor `\` is an allowed character:
      `a_traversal_attempt_is_rejected_however_its_spelled`,
      `a_query_string_is_stripped_before_resolving`,
      `an_empty_or_root_only_path_resolves_to_nothing`.
- [x] `asset_content_type` — extension-only (no body to sniff, unlike
      `content_type_for`), `application/octet-stream` fallback (not
      `content_type_for`'s `application/json`, which exists only to
      preserve pre-milestone-52 JSON-API behavior). Verified directly:
      `asset_content_type_is_extension_only_with_an_octet_stream_default`.
- [x] `write_http_response` widened from `body: &str` to `body: &[u8]`
      — real binary content is never guaranteed valid UTF-8. Every
      existing text call site updated to pass `.as_bytes()`;
      content-type resolution moved to each call site
      (`content_type_for` for `handle_request`'s responses,
      `asset_content_type` for a served file) rather than staying
      inside the response-writer, which no longer needs to know why a
      body is what it is.
- [x] **A real integration test over a genuine TCP connection**
      (`crates/cli/tests/http_server.rs`,
      `http_serve_assets_serves_real_files_and_rejects_traversal`,
      matching `FEEDBACK.md`'s own recommended verification shape): a
      real file served with the right bytes and `Content-Type`; a
      `/../...` traversal attempt planted to reach a real file one
      directory above the declared root — confirmed it never does, and
      falls through to `handle_request` instead; a path that doesn't
      exist under the root also falls through, same as any other miss.
- [x] `docs/SPECIFICATION.md`'s `http` stdlib row updated.
- [x] `cargo test --workspace`: 0 failures (5 new unit tests + 1 new
      integration test). `cargo clippy --workspace --all-targets`:
      clean. `cargo fmt --check`: clean.

## Renumbering note

`FEEDBACK.md` proposed this as its own milestone 50. Shipped here as
52, the next free slot, per this repo's strictly-sequential-by-
completion-order convention.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Directory listings, conditional requests, byte-range requests,
  caching headers.
- Actually using this for a favicon/logo/font — milestone 53.
- Any change to `handle_request`'s own signature or contract.
