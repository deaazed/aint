# Milestone 46 — a widget layer, no HTML or CSS assumptions

## Scope

Milestone 44 built `Node`/`render_html` on the principle "abstract roles,
not HTML tags" — but the role vocabulary it shipped with (`Group`,
`Heading`, `Button`, `Section`, a `class` prop, a generic fallback that
uses *any* unrecognized role as a literal tag name) was HTML wearing a
thin disguise: `Group { class: "wrap-wide navrow" ... }` is a `<div
class="...">` with extra steps. Milestone 45's follow-on work made this
worse in one dimension (a `style()` renderer with `Rule`/`Media` roles
that are CSS selectors and `@media` queries verbatim) while fixing it in
another (real Node literals instead of `Raw`-wrapped HTML strings).
Migrating a real site onto it surfaced the actual problem directly: the
tree stopped being HTML strings, but it never stopped being *shaped like*
HTML and CSS.

The brief for this milestone, stated directly: make AINT's UI story
unique, easy to write, and easy to understand — the way Flutter's widget
model is a real abstraction over "a screen," not a thin wrapper around
each target platform's native view types — while still producing a real,
working web page. Two decisions, both made directly before this SPEC was
written:

1. **Full replace, not a layer on top.** `Node`/`render`/`style` and
   every HTML-tag-shaped role (`Group`, `Section`, `Nav`, the generic
   tag-name fallback, `Raw`, `class`) are removed as the authoring
   surface. There is no HTML/CSS escape hatch this time — the whole
   point is that the widget vocabulary has to be sufficient on its own,
   the same way a Flutter developer never drops down to platform view
   code for a layout `Container` can already express.
