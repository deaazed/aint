# Milestone 48 — design tokens: spacing scale + closed font choice — acceptance

## Scope

See `SPEC.md`. A named spacing scale accepted anywhere a raw pixel
number already is, plus a closed `Page.font` choice replacing the one
hardcoded font-family literal.

## Acceptance criteria

- [x] `SPACING_SCALE` (`crates/runtime/src/widgets.rs`) — six named
      values (`xs`/`sm`/`md`/`lg`/`xl`/`xxl`, 4–48px). `resolve_scale`
      accepts either a `PropValue::Num` or a `PropValue::Str` matching
      a scale name; `px()` — the one shared helper every pixel-valued
      style prop (`padding`, `gap`, `corner_radius`, `Box.width`/
      `height`, `Text`/`Heading`/`Code.size`) already goes through —
      widened to call it instead of reading `Num` only. An unrecognized
      string is dropped, not an error, matching every other malformed
      style value's existing degrade-silently posture. Verified
      directly: `a_named_spacing_scale_resolves_alongside_raw_numbers`
      (a scale name and the equivalent raw number produce byte-identical
      output and dedupe to the same generated class),
      `an_unrecognized_scale_name_is_dropped_not_an_error`.
- [x] **Deliberately not touched**: `num_prop`-only call sites that
      aren't CSS pixel values (`Heading.level`, `Image.width`/`height`)
      — `px()` is scale-aware, `num_prop()` stays exactly what it was.
- [x] `Page` gained a `font` prop (`"sans"`/`"serif"`/`"mono"`,
      default `"sans"`) resolving to one of `FONT_STACKS`' three
      hardcoded system-font stacks — validated at render time the same
      way `Input.kind` already is against `INPUT_KINDS`: unrecognized
      is a render error, not a silent fallback. `Stylesheet::finish`
      widened to take the resolved stack as a parameter instead of
      hardcoding one literal. Verified directly:
      `page_font_selects_one_of_three_closed_stacks`,
      `an_unrecognized_page_font_is_a_render_error`.
- [x] Default behavior unchanged: an unset `font` resolves to the exact
      same `-apple-system,...` stack `Stylesheet::finish` always
      hardcoded, byte-for-byte — confirmed by the pre-existing
      `a_page_compiles_to_one_complete_html_document` exact-match test
      passing with zero changes needed to its expected string.
- [x] `docs/SPECIFICATION.md` updated: the widget/prop description now
      lists the spacing-scale names, all ten theme tokens, and `Page`'s
      `font` prop.
- [x] `cargo test --workspace`: 0 failures. `cargo clippy --workspace
      --all-targets`: clean. `cargo fmt --check`: clean.

## Explicitly out of scope (unchanged from `SPEC.md`)

- Arbitrary font strings, `@font-face`, or external font URLs —
  milestone 51, depends on static asset serving (50).
- A per-widget `font` override — one choice per document, on `Page`.
- A separate radius-specific scale — `SPACING_SCALE` is shared across
  every pixel-valued prop, including `corner_radius`.
