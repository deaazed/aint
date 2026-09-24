# Milestone 49 — elevation + max-width + per-side padding/border — acceptance

## Scope

See `SPEC.md`. Three additions to `Box`: a fixed elevation (`shadow`)
scale, `max_width` (the real fix for a problem `aint-website`'s own
`layout.an` named directly), and per-side padding/border. Deliberately
no `margin`.

## Acceptance criteria

- [x] `shadow` — `Int` 0-3, clamped via `num_prop(...).round().clamp
      (0.0, 3.0)`, the same pattern `Heading.level` already uses for
      its own small numeric range. `0`/unset emits no `box-shadow`
      declaration at all; `1`-`3` map to one of three fixed
      (non-themed) `SHADOWS` values. Verified directly:
      `shadow_maps_a_clamped_level_to_a_fixed_elevation_value` (also
      proves the out-of-range `99` clamps to the same value as `3`),
      `a_zero_shadow_emits_no_box_shadow_declaration_at_all`.
- [x] `max_width` — goes through the same `px()` (and, by extension,
      milestone 48's named spacing scale) `width`/`height` already do;
      emits `max-width`, distinct from `width`. Verified directly:
      `max_width_emits_max_width_not_width`.
- [x] Eight per-side props (`padding_top`/`right`/`bottom`/`left`,
      `border_top`/`right`/`bottom`/`left`), each resolving through the
      exact `px()`/`resolve_color()` path their uniform counterpart
      already uses, emitted *after* the uniform `padding`/`border`
      declaration so a same-rule shorthand/longhand collision resolves
      the way real CSS does (later, more specific declaration wins for
      the side it names). No change needed to `class_for`'s dedup — it
      still just keys on final declaration text. Verified directly:
      `per_side_padding_overrides_only_that_side_of_the_uniform_shorthand`,
      `per_side_border_resolves_color_the_same_way_the_uniform_border_does`.
- [x] **No `margin` prop added** — a permanent exclusion stated in
      `SPEC.md`, not an oversight: `gap`/`padding` stay the only
      spacing primitives, avoiding CSS margin-collapse.
- [x] `docs/SPECIFICATION.md`'s `Box` description updated with all new
      props.
- [x] `cargo test --workspace`: 0 failures (no existing exact-match
      test touched `Box`'s declaration set in a way this milestone's
      purely-additive props could break). `cargo clippy --workspace
      --all-targets`: clean. `cargo fmt --check`: clean.

## Explicitly out of scope (unchanged from `SPEC.md`)

- `margin` — permanent exclusion.
- Theme-integrated/colored shadows.
- Per-side `corner_radius`.

## Phase A complete

Milestones 47, 48, and 49 — the "first bundle" `FEEDBACK.md`'s own
sequencing recommended doing together, since everything downstream in
that roadmap (Badge, Table, Grid, Code syntax highlighting, the Phase F
contrast checker) reads from these tokens — are done. Verified as a
whole: `cargo test --workspace` green, `clippy`/`fmt` clean, across all
three milestones' commits. `aint-website`'s own theme/spacing/shadow
adoption is intentionally left to that project's own session, per its
standing "don't commit without being asked" rule.
