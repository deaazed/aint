# Milestone 47 — Theme v2: semantic & status colors — acceptance

## Scope

See `SPEC.md`. `Palette` widened from 6 fields to 10 — `muted` plus
`success`/`warning`/`danger` — surfaced by `aint-website`'s own
dogfooded gap: no slot for secondary/muted text existed, forcing
`border` into double duty as both the hairline-divider color and the
only available muted-text color.

## Acceptance criteria

- [x] `Palette` (`crates/runtime/src/widgets.rs`) gained `muted`,
      `success`, `warning`, `danger`. `THEME_TOKENS`, `light_default`/
      `dark_default`, and `Palette::set` extended in lockstep with the
      existing six fields — same pattern, no special-casing for the new
      ones anywhere in `resolve_color`/`box_decls`/`text_decls`/etc.
- [x] **`Theme::css()` refactored alongside the extension**: a single
      `format!` with 12 positional placeholders (already at the edge of
      hand-maintainable) would have grown to 20 under this milestone.
      Replaced with `Palette::as_pairs()` (a fixed 10-entry `(name,
      value)` array) and `Theme::css()` building each palette's
      declarations by iterating it — a real reduction in code size and
      copy-paste-mistake surface for the *existing* fields too, not new
      scope.
- [x] Default values chosen for legibility against both palettes'
      `background`/`surface` (a formal contrast lint is milestone 65,
      not duplicated here): light `muted:#7c7568` `success:#1a7f4f`
      `warning:#9a6400` `danger:#c02b3c`; dark `muted:#9b968a`
      `success:#3ecf7e` `warning:#facc15` `danger:#f87171`. `border`
      itself untouched — still divider-only at the language level.
- [x] A `Theme { Light { muted: "..." } Dark { ... } }` child can
      override any of the four new fields exactly like the original
      six — no new code path, `parse_theme`/`Palette::set` already
      handle any recognized field name generically. Verified directly:
      `a_theme_child_can_override_muted_and_status_colors`.
- [x] A widget's `color`/`background`/`border` prop resolves `"muted"`/
      `"success"`/`"warning"`/`"danger"` to `var(--...)` through the
      same `resolve_color`/`THEME_TOKENS` path every other token uses.
      Verified directly:
      `a_muted_or_status_color_token_resolves_like_any_other_theme_token`.
- [x] `docs/SPECIFICATION.md`'s `Theme` widget description updated to
      list all ten fields.
- [x] `cargo test --workspace`: 0 failures (the one pre-existing
      exact-match test asserting the full `:root` CSS string,
      `a_page_compiles_to_one_complete_html_document`, updated to the
      new 10-field output). `cargo clippy --workspace --all-targets`:
      clean. `cargo fmt --check`: clean.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Any widget consuming these colors (Badge/alerts) — Phase C, a later
  milestone.
- A contrast-ratio checker — Phase F.
- Retuning or removing `border`'s existing divider role.

## Note for `aint-website`

This milestone adds the *capability*; it doesn't touch
`aint-website`'s own `Theme { Light { ... } Dark { ... } }` call, which
already hardcodes all of its own hex values explicitly and is
unaffected by any default-value change here. Once that project pulls
this release, it can retire its own `border`-as-muted-text workaround
by adding real `muted`/`success`/`warning`/`danger` fields to its
existing `Theme` call — not done as part of this milestone, since that
repo has its own active session and its own standing "don't commit
without being asked" rule.