2. **The tree representation doesn't change** — `Value::Node { role,
   props, children }` stays exactly what milestone 44 built, for the
   same reason milestone 44's own SPEC already gave: "a second renderer
   ... is a new native function over the same `Value::Node`, not a
   redesign — the entire point of keeping the tree itself free of HTML
   assumptions." What changes is the *vocabulary* of roles or the
   *renderer* — never the tree shape, never the parser/grammar (still
   milestone 44's `Identifier { item* }` literal, unmodified) — with one
   exception, described below, to the *type* of a prop value.

## Widget vocabulary v1

Every widget below is an ordinary `Node` literal — same grammar as
today, same composition story (a function returning `Node`, a child
that's a `List<Node>` spliced in, an `infer`-returning-`Node` composing
with hand-authored widgets in the same tree). Nothing new to parse.

**Layout** — the primitives HTML doesn't give you directly (you assemble
`display:flex` by hand today); these are first-class here, the same way
`Column`/`Row` are Flutter's own primitives, not a coincidence:

- `Column { gap: N align: "start"|"center"|"end"|"stretch" children }`
- `Row { gap: N align: ... wrap: true children }`
- `Box { padding: N background: color corner_radius: N border: color grow: true child }`
  — the one generic single-child container (replaces `Group`/`div`).
- `Spacer {}` — flexible expansion inside a `Row`/`Column`.
- `Responsive { narrow: Node wide: Node }` — swaps its rendered child
  below a fixed breakpoint (one project-wide constant, not an authored
  pixel value — see "Responsive design" below).

**Content**:

- `Text { size: N weight: "regular"|"medium"|"bold" color: color "..." }`
- `Heading { level: 1..6 "..." }` — the one place semantic HTML rank
  still matters (screen readers, SEO); `level` is a UI concept, not an
  HTML one, the same way Flutter's own `Semantics` widget carries
  heading level without the author writing a tag name.
- `Button { "..." }` — a real, non-navigating `<button>`.
- `Link { to: "/path" "..." }` — navigation. `to`, not `href`: `href` is
  an HTML attribute name; `to` is a widget prop that happens to compile
  to one.
- `Image { src: "..." alt: "..." width: N height: N }`
- `Input { kind: "checkbox"|"text" id: "..." }`
- `Label { target: "input-id" "..." }`
- `List { children }` — each child wrapped as a list item; still a real
  semantic list for assistive tech, same reasoning as `Heading`.
- `Page { title: "..." description: "..." theme: my_theme body: Node }`
  — the document root. Exactly one per `render()` call. Owns everything
  that today's `head()` hand-assembles: doctype, `<title>`,
  `<meta description>`, the generated stylesheet, `<body>`. An author
  never manually builds a document shell again.

Nothing here is a tag name, an HTML attribute, or a CSS property name
verbatim, mirroring the same rule milestone 44 used to justify
`Heading`/`Paragraph`/`Button` originally — except this time there's no
fallback path that lets an unrecognized role degrade into a literal tag
name (`is_plausible_tag_name`, `VOID_ELEMENTS`, the whole generic
tag-fallback are removed). An unrecognized role is a render-time error,
not a graceful degrade — the point isn't "handle anything," it's "the
vocabulary is closed and it's the whole surface."

## Styling: structured props, not CSS

No `class` prop, no CSS strings, no selectors. A widget's own props
*are* its style — `Box { padding: 24 background: "surface" }`, not
`Group { class: "card" }` plus a `Rule { selector: ".card" ... }`
somewhere else. This is the same design Flutter, SwiftUI, and Compose
all converged on independently: colocate a widget's appearance with the
widget, because a separate selector-matched stylesheet is exactly the
indirection that makes CSS hard to trace.

**Numbers, not unit strings.** `padding: 24`, not `padding: "24px"` — a
unit suffix on every number is CSS leaking through the back door. All
widget-level sizing (`padding`, `gap`, `corner_radius`, `width`,
`height`, `size`) is a bare number in one consistent logical-pixel unit,
resolved to `px` only inside the compiler, never written by the author.

**This needs one real type-system change.** Milestone 44 restricted
every node prop to `String` (`checker.rs`'s `NodeLiteral` arm:
`if ty != Type::String { ... }`). That's too narrow for `padding: 24` or
`wrap: true`. Widened to accept `String | Int | Float | Bool` — still no
new AINT-level type, just a wider acceptance check in the same one
place, matched by widening `Value::Node`'s prop representation from
`Vec<(String, String)>` to `Vec<(String, PropValue)>` where `PropValue`
is a small enum (`Str`, `Num(f64)`, `Bool`) in `aint-runtime`. Every
existing prop-reading site in the interpreter/stdlib updates to match on
`PropValue` instead of assuming `String` — a mechanical but real change
across `value.rs` and the new renderer.

**Theme, not repeated hex codes.** `Theme { light: Palette dark: Palette
}`, where `Palette` is a small Node of named colors (`accent`,
`background`, `surface`, `text`, `border`). A color-valued prop
(`background`, `color`, `border`) accepts either a literal color string
or one of the active theme's palette names (`"accent"`, `"surface"`,
...) — resolved against the theme at render time, falling back to
treating the string as a literal CSS color if it doesn't match a palette
field. One `Theme` value threads through `Page { theme: ... }`; nothing
else needs to know about it.

**Automatic light/dark, no manual toggle in v1.** `render()` emits the
light palette as the default custom-property set and the dark palette
under `@media (prefers-color-scheme: dark)` — both generated, neither
hand-written. The in-page manual toggle button the current site has (a
hidden checkbox plus a `:has()` selector trick) is a CSS-specific
pattern with no obvious widget-shaped equivalent; it's dropped for v1
rather than smuggling `:has()` back in under a different name. Automatic
OS-preference adaptivity is the real requirement this satisfies; a
manual override is a follow-up if it turns out to matter once the rest
of this ships.

**Interaction affordances are built in, not authored.** `Link`/`Button`/
`Input` get one small, fixed hover/focus treatment generated by the
compiler unconditionally (a focus outline for accessibility, a subtle
hover tint) — there is no `:hover` concept exposed to an author in v1.
Custom interaction styling is out of scope, tracked below.

## Compilation model

`render(page: Node) -> String` (`import ui`) is the *only* renderer —
`style()` is removed entirely; there is no author-visible CSS output at
any point. Two passes over the widget tree, in one call:

1. **Collect.** Walk the tree; for every widget with resolved style
   props, compute a stable signature (the resolved, theme-substituted
   prop set) and hash it to a short class name. Two widgets with
   identical resolved styling anywhere in the tree share one generated
   class — this is deliberately the same idea CSS-in-JS/atomic-CSS
   systems use internally, kept as an implementation detail no AINT
   program ever sees or writes.
2. **Emit.** Walk again, producing real markup (`Column`/`Row` →
   `display:flex` containers with the right direction, `Box` → `div`
   with the resolved classes, `Heading` → `h1`-`h6`, etc.) referencing
   the class names from pass one, plus the one `<style>` block (the
   deduped stylesheet from pass one) spliced into `<head>` by `Page`.

The output of `render()` is one complete HTML document string, not a
fragment — `doc_page()`-style manual assembly
(`string_concat(head(...), string_concat(nav(), ...))`) goes away with
it; a program builds one `Page { ... }` tree and calls `render` once.

## Responsive design

One fixed, documented breakpoint (720 logical px, matching the
project's existing "narrow" cutoff) — not an arbitrary number an author
picks per call site. `Responsive { narrow: Node wide: Node }` renders
*both* children into the document and uses the two halves of that one
`@media` query to show exactly one — the same mechanism the current
hamburger-menu CSS already uses, now generated rather than hand-written.
Anything needing more than one breakpoint is out of scope for v1.

## Explicitly out of scope

- **A manual light/dark toggle** — see above; automatic
  `prefers-color-scheme` adaptivity ships, the checkbox/`:has()` toggle
  doesn't, yet.
- **Custom hover/focus/animation styling** — the built-in, fixed
  affordance on interactive widgets is all v1 has.
- **More than one responsive breakpoint.**
- **Event handling / client-side interactivity** — same exclusion
  milestone 44 already carried forward; still nothing in AINT executes
  in the browser.
- **A non-web renderer.** Still nothing else for AINT to target, same
  as milestone 44 — but exactly as before, a second renderer is a new
  native over the same `Value::Node`, not a reason to touch this design.
- **Migrating `aint-website` onto this** — a real, separate follow-up
  once this ships and is verified on its own, the same sequencing
  milestone 44 used.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
