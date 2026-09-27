# Milestone 53 — favicon + self-hosted fonts

*(Corresponds to `FEEDBACK.md`'s proposed milestone 51 — renumbered to
53, the next free slot, per this repo's strictly-sequential-by-
completion-order convention.)*

## Scope

`FEEDBACK.md` proposed three things together: `Image`-from-local-file,
a `Page.favicon` prop, and a `"custom"` self-hosted `font` variant. The
first needed no new code at all — see "A finding, not just a build"
below. This milestone covers the two that do: `favicon` and
`font: "custom"`.

## Design

### `favicon`

`Page { favicon: "/favicon.ico" ... }` → one `<link rel="icon"
href="...">` spliced into `<head>`, alongside `<title>`/`<meta
description>`. Goes through `safe_url` exactly like `Link.to`/
`Image.src` already do — no new safety mechanism, the same one applied
to every other URL-shaped prop.

### `font: "custom"`

`FONT_STACKS`' three closed values (`sans`/`serif`/`mono`) get a fourth,
`"custom"`, which needs a second prop to mean anything: `Page { font:
"custom" font_url: "/fonts/brand.woff2" ... }`. Unlike the fixed three,
this generates a real `@font-face` rule referencing an author-supplied
URL — a genuinely new injection surface (`resolve_color`'s validated-
literal-or-drop posture and `safe_asset_path`'s structural-rejection
posture both exist for exactly this class of problem: a string that
ends up spliced into generated CSS or a filesystem path, sourced from
an author or, through `infer -> Node`, a model). `font_url` gets its own
allowlist, `is_plausible_font_url`: letters, digits, and the handful of
characters a real path or URL legitimately needs (`/ _ - . : ? = &`) —
no quote, no paren, no backslash, no whitespace, nothing that could
close the `url('...')` it's spliced into early. `resolve_font` (new,
replacing the plain `FONT_STACKS` lookup `render` used inline) returns
the resolved `font-family` CSS value *and*, only for `"custom"`, the
`@font-face` rule text to prepend to the stylesheet — using a fixed,
compiler-chosen family name (`CustomFont`, not author-configurable, the
same "nothing to configure beyond what matters" posture `ThemeToggle`
already has) with the ordinary `sans` stack appended as a fallback, so
a failed font load still degrades to something readable rather than the
browser's own unstyled default.

`font: "custom"` with no `font_url` is a render error (there's nothing
sensible to fall back to — unlike a malformed *style* value elsewhere,
this is a structural, closed-vocabulary requirement, the same posture
`Input.kind`/`Page.font`'s three fixed values already have).

## A finding, not just a build

`FEEDBACK.md` proposed "`Image.src` resolves a local path through the
asset root from milestone 52" as if `render` needed new awareness of
where assets live. It doesn't, and adding any wouldn't even be
possible without breaking the actual architecture: `render` is a pure,
disk-free function — it has no idea an HTTP server exists, let alone
what directory it declared as its asset root, and shouldn't, since
`Value::Node` (and therefore a `Page` tree) can be produced and
inspected with no server running at all. `Image { src: "/logo.png" }`
already emits `<img src="/logo.png">` exactly as it always has;
`http_serve_assets(port, "public")` serving a real `public/logo.png`
is what makes that path resolve to a real file, entirely by both sides
agreeing on the same string — no new code connects them, because
nothing needs to. The same shape of finding as milestone 51's
mobile-nav toggle: a proposed milestone that turns out to already work
once the pieces that shipped before it are used together, not built
again.

## Explicitly out of scope

- **Multiple custom fonts, font weights/styles beyond one `@font-face`
  declaration** — one family, one file, the smallest useful version.
- **A fallback if the font file itself 404s at request time** — the
  CSS `font-family` fallback chain already covers this at the browser
  level; nothing server-side to add.
- **Validating that `favicon`/`font_url` actually resolve to a real
  file** — `render` has no filesystem access, by design (see above);
  a broken path is a 404 at request time, the author's own to notice,
  same as a broken `Image.src` already is today.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
