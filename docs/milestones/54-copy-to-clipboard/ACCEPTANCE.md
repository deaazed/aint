# Milestone 54 — copy-to-clipboard on `Code` — acceptance

## Scope

See `SPEC.md`. `Code { copyable: true }` — the compiler's first-ever
generated JavaScript, deliberately the smallest possible real case of
one.

## Acceptance criteria

- [x] `render_code` (replacing the previous direct `wrap_tag("pre",
      ...)` call for `Code`) wraps the `<pre>` in `<div class="w-code-
      wrap">...<button type="button" class="w-code-copy" aria-
      label="Copy code">Copy</button></div>` only when `copyable` is
      set — an unset/`false` `copyable` produces exactly the same
      output `Code` always has. Verified directly:
      `a_copyable_code_block_gets_a_copy_button_wired_to_its_own_pre`,
      `a_plain_code_block_with_no_copyable_prop_gets_no_button_or_script`.
- [x] `Stylesheet` gained `used_copy_button` (the same shape `used_tabs`/
      `used_theme_toggle` already are) gating a fixed CSS block in
      `finish()` — positioning and a small border/background button,
      no author-configurable style.
- [x] **`COPY_BUTTON_SCRIPT`**: one fixed `const`, never built with
      `format!`/interpolation from anything author- or model-
      controlled — there's no variable content in it to validate, a
      different (simpler) safety story than every other injection
      surface this roadmap has had to guard (`resolve_color`,
      `safe_asset_path`, `is_plausible_font_url`). Spliced into the
      final document right before `</body>` — not into the `<style>`
      block, since it isn't CSS — read from `sheet.used_copy_button`
      the same way `render` already reads `used_theme_toggle`, once
      the tree walk completes. Verified as a golden string, not just
      "a script tag exists somewhere": exact byte-for-byte match,
      confirmed emitted exactly once with two separate copyable code
      blocks on the same page, confirmed positioned immediately before
      `</body></html>`. Verified directly:
      `the_copy_script_is_a_fixed_golden_string_spliced_once_before_body_close`.
- [x] The script reads `.textContent` off the button's adjacent `<pre>`
      — the DOM's own decoded text, not the escaped HTML source
      `render` produced it from, so the copied text matches exactly
      what's visibly displayed.
- [x] Output verified by direct rendering and inspection before the
      formal tests were written, same discipline as every prior
      milestone in this roadmap.
- [x] `docs/SPECIFICATION.md` updated.
- [x] `cargo test --workspace`: 0 failures (217 → 220 tests in
      `aint-runtime`). `cargo clippy --workspace --all-targets`:
      clean. `cargo fmt --check`: clean.

## The "first bundle" is complete

This closes out every item in `FEEDBACK.md`'s own recommended first
bundle (Part 3): theme/design tokens (47-49), Accordion/Tabs/
`ThemeToggle` (50-51), static assets/favicon/fonts (52-53), and now
copy-to-clipboard (54) — the smallest real proof of "one small fixed
generated JS asset, referenced not authored" the roadmap itself asked
to see shipped before milestone 61's real interactivity core. That
core is explicitly flagged, in the same roadmap, as deserving its own
dedicated focused session rather than being rushed alongside anything
else — the natural next checkpoint, not a continuation of this one.

## Renumbering note

`FEEDBACK.md` proposed this as one third of its own milestone 60.
Shipped here as 54, the next free slot, per this repo's strictly-
sequential-by-completion-order convention.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Any other interactive behavior — milestone 61's own subject, not
  this one's.
- A visible "copied" toast/animation beyond the button's own text
  change.
- Feature-detecting `navigator.clipboard`.
