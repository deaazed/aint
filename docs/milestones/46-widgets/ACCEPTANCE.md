# Milestone 46 — widgets: no HTML or CSS assumptions — acceptance

## Scope

See `SPEC.md`. A full replace of milestone 44/45's HTML-tag-shaped role
vocabulary and CSS-shaped `style()` renderer with a closed, abstract
widget vocabulary and one renderer, `render(page: Node) -> String`,
compiling a widget tree straight to a complete HTML document with a
generated, deduplicated stylesheet — no `class` prop, no CSS string, no
tag-name fallback anywhere in the authoring surface.

## Acceptance criteria

- [x] **`Type::Node` prop values widened from `String`-only to
      `String | Int | Float | Bool`** (`crates/typechecker/src/
      checker.rs`'s `NodeLiteral` arm), and `Value::Node`'s
      representation widened to match (`Vec<(String, PropValue)>`,
      `crates/runtime/src/value.rs`) — a new `PropValue` enum (`Str`/
      `Num`/`Bool`). Every construction site updated: the interpreter's
      `NodeLiteral` eval arm, `test_runner`'s restricted mock-value
      evaluator, and `HttpModel`'s `RawNode`/`RawPropValue` wire shape
      (a real model can answer a style prop with a JSON number/boolean
      directly, not a quoted string). This is what makes `padding: 24`
      and `wrap: true` possible instead of `padding: "24px"`.
- [x] **A new `crates/runtime/src/widgets.rs`** replaces the entire old
      render/style engine (`render_node_html`, `attr_string`,
      `is_plausible_tag_name`, `is_safe_attr_name`, `safe_url`,
      `DANGEROUS_TAGS`, `VOID_ELEMENTS`, `style_node`,
      `declarations_text`, `style_stylesheet` — all removed from
      `stdlib.rs`). `NativeFunction::Style` removed; `NativeFunction::
      Render` is the only widget-related native, rebound to the new
      compiler.
- [x] **The widget vocabulary**: `Page` (document root — `title`/
      `description` props, an optional `Theme` child, exactly one body
      child), `Theme` (`Light`/`Dark` children, each a palette of
      `accent`/`background`/`surface`/`text`/`border`/`on_accent` color
      props), `Column`/`Row` (`gap`/`align`/`wrap`), `Box` (`padding`/
      `background`/`corner_radius`/`border`/`width`/`height`/`grow`),
      `Spacer`, `Responsive` (`Narrow`/`Wide` children), `Text`/
      `Heading` (`size`/`weight`/`color`, `level` for `Heading`),
      `Button`/`Link` (`to` for `Link`, plus `Box`'s style props on
      both), `Image` (`src`/`alt`/`width`/`height`), `Input` (`kind`/
      `id`, kind validated against a fixed allowlist), `Label`
      (`target`), `List`. Anything outside this list is a render error,
      at *any* depth in the tree — no fallback path exists at all,
      unlike milestone 44/45's tag-name fallback.
- [x] **Structured, colocated styling, no CSS strings.** Every size
      prop is a unit-less number, resolved to `px` only inside the
      compiler. A color-valued prop resolves against the active
      theme's palette field names (`"accent"` → `var(--accent)`, so one
      generated class stays correct across a light/dark switch) or,
      failing that, a validated literal CSS color
      (`is_plausible_css_color`) — an unrecognized value is dropped,
      not spliced into the generated stylesheet, closing the same class
      of injection risk milestone 44/45's `safe_url` covered for URLs,
      now for CSS declaration values sourced from an unvalidated
      `infer -> Node` response.
- [x] **Deduplicated class generation.** `Stylesheet::class_for` keys
      on the widget's exact resolved declaration (and hover-rule) text;
      two widgets anywhere in the tree with identical resolved styling
      share one generated class. Verified directly (`identical_style_
      props_on_different_widgets_share_one_generated_class`): two
      differently-labeled `Box` widgets with the same `padding` compile
      to one `.w1{padding:8px;}` rule, referenced by both `<div>`s.
- [x] **Automatic light/dark, no manual toggle in v1** (an explicit
      scope cut, not an oversight — see `SPEC.md`). `Theme`'s `Light`/
      `Dark` children generate a `:root` default and a `@media
      (prefers-color-scheme: dark)` override, once per `render()` call,
      not per widget.
- [x] **`Responsive { Narrow { ... } Wide { ... } }`** renders both
      branches and shows exactly one via the project's one fixed
      720px breakpoint (`w-narrow-only`/`w-wide-only`, two fixed
      globally-shared utility classes, not per-instance-hashed).
