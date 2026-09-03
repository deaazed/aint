# Milestone 40 — URL/query percent-decoding

## Scope

`router_query_param` (`examples/router/router.an`) has always returned
a query value exactly as it arrived on the wire — `%20`/`%3C`/etc.
never decoded — because the stdlib had no primitive an AINT program
could use to decode them itself (no hex parsing, no char-code
arithmetic). Found testing `aint-website`'s `/try` page: a message
typed with a space or an apostrophe came back through as literal
percent-escapes instead of readable text. See `ROADMAP.md`'s Phase 3
framing, which left open whether the fix belonged in a native, inside
`router_query_param` itself, or both — this milestone does both.

## What this milestone actually builds

**`string_url_decode(s: String) -> String`** — strict RFC 3986
percent-decoding:

```an
import string
print(string_url_decode("Caf%C3%A9"))   // "Café"
```

Deliberately **does not** also map `+` to a space — that's the
`application/x-www-form-urlencoded` convention query strings layer on
top of plain percent-decoding, not something percent-decoding itself
does, and a native that silently did it would decode a URL *path*
segment's literal `+` wrong. A caller decoding a query value composes
it explicitly instead, now that milestone 39 gives it something to
compose with:

```an
fn decode_query_value(raw: String) -> String {
    return string_url_decode(string_replace(raw, "+", " "))
}
```

A `%` not followed by two valid hex digits is copied through literally
rather than erroring; a decoded byte sequence that isn't valid UTF-8 is
replaced lossily rather than rejected — this is data arriving off the
network, not a program the type checker already vetted.

**`examples/router/router.an`'s `find_param` now decodes the value it
returns**, using exactly the `decode_query_value` composition above —
so every existing caller of `router_query_param` (including
`aint-website`'s `/try` page) gets readable query values automatically,
with no change needed on their end. The key comparison itself is
unaffected (query parameter *names* aren't the thing that was found
broken).

## Design decisions

**No VM parity gap**, same reasoning as milestone 39's
`string_replace`: `string_url_decode` is a plain native function call,
resolved through the same shared `stdlib::module_bindings` table the
tree-walking interpreter and bytecode VM already both use. Confirmed
by the whole workspace building clean immediately after adding the
three stdlib-table edits, with zero `crates/vm` changes.

**The fix lives in two places on purpose**: the native itself
(general-purpose, correct for any percent-encoded string, not just a
query value), and `router.an`'s own `find_param` (the actual call site
that was producing unreadable output). A native alone, never wired
into the one place that actually needed it, wouldn't have fixed
anything a real caller could see.

## Explicitly out of scope

- **Percent-*encoding*** (the inverse operation, for building a URL
  rather than reading one). Not what this milestone's retrospective
  found missing — `aint-website` only ever needed to read incoming
  query values, never construct outgoing ones.
- **Decoding a query parameter's *key***, or the path portion of a
  URL. Scoped to the value, matching the actual found gap.
- **A general string ↔ byte/char-code primitive** that would let an
  AINT program write its own decoder from scratch. `string_url_decode`
  is a native specifically because building this from existing
  primitives isn't reasonably possible without one (see `ROADMAP.md`'s
  Phase 3 framing) — a general byte-level primitive is real, separate,
  larger work, not attempted here.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
