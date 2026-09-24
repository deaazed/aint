# Milestone 47 — Theme v2: semantic & status colors

## Scope

`aint-website` (milestone 46's first real dogfooding target) hit a real,
named gap directly: `Palette`'s six fields (`accent`/`background`/
`surface`/`text`/`border`/`on_accent`) have no slot for secondary/muted
text, so `layout.an` was forced to make `border` do double duty as both
the hairline-divider color *and* the only available muted-text color —
documented as a compromise in that file's own comments, not an oversight.
There's also no status-color vocabulary at all (success/warning/danger),
which blocks any future Badge/alert-shaped widget (Phase C of
`FEEDBACK.md`) from having anything principled to draw on.

This milestone: widen `Palette` from 6 fields to 10 — `muted` (real
secondary text) plus `success`/`warning`/`danger`. Nothing else. No new
widgets, no new props on existing widgets beyond what `THEME_TOKENS`
already exposes to every color-valued prop for free. Additive only:
every existing 6-field theme reference keeps meaning exactly what it did.

## Design

`Palette` (`crates/runtime/src/widgets.rs`) gains four fields:
`muted`, `success`, `warning`, `danger`. `THEME_TOKENS`, `light_default`/
`dark_default`, and `Palette::set` extend in lockstep — the same pattern
already used for the existing six fields.

**Refactor alongside the extension, not deferred**: `Theme::css()` was a
single `format!` call with 12 positional placeholders (6 fields × 2
palettes) — already at the edge of hand-maintainable, and this milestone
would push it to 20. Replaced with `Palette::as_pairs()` (a fixed-size
array of `(field name, value)`) and `Theme::css()` building each
palette's `:root` declarations by iterating it. This is a reduction in
code size and mistake-surface for the existing six fields, not scope
creep for the new four — the same "smallest implementation" principle
applied to a spot that was already straining before this milestone
touched it.

**Default values, chosen to hold WCAG-AA-shaped contrast against both
palettes' `background`/`surface` without a formal checker yet** (a real
contrast lint is milestone 65, Phase F — not duplicated here):

- `muted` — a genuine third tone between `text` and `border`: readable as
  small text, visibly dimmer than primary text, without being the same
  color as a hairline divider. `border` itself is *not* touched — it
  keeps its current values and its divider-only job; `aint-website`'s own
  fix of retuning `border` to double as muted text was a real, working
  patch made necessary by this exact gap, and this milestone is what lets
  a future update to that site retire the double-duty rather than
  standardize on it.
- `success`/`warning`/`danger` — conventional green/amber/red, one shade
  per palette (light/dark), picked for legibility as both a background
  fill and a text color against `background`/`surface`.

## Explicitly out of scope

- **Any new widget** (Badge, alerts) that would consume these — Phase C,
  milestone 53, a later, separate milestone.
- **A contrast-ratio checker** — milestone 65, Phase F. This milestone's
  defaults are chosen carefully but not mechanically verified against a
  formula yet.
- **Changing `border`'s existing values or role.** Still divider-only at
  the language level; a project choosing to keep using it for muted text
  anyway isn't broken by this milestone, just no longer forced into it.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
