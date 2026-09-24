# Milestone 50 — Accordion + Tabs: zero-JS interactive content

*(Corresponds to `FEEDBACK.md`'s proposed milestone 55 — renumbered to
50 to keep this repo's own milestone directories strictly sequential by
completion order, this project's existing convention since milestone 0.
`FEEDBACK.md`'s own 50-54 — static assets, SEO, Divider/Badge/Icon,
Table — haven't shipped yet and keep their original numbers reserved
for when they do.)*

## Scope

Two new widgets, chosen and sequenced deliberately ahead of any real
client-side interactivity work: they prove "interactive-feeling" content
doesn't require a JS runtime at all, using two well-established zero-JS
HTML/CSS techniques rather than inventing anything new about the
language. Real gap closed: `aint-website`'s docs today stack everything
vertically — no disclosure, no tabs, at all.

1. **`Accordion`** — compiles to native `<details>`/`<summary>`. Real
   HTML disclosure: free keyboard and assistive-tech support, nothing
   to invent.
2. **`Tabs`** — the classic radio-input-plus-sibling-selector CSS
   pattern. Fully compiler-generated and invisible to the author, same
   posture every other widget already has toward its own markup.

## Design

### `Accordion` / `AccordionItem`

```
Accordion { gap: N
    AccordionItem { title: "..." open: true|false  ...children }
    AccordionItem { title: "..." ...children }
}
```

`Accordion` is `Column` in every way that matters — same `flex_decls`
call, so `gap`/`align`/`wrap` all work identically — wrapping one
`<details>` per `AccordionItem` child. No new structural validation:
`AccordionItem` handles its own props exactly like any other widget
reached through the ordinary `render_widget` dispatch (no need for
`Responsive`-style raw-child access, since `title`/`open` are plain
props and the item's own children render normally before `AccordionItem`
builds its own markup around them).

`AccordionItem`: `title` (`String`), `open` (`Bool`, default `false`) →
`<details{ open}><summary>{title}</summary><div>{content}</div></details>`.
Fixed built-in styling (border, rounded corners, summary
padding/cursor/weight) — the same "sensible default, no prop surface for
it" posture `Button`'s unstyled look already has. No author-facing style
props on `AccordionItem` itself in v1; wrap it in a styled `Box` if more
control is ever needed.

### `Tabs` / `Tab`

```
Tabs {
    Tab { label: "One" ...children }
    Tab { label: "Two" ...children }
}
```

The first tab is checked by default; there's no prop to choose a
different default in v1 (a real, stated scope cut, not an oversight).

**The genuinely new piece**: every other widget's generated CSS is
class-based and instance-agnostic (`class_for`'s whole dedup story). A
CSS-only tab switcher needs the opposite — an `#id:checked ~ selector`
relationship unique to *this* `Tabs` instance, since two `Tabs` widgets
on the same page must not cross-wire each other's radio groups.
`Stylesheet` gains a second counter, `next_instance_id` (parallel to,
not reusing, the class-dedup `next_id`), handed out via
`Stylesheet::next_instance()`. `render_tabs` (needs the raw, un-rendered
children up front to extract `label` before rendering each `Tab`'s own
content — the same reason `render_responsive` special-cases before the
generic dispatch prepass) builds, per tab `i` in instance `n`:

- `<input type="radio" name="tabs{n}" id="tabs{n}-{i}" class="w-tab-radio"{checked if i==0}>`
- `<label for="tabs{n}-{i}" class="w-tab-label">{label}</label>`
- `<div id="tabs{n}-{i}-pane" class="w-tab-pane">{content}</div>`

wrapped as `<div class="w-tabs">{radios}<div class="w-tab-bar">
{labels}</div>{panes}</div>` — radios first, then the label bar, then
the panes, all as direct siblings of the outer wrapper (the general
sibling combinator `~` needs this flat sibling relationship; it ignores
whatever sits between the checked radio and its target).

Two CSS layers, mirroring how `Responsive`'s breakpoint rules already
split into a fixed global part and a per-use part:
- **Fixed, emitted once** if any `Tabs` was used (a new `Stylesheet.
  used_tabs` flag, same shape as `used_responsive`): hides radios,
  lays out the tab bar, gives every pane `display:none` by default,
  styles an inactive label in `var(--muted)` (milestone 47's new token,
  its first real consumer).
- **Per-instance, accumulated during the tree walk** (`Stylesheet.
  tab_rules: Vec<String>`, pushed by `register_tab_rule`, emitted in
  `finish()` right after the fixed block): for each tab, `#{id}:checked
  ~ .w-tab-bar label[for="{id}"]{...active style...}` and `#{id}:checked
  ~ #{id}-pane{display:block}`. `id` is always compiler-generated
  (`tabs{n}-{i}`), never derived from any author- or model-supplied
  string, so there's no attribute-selector-injection surface to guard
  here the way `resolve_color`/`safe_url` guard author-controlled values
  elsewhere.

## Explicitly out of scope

- **Choosing a non-first default-open tab.**
- **Any style props on `Accordion`/`Tabs`/`AccordionItem`/`Tab` beyond
  `Accordion`'s inherited `gap`/`align`/`wrap`** — fixed built-in looks,
  same posture as `Button`'s.
- **Nested `Tabs`/`Accordion`** — not disallowed structurally, not
  specifically tested either; revisit if a real project needs it.
- **Nested `Tabs` sharing state, URL-driven tab selection, or any other
  JS-dependent behavior** — this stays zero-JS by construction.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
