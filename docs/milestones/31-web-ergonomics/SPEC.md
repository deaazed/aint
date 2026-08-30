# Milestone 31 — Web framework ergonomics

## Scope

The concrete, measured pain this milestone fixes: `examples/website/
site.an`'s own `handle_request` is 7 levels of nested `if`/`else` for
7 routes, one indent level per route, closing braces cascading to
column ~30. `examples/customer_support/server.an`'s is the same shape
at 5 routes. Neither is a one-off — it's what `import`-less, closure-
less AINT forced every HTTP program to do. Milestones 29 (modularity)
and 30 (closures) exist to fix exactly this, and this milestone is
where they're actually put to use, plus the one real stdlib gap they
don't cover on their own: AINT could not split a string on anything,
at all, which made query-string or form-body parsing simply
impossible to write, in source, no matter how the rest of the language
grew.

## What this milestone actually builds

**One new stdlib primitive: `string_split(s: String, sep: String) ->
List<String>`**, added to the `string` module. Not a
`parse_query_string` native or a `parse_form_body` native — this
milestone deliberately adds the one general primitive underneath both
(and CSV-shaped data, and anything else string-splitting turns out to
be useful for), and lets AINT source compose the rest, the same
reasoning that kept `collections_length` a single polymorphic function
rather than one native per list operation. A separator that doesn't
occur yields a one-element list (the whole string) — same convention
`str::split` in the standard libraries this mirrors already uses.

**A route table, written entirely in AINT, not as new Rust stdlib
surface.** `examples/router/router.an` is a small, real, *importable*
library:

```an
fn route(paths: List<String>, handlers: List<fn(String, String, String) -> String>,
          method: String, full_path: String, body: String,
          not_found: fn(String, String, String) -> String) -> String {
    let clean_path = string_split(full_path, "?")[0]
    return dispatch(paths, handlers, 0, clean_path, method, full_path, body, not_found)
}
```

`dispatch` recurses down the parallel `paths`/`handlers` lists,
comparing the request path (with any `?query` stripped before
matching) against each registered path in turn, and calls the
matching handler — or `not_found` if nothing matches. This is possible
*only* because of milestone 30: `handlers` is a `List<fn(String,
String, String) -> String>`, a list of closures, called by index.
Milestone 29 is what makes it *reusable*: any program imports it with
`import "./router.an" as router` and gets `router_route` — a flat
route table registration, not a hand-nested pyramid, regardless of how
many routes it has.

**Query-string parsing, in AINT, on top of `string_split` alone.**
`router.an` also provides `query_param(full_path: String, key: String)
-> String`, returning the value for a key in `full_path`'s query
string, or `""` if absent or there's no query string at all. `""` as
the not-found sentinel, not `Option<String>` — AINT still has no
`Option<T>` construction syntax (a stated, unrelated known gap), so
this follows the exact pattern `examples/customer_support/server.an`'s
own `field()` helper already established for the same reason.

## Design decisions

**No changes to `http_serve`, `content_type_for`, or any existing
runtime HTTP code.** Everything here is a stdlib primitive
(`string_split`) plus an ordinary AINT library built from primitives
that already existed after milestones 29–30. Deliberately: the
pain was never in `http_serve` itself, it was in what a program had to
write *around* it.

**The router matches exact paths only — no path parameters
(`/users/:id`), no wildcards.** `paths[i] == clean_path` is a plain
string comparison. Path parameters would need either a real pattern
matcher (regex-shaped, which nothing in AINT's stdlib provides) or a
hand-rolled segment-splitting scheme; both are real, separate design
work, not attempted here. A route needing a dynamic segment still
reads it out of `full_path` or `body` itself, the same way it always
could.

**`router.an` lives under `examples/`, not the stdlib.** It's proof
that the *language* is now expressive enough to build this without any
new Rust code — an actual library someone could import and use, not a
demonstration of a Rust-side feature. Consistent with `aint-loader`
having no privacy/module-registry concept yet (milestone 29): "import
it" means "point at the file," not "add a stdlib dependency."

## Explicitly out of scope

- **Path parameters/wildcards in the router.** See above.
- **Form-body parsing as a named feature.** `string_split` makes it
  writable the same way query-string parsing is (split on `&`, split
  each pair on `=`) — not written out as a separate library function
  here since nothing in this milestone's own verification needs it,
  but nothing blocks it either.
- **Concurrency in `http_serve`.** Still one connection at a time — a
  separate, larger runtime change untouched by this milestone.
- **Real multi-line string literals, or any other lexer/grammar
  change.** Not scoped here.
- **Any change to `aint-vm`/`aint-ir`.** `string_split` is a plain
  synchronous native — already generic-dispatch-compatible with the VM
  exactly the way every other stdlib native is (see milestone 22's
  design), verified directly rather than assumed.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
