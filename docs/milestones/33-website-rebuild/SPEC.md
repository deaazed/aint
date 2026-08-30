# Milestone 33 — Rebuild the language's own website

## Scope

The same validation pattern milestones 25/26 used: prove a feature
against a real, shipped program, not just unit tests. `examples/
website/site.an` — the language's own docs site, built earlier in this
phase before milestones 29–31 existed — was a single 635-line file
whose `handle_request` was 7 levels of nested `if`/`else` for 7 routes.
That was direct, current evidence of the exact pain milestones 29
(modularity), 30 (closures), and 31 (a router built on both) exist to
fix. This milestone rebuilds it on top of all three, live.

## What this milestone actually builds

**One file becomes nine.** `examples/website/`:

- `layout.an` — `head`/`nav`/`page_footer`/`doc_page`/`shell_block` and
  every `style_*` function. Imported by exactly one file (`main.an`) —
  see "Design decisions" for why every other page file can't import it
  too.
- `home.an`, `docs_index.an`, `install.an`, `quickstart.an`,
  `guide.an`, `stdlib_page.an`, `reference.an` — one file per route,
  each exposing a `content()` function (or `content(shell_block)` where
  a page needs shell-command blocks — see below) that returns its
  page's inner HTML, nothing about `<head>`/nav/footer.
- `main.an` — imports all eight of the above, plus
  `examples/router/router.an` (milestone 31) as `router`, assembles
  each page (`layout_head`/`layout_nav`/`content()`/`layout_page_footer`,
  or `layout_doc_page` for the docs-shaped ones), and registers all
  seven routes in one flat table via `router_route` — no nested
  `if`/`else` anywhere in this program anymore.

**Content pages needing `shell_block` (`install.an`, `quickstart.an`,
`guide.an`) take it as a closure parameter**, not a second import of
`layout.an`:

```an
fn content(shell_block: fn(String) -> String) -> String { ... }
```

`main.an` passes `layout_shell_block` in at the call site
(`install_content(layout_shell_block)`). A real, small demonstration of
what milestone 30 was actually for: sharing behavior across files
without needing a second import path to the same declaration.

**`guide.an` and `reference.an`'s own copy now documents modularity,
closures, and the router** — the previous "known gaps" list on the
reference page said "no cross-file `import`"; that's no longer true,
so it now says what *is* true instead: no diamond imports, closures
don't reach `--vm` yet, no generics/structs/traits.

## Design decisions

**No diamond imports means `layout.an` has exactly one importer.**
Milestone 29 restricted cross-file imports to "each file importable
from exactly one place in the program" — a real, load-bearing
constraint here, not a hypothetical: seven page files all naturally
*want* `layout.an`'s `head`/`nav`/`shell_block`. The resolution is
structural: only `main.an` imports `layout.an`; every page file gets
what it needs either called by `main.an` after the fact (`layout_head`,
composed outside the page file) or passed in as a closure
(`shell_block`) rather than imported directly.

**Each page file keeps its own small `join_lines` copy.** The same
reasoning — `layout.an`'s copy can't be imported a second time. Six
lines of real duplication per file, in exchange for each page file
being independently `aint check`-able and readable without chasing an
import graph. An honest, small cost, not hidden.

**Routes are registered as parallel `List<String>`/`List<fn(...) ->
String>` literals in `main.an`, not spread across the page files.**
Keeps the one place anyone would look for "what routes exist" actually
being one place — the alternative (each page file registering itself
somehow) would need a mechanism this milestone doesn't have.

## Explicitly out of scope

- **Diamond-import support**, or any workaround beyond the closure/
  duplication approach above — that's milestone 29's own stated gap,
  not re-litigated here.
- **New content or new routes** beyond what `site.an` already had —
  this is a structural rebuild, not a content rewrite (beyond the
  guide/reference pages' own copy catching up to what's actually true
  now).
- **Any further stdlib or core-language change.** Everything here
  composes what milestones 29–31 already built.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
