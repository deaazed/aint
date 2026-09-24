# Milestone 51 — `ThemeToggle`: a manual dark-mode override — acceptance

## Scope

See `SPEC.md`. One new widget, `ThemeToggle {}`, reintroducing a
manual light/dark override milestone 46 deliberately dropped —
justified now because it's fully compiler-generated, the same
reframing milestone 50 already used for `Tabs`.

## Acceptance criteria

- [x] `Theme::css()` widened to take a `manual_override: bool` and,
      when set, append `:root:has(#w-theme-toggle:checked){...dark
      declarations...}`, built from the exact same `Palette::as_pairs()`
      declaration-building logic the `@media` block already uses — no
      new formatting code. Verified directly (against the real dark
      palette's actual values, not a placeholder):
      `theme_toggle_appends_a_root_has_checked_dark_override`.
- [x] `Stylesheet` gained `used_theme_toggle` (the same shape
      `used_responsive`/`used_tabs` already are), set by
      `render_theme_toggle` and read once in `render()` before
      `theme.css(...)` is called. When unused, no `:has(` or
      `w-theme-toggle` text appears anywhere in the output at all.
      Verified directly:
      `no_manual_override_block_is_emitted_when_theme_toggle_is_unused`.
- [x] `ThemeToggle {}` takes no props — fixed built-in look and fixed
      `aria-label`, same posture as `Button`'s default/`Code`'s
      formatting. Renders one fixed, adjacent input-then-label pair
      (the adjacency is load-bearing: `Stylesheet::finish`'s
      `:checked+.w-theme-toggle-label` CSS depends on it). Verified
      directly: `theme_toggle_emits_an_adjacent_input_and_label_pair`.
- [x] Output verified by direct rendering and inspection before the
      formal tests were written, same discipline as milestone 50.
- [x] **A real finding, not just a build**: `FEEDBACK.md`'s proposed
      "mobile-nav toggle" (paired with the theme toggle in its own
      milestone 60) needed no new code at all — milestone 50's
      `Accordion`/`AccordionItem` already is a zero-JS disclosure
      toggle, and `Responsive { Narrow { AccordionItem { ... } } Wide {
      ...full nav... } }` already composes a working mobile-nav toggle
      from two widgets that shipped for unrelated reasons. Recorded in
      `SPEC.md` rather than silently building a redundant second
      mechanism.
- [x] `docs/SPECIFICATION.md` widget list updated.
- [x] `cargo test --workspace`: 0 failures (210 → 213 tests in
      `aint-runtime`). `cargo clippy --workspace --all-targets`: clean.
      `cargo fmt --check`: clean.

## Renumbering note

`FEEDBACK.md`'s proposed milestone 60 covered three things: a mobile-
nav toggle (turned out to need no new code, see above), a manual
theme toggle (this milestone), and copy-to-clipboard on `Code` (needs
real generated JS, a different technique — deferred to its own
milestone). Shipped here as 51, the next free slot, per this repo's
strictly-sequential-by-completion-order convention.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Bidirectional forcing (forcing light when the system prefers dark) —
  needs JS or a 3-state control.
- Any prop on `ThemeToggle`.
- Persisting the toggle's state across page loads — every fresh load
  starts unchecked again, deferring to system preference.
