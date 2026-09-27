# Milestone 53 — favicon + self-hosted fonts — acceptance

## Scope

See `SPEC.md`. `Page.favicon` and `font: "custom"` + `font_url` — the
two genuinely new pieces from `FEEDBACK.md`'s proposed milestone 51;
its third piece, `Image`-from-local-file, needed no new code at all
(see below).

## Acceptance criteria

- [x] `Page { favicon: "..." }` → one `<link rel="icon" href="...">`
      spliced into `<head>`, `safe_url`-checked exactly like `Link.to`/
      `Image.src` already are — no new safety mechanism. Verified
      directly: `page_favicon_emits_a_link_icon_tag_in_head`,
      `no_favicon_link_is_emitted_when_favicon_is_unset`,
      `a_javascript_scheme_favicon_is_dropped_not_emitted`.
- [x] `resolve_font` widened from a plain `FONT_STACKS` lookup to
      return `(font-family CSS value, Option<@font-face rule>)`,
      reading `props` directly instead of just a `name` — `font:
      "custom"` requires a paired `font_url`, validated by a new
      `is_plausible_font_url` allowlist (letters, digits, `/ _ - . : ?
      = &` only) before it's ever spliced into `url('...')` — the same
      class of problem `resolve_color`/`safe_asset_path` both exist
      for, applied to a genuinely new injection surface. Uses a fixed,
      non-configurable generated family name (`CustomFont`) with the
      ordinary `"sans"` stack appended as a fallback. Verified
      directly:
      `font_custom_generates_a_font_face_rule_and_uses_it_with_a_sans_fallback`.
- [x] `font: "custom"` with no `font_url`, or a `font_url` that fails
      the allowlist (tested with a string shaped to close `url('...')`
      early and inject a second declaration), is a render error — the
      same closed-vocabulary posture `Input.kind`/`Page.font`'s three
      fixed values already have, not a silent degrade. Verified
      directly: `font_custom_with_no_font_url_is_a_render_error`,
      `a_font_url_that_could_break_out_of_the_generated_css_is_a_render_error`.
- [x] Output verified by direct rendering and inspection (the exact
      `@font-face` rule text and its placement ahead of the rest of the
      stylesheet, the `<link rel="icon">` placement in `<head>`) before
      the formal tests were written, same discipline as every prior
      milestone in this roadmap.
- [x] **A finding, not just a build**: `FEEDBACK.md`'s proposed
      "`Image.src` resolves a local path through the asset root" needed
      no code at all, and couldn't be built the way it was framed
      without breaking `render`'s own architecture — `render` is a
      pure, disk-free function with no awareness an HTTP server even
      exists. `Image { src: "/logo.png" }` already emits `<img
      src="/logo.png">` exactly as it always has; `http_serve_assets`
      (milestone 52) serving a real file at that path is what makes it
      resolve, purely by both sides agreeing on the same string.
      Recorded in `SPEC.md` rather than adding redundant resolution
      logic that would never do anything a plain string doesn't
      already do.
- [x] `docs/SPECIFICATION.md` updated.
- [x] `cargo test --workspace`: 0 failures (211 → 217 tests in
      `aint-runtime`). `cargo clippy --workspace --all-targets`:
      clean. `cargo fmt --check`: clean.

## Renumbering note

`FEEDBACK.md` proposed this as its own milestone 51 (all three pieces
together). Shipped here as 53, the next free slot, per this repo's
strictly-sequential-by-completion-order convention.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Multiple custom fonts, or font weights/styles beyond one
  `@font-face` declaration.
- Any fallback mechanism if the font file itself 404s — the CSS
  `font-family` chain already covers this at the browser level.
- Validating that `favicon`/`font_url` resolve to a real file —
  `render` has no filesystem access, by design.