- [x] **Built-in interaction affordances, not authored.** `Button`
      gets a fixed default look (padding/background/color/radius) if
      unstyled, plus a fixed `:hover{opacity:.88}`; `Link` is a plain
      accent-colored hyperlink with a hover underline by default, and
      picks up `Box`'s style props (and `Button`'s hover treatment)
      the moment an author sets `padding`/`background` — a navigating
      call-to-action is a `Link` with those props set, not a separate
      widget or mode.
- [x] **`render`'s own structural errors**: the root must be `Page`; a
      `Page` needs exactly one body widget (plus an optional `Theme`);
      an `Input`'s `kind` must be one of the fixed allowlist; a
      `Theme` child must be `Light`/`Dark`; a `Theme` color must parse
      as a real color. All verified directly with `run_expect_err`.
- [x] `examples/ui_nodes.an` rewritten onto the new vocabulary (3 test
      blocks: AI/hand-authored composition, generated-class dedup,
      unit-less numeric props) — `aint check`/`run`/`test` all pass
      through the real CLI binary, output traced by hand against the
      compiler's actual class-assignment order and confirmed to match
      exactly.
- [x] `docs/SPECIFICATION.md` §4.10 and its §9 stdlib table entry
      rewritten for the new vocabulary/renderer; `ROADMAP.md` gained a
      new milestone 46 entry (44/45's entries stay as historical
      record, unedited); `aint migrate`'s AI system prompt
      (`crates/cli/src/migrate_cmd.rs`) rewritten to describe the new
      widget vocabulary instead of the old HTML-tag-shaped one, so
      `--ai` proposals against real code stop suggesting syntax that no
      longer exists.
- [x] `cargo test --workspace`: 0 failures. `cargo clippy --workspace
      --all-targets`: clean. `cargo fmt --check`: clean.

## Explicitly out of scope (unchanged from `SPEC.md`)

- A manual light/dark toggle (automatic `prefers-color-scheme`
  adaptivity ships; the checkbox/`:has()` toggle the old site used
  doesn't).
- Custom hover/focus/animation styling beyond the one fixed built-in
  treatment.
- More than one responsive breakpoint.
- Event handling / client-side interactivity.
- A non-web renderer — still nothing else for AINT to target; still a
  new native over the same `Value::Node`, not a reason to touch this
  design, exactly as milestone 44 argued for its own second renderer.
- **Migrating `aint-website` onto this.** A real, separate follow-up,
  not bundled here — the same sequencing milestone 44 used for its own
  site migration. `layout.an` still calls the old `render`/`style`
  API shape today and will fail to type-check against this milestone's
  stdlib until that follow-up happens.

## A design wrinkle worth naming

`SPEC.md`'s own prose described `Page`'s `theme` and `Responsive`'s
`narrow`/`wide` as if they were props (`Page { ... theme: my_theme
body: Node }`). They can't be: a `Theme`/a widget is a `Node`, and
`Node` values are never valid prop values (props are `String | Int |
Float | Bool`, deliberately — that's the exact rule this milestone
tightened, not loosened). The implementation resolves this the only
way consistent with everything else in the language: `Theme` and
`Narrow`/`Wide` are ordinary **children** (`Page { Theme { ... } body
}`, `Responsive { Narrow { ... } Wide { ... } }`), not props. The
SPEC's prose was loose shorthand written before implementation
surfaced the inconsistency; the confirmed nav-bar preview shown before
implementation began didn't include `Page`/`Theme`/`Responsive`, so
nothing already agreed on changed — noted here for the record rather
than silently reconciled.

## Security posture, compared to milestone 44/45

The old design needed an active blocklist because its authoring
surface was still open-ended (any role could become a real tag via the
fallback, any prop name could become a real attribute): event-handler
prop names blocked, `javascript:` URLs blocked, a short list of
dangerous tags (`script`/`iframe`/`object`/`embed`/`base`) blocked at
every depth. This milestone's vocabulary is closed instead of
blocklisted — there is no path from an unrecognized role to any
markup at all, at any depth (`an_unrecognized_role_nested_inside_an_
otherwise_valid_tree_is_still_a_render_error`), so most of that
blocklist has nothing left to guard. What's still real and still
checked: `safe_url` (a `javascript:`-scheme `to`/`src` is dropped, same
as before) and `is_plausible_css_color` (an unrecognized color value
is dropped rather than spliced into the generated stylesheet — the
CSS-side counterpart to the old URL check, needed because a color prop
can still come from an unvalidated `infer -> Node` response).
