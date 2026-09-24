# Milestone 50 — Accordion + Tabs: zero-JS interactive content — acceptance

## Scope

See `SPEC.md`. Two widgets, both zero-JS: `Accordion`/`AccordionItem`
(native `<details>`/`<summary>`) and `Tabs`/`Tab` (the classic
radio-plus-sibling-selector CSS pattern) — proving "interactive-feeling"
content doesn't need the interactivity core (`FEEDBACK.md`'s Phase E)
at all.

## Acceptance criteria

- [x] `Accordion` reuses `flex_decls("column", props)` — the exact
      function `Column` itself calls — so `gap`/`align`/`wrap` all work
      identically, and an `Accordion` with the same resolved
      declarations as a `Column` elsewhere in the tree dedupes to the
      *same* generated class. Verified directly:
      `accordion_reuses_columns_own_flex_layout_and_style_dedup`.
- [x] `AccordionItem { title open ...children }` → `<details{ open}>
      <summary>{title}</summary><div>{content}</div></details>`, fixed
      built-in styling (border/radius/summary padding+cursor+weight, no
      style-prop surface on the item itself — same posture `Button`'s
      unstyled look already has). Verified directly:
      `accordion_item_compiles_to_native_details_and_summary` (both an
      unset and an explicit `open: true` item, on the real generated
      output, not just presence of the word "open" anywhere).
- [x] `Stylesheet` gained a second counter (`next_instance_id`/
      `next_instance()`), parallel to and independent from the
      class-dedup `next_id` — `Tabs`' CSS is instance-specific by
      construction, the opposite of every other widget's deliberately
      shared class. Verified directly:
      `two_tabs_instances_on_one_page_get_distinct_non_colliding_ids`
      (two `Tabs` on one page get `name="tabs1"`/`name="tabs2"`, not a
      shared/colliding radio group).
- [x] `render_tabs` emits radios, then the label bar, then the panes —
      all flat siblings of one wrapper, the exact DOM shape the CSS
      general-sibling-combinator trick needs. First tab checked by
      default; no prop chooses a different one (a stated scope cut).
      `Tab`'s `id` is always compiler-generated
      (`tabs{instance}-{index}`), never derived from an author- or
      model-supplied string, so the attribute-selector rules built from
      it (`label[for="{id}"]`) carry no injection surface the way
      `resolve_color`/`safe_url` guard elsewhere. Verified directly:
      `tabs_wires_radios_labels_and_panes_by_a_shared_generated_id`
      (the exact per-tab `#id:checked~...` rule text, both halves —
      the label-highlight rule and the pane-visibility rule).
- [x] A non-`Tab` child of `Tabs` is a render error, the same closed-
      structure posture `Responsive`'s `Narrow`/`Wide` requirement
      already has. Verified directly:
      `a_non_tab_child_of_tabs_is_a_render_error`.
- [x] `Stylesheet::finish` gained a `used_tabs`-gated fixed CSS block
      (hides radios, lays out the tab bar, gives every pane
      `display:none` by default, colors an inactive label with
      `var(--muted)` — milestone 47's token's first real consumer)
      followed by the accumulated per-instance `tab_rules` — the same
      two-layer shape (fixed-once-if-used, then per-use) `Responsive`'s
      breakpoint CSS already established.
- [x] Output verified by direct rendering and inspection (not just
      `.contains()` assertions) before the formal tests were written —
      confirmed the `<details>`/`<summary>` nesting and the full
      radio/label/pane DOM shape and CSS selector text are exactly
      right, byte for byte.
- [x] `docs/SPECIFICATION.md` widget list updated with both new
      widgets and their props.
- [x] `cargo test --workspace`: 0 failures (205 → 210 tests in
      `aint-runtime`). `cargo clippy --workspace --all-targets`: clean.
      `cargo fmt --check`: clean.

## Renumbering note

`FEEDBACK.md` proposed this as its own milestone 55. Shipped here as
50 — the next free slot — to keep this repo's milestone directories
strictly sequential by completion order, this project's existing
convention since milestone 0. `FEEDBACK.md`'s own 50-54 (static
assets, SEO, Divider/Badge/Icon, Table) haven't shipped yet and keep
their original numbers reserved for when they do.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Choosing a non-first default-open tab.
- Any style props on `Accordion`/`Tabs`/`AccordionItem`/`Tab` beyond
  `Accordion`'s inherited `gap`/`align`/`wrap`.
- Nested `Tabs`/`Accordion`, or any JS-dependent tab behavior (shared
  state, URL-driven selection) — this stays zero-JS by construction.
