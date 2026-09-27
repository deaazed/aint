# Milestone 52 — static asset serving (foundation)

*(Corresponds to `FEEDBACK.md`'s proposed milestone 50 — renumbered to
52, the next free slot, per this repo's strictly-sequential-by-
completion-order convention; see milestone 50's own `ACCEPTANCE.md` for
why this repo renumbers rather than leaving gaps.)*

## Scope

Today `handle_request` is 100% responsible for every response byte —
`http_serve` has zero filesystem access anywhere in its own request
path. That means no favicon, no self-hosted image, no self-hosted font
is possible at all today, not even in principle: everything has to be a
`String` an AINT program builds and returns. This milestone: a second,
additive native, `http_serve_assets(port: Int, asset_root: String) ->
Unit`, which serves a file straight off disk for a `GET` request whose
path resolves under a declared local directory, before falling through
to `handle_request` unchanged for everything else.

**Deliberately additive, not a breaking change to `http_serve`.**
AINT has no optional/default parameters, so widening `http_serve`
itself to take a second argument would break every existing caller
(`aint-website`'s own `main.an` included). A second native, bound
alongside the first in the same `http` module table, needs zero changes
anywhere `http_serve(port)` is already called.

**First filesystem-serving capability in the runtime** — a real
departure from "`handle_request` owns every response byte," and the
first place request-path-derived data reaches `std::fs`/`tokio::fs` at
all. Gets the same security treatment milestone 28 already gave
`db`'s table names for the identical underlying reason (attacker-
influenced data becoming a filesystem path).

## Design

### The path-safety check

Mirrors `db.rs`'s `valid_table_name` — "deliberately conservative rather
than merely rejecting `..`," the same reasoning, adapted from a flat
name to a hierarchical path:

```rust
fn safe_asset_path(asset_root: &Path, request_path: &str) -> Option<PathBuf> {
    let relative = request_path.split('?').next().unwrap_or(request_path).trim_start_matches('/');
    if relative.is_empty() {
        return None;
    }
    for segment in relative.split('/') {
        let ok = segment != "." && segment != ".."
            && !segment.is_empty()
            && segment.chars().all(|c| {
                c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'
            });
        if !ok {
            return None;
        }
    }
    Some(asset_root.join(relative))
}
```

Every segment must consist only of letters, digits, `_`, `-`, and `.`
(needed for extensions and multi-part filenames, unlike `db`'s flat
table names — the one real difference from `valid_table_name`'s own
allowlist), and must not be exactly `.` or `..`. This is a structural
guarantee, not a runtime check against a canonicalized path: a string
built *only* from validated segments joined with `/` can never contain
a `..` component, so `asset_root.join(relative)` can never resolve
outside `asset_root`, by construction — the same "reject by
construction" posture `db.rs` already documents choosing over a
blocklist, for the same reason (no need to separately reason about
Windows/Unix path syntax differences, encoded traversal, or every other
way a blocklist could be bypassed — none of those characters are in the
allowlist to begin with). Percent-encoded traversal attempts
(`%2e%2e%2f...`) are rejected for free, since `%` itself isn't an
allowed character — the request parser never decodes the path before
this check runs.

Query strings are stripped before validation (`/logo.png?v=2` still
resolves `logo.png`) — `read_http_request`'s own `path` includes
whatever query string a client sent, unparsed, same as it always has.

### Serving

Inside `http_serve`'s existing loop, for a `GET` request only, before
the existing `handle_request` dispatch: if `safe_asset_path` resolves
and `tokio::fs::read` on the result succeeds, write a `200` response
with the file's bytes and an extension-derived `Content-Type`
(`asset_content_type`, new — a small, static, extension-to-MIME table:
`html`/`css`/`js`/`json`/`svg`/`png`/`jpg`/`jpeg`/`gif`/`ico`/`webp`/
`woff`/`woff2`/`txt`, falling back to `application/octet-stream` for
anything unrecognized), then move on to the next connection —
`handle_request` is never called for that request. Any failure at any
step (validation fails, the file doesn't exist, isn't a regular file,
can't be read) falls through to the existing `handle_request` flow
completely unchanged — an AINT program's own routing (including its own
`not_found` handler) still owns everything the asset root doesn't
serve. No new 404 path invented here.

**`write_http_response` widened from `body: &str` to `body: &[u8]`** —
needed for real binary content (images, fonts — milestone 53's own
subject) to ever be possible at all; every existing text call site
updated to pass `.as_bytes()`, content-type resolution for the existing
`handle_request` flow (`content_type_for`, unchanged) computed by the
caller before the call rather than inside it, matching
`asset_content_type`'s own shape and keeping the response-writer itself
oblivious to *why* a body is what it is.

## Explicitly out of scope

- **Directory listings** — a `GET` for a directory itself (no matching
  file) simply falls through to `handle_request`/`not_found`, same as
  any other miss.
- **Conditional requests** (`If-Modified-Since`/ETags), byte-range
  requests, or any caching headers — a real static file server's usual
  feature set, none of it needed for this foundation to be useful.
- **Actually using this for a favicon/logo/font** — milestone 53,
  depends on this.
- **Any change to `handle_request`'s own signature or contract.**

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
