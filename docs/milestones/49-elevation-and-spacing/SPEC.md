# Milestone 49 — elevation + max-width + per-side padding/border

## Scope

Three additions to `Box`, all found the same way every gap in this
roadmap was — building `aint-website` for real:

1. **`shadow`** — a fixed elevation scale (0-3), not arbitrary CSS.
2. **`max_width`** — the real fix for a documented, still-open problem in
   `aint-website`'s own `layout.an`: `width` compiles to a literal CSS
   `width`, with no way to say "cap at N px, but still shrink below
   that." The site's current workaround (generous breakpoint-aware
   padding instead of a real max-width) is explicitly called out in that
   file's own comments as a substitute, not a fix.
3. **Per-side `padding_top`/`right`/`bottom`/`left`** and
   **`border_top`/`right`/`bottom`/`left`** — `Box` currently has no way
   to pad or border fewer than all four sides.

**Deliberately no `margin` prop** — stated once here, not per-call-site:
`gap` (on `Column`/`Row`) and `padding` (on `Box`) stay the only spacing
primitives. This avoids reintroducing CSS margin-collapse — two adjacent
elements' margins silently merging instead of adding, one of CSS's
best-known footguns and exactly the kind of authoring surprise this
whole widget system exists to not have. Flutter and Compose make the
same call for the same reason: a widget tree with real nesting doesn't
need margin to get consistent spacing, `gap` on the parent already does
that without the collapse behavior.

## Design

All three land in `box_decls()` (`crates/runtime/src/widgets.rs`), the
one function that already resolves every other `Box` style prop.

**`shadow`**: an `Int` 0-3, clamped the same way `Heading.level` already
clamps its own small numeric range (`num_prop(...).round().clamp(...)`)
— not a render-time hard error, since this is a style value like
`padding`, not a closed-vocabulary choice like `Page.font`. `0` (or
unset) emits nothing. `1`-`3` map to one of three fixed, non-themed
`box-shadow` values — elevation conventionally reads the same regardless
of light/dark theme, so these aren't threaded through `Theme` the way
colors are.

```rust
const SHADOWS: &[&str] = &[
    "0 1px 2px rgba(0,0,0,.08)",
    "0 4px 12px rgba(0,0,0,.12)",
    "0 12px 32px rgba(0,0,0,.18)",
];
```

**`max_width`**: goes through `px()` exactly like `width`/`height`
already do (so it accepts a named spacing-scale value too, milestone 48,
for free) — emits `max-width`, not `width`. Nothing else changes; an
author who wants both a hard `width` and a `max_width` ceiling can set
both, same as real CSS.

**Per-side padding/border**: eight new props, each going through the
exact same `px()`/`resolve_color()` path their uniform counterpart
already does. Emitted *after* the uniform `padding`/`border` declaration
in the same generated rule — CSS resolves a same-rule shorthand-then-
longhand collision by declaration order, so `padding: 16
padding_top: 24` correctly ends up with top overridden and the other
three sides still `16px`, with no special-casing needed in `class_for`'s
dedup (it still just keys on the final declaration text, unchanged).

## Explicitly out of scope

- **`margin`** — see above, a permanent exclusion, not deferred.
- **A shadow color/theme integration** — fixed, non-themed values for
  v1; revisit only if a real project surfaces a concrete need.
- **Per-side `corner_radius`** (individually rounded corners) — not
  asked for by `FEEDBACK.md` or surfaced by `aint-website`; add later if
  a real use appears, same bar as everything else in this roadmap.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
