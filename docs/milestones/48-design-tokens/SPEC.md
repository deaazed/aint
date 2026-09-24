# Milestone 48 — design tokens: spacing scale + closed font choice

## Scope

Two small, independent additions, both closing a gap named directly in
`FEEDBACK.md` (produced from actually building `aint-website`):

1. A **named spacing scale** — `"sm"`/`"md"`/`"lg"`/etc. — accepted
   anywhere a raw pixel number is accepted today (`padding`, `gap`,
   `corner_radius`, `Box.width`/`height`, `Text`/`Heading`/`Code.size`).
   Purely additive: every existing `padding: 24`-shaped program keeps
   meaning exactly what it did.
2. A **closed font choice** — `Page { font: "sans" | "serif" | "mono" }`
   — resolving to one of three hardcoded, safe system-font stacks. Today
   `Stylesheet::finish` hardcodes exactly one font-family string with no
   way to override it at all.

## Design

### Spacing scale

One shared scale, not a separate one per prop — `"sm"` means the same
pixel value whether it's a `gap` or a `corner_radius`. A second,
radius-specific scale would be more conventional (rounder corners read
worse at large radii than large gaps read badly at large spacing) but
adds a second named vocabulary for no capability a single shared one
doesn't already provide — an author who wants a smaller radius than
scale-`"sm"` still has the plain numeric prop available, unchanged.

```rust
const SPACING_SCALE: &[(&str, f64)] = &[
    ("xs", 4.0), ("sm", 8.0), ("md", 16.0),
    ("lg", 24.0), ("xl", 32.0), ("xxl", 48.0),
];
```

`px()` (`crates/runtime/src/widgets.rs`), the one shared helper every
pixel-valued style prop already goes through, is the only function that
changes: it currently reads `PropValue::Num` only (via `num_prop`).
Widened to also accept `PropValue::Str` matching a scale name, resolving
either shape to the same `{n}px` output. An unrecognized string is
dropped, not an error — the same "malformed style value degrades
silently" posture every other style prop already has (documented in
milestone 46's own `SPEC.md`), not a new exception.

**Deliberately not touched**: `num_prop`-only call sites that aren't CSS
pixel values at all — `Heading.level`, `Image.width`/`height` (real HTML
attributes, not style declarations). A named scale for "how tall is this
image" or "which heading level" isn't the same kind of value a spacing
scale describes, and conflating them would make `num_prop`/`px` do two
different jobs depending on caller. `px()` is scale-aware; `num_prop()`
stays exactly what it is today.

### Font choice

`Page` gains a `font` prop (`String`, one of `"sans"`/`"serif"`/
`"mono"`, default `"sans"` — today's existing hardcoded stack, so an
unset `font` changes nothing). **Validated at render time as a closed
enum, the same posture `Input.kind` already has** (`INPUT_KINDS`,
`widgets.rs`) — an unrecognized value is a render error, not a silent
fallback, since this is a real, closed vocabulary choice, not a
degrade-safely style value.

```rust
const FONT_STACKS: &[(&str, &str)] = &[
    ("sans", "-apple-system,BlinkMacSystemFont,'Segoe UI',sans-serif"),
    ("serif", "Georgia,'Times New Roman',serif"),
    ("mono", "'JetBrains Mono','Courier New',monospace"),
];
```

No custom/arbitrary font string and no external font URL — that's
milestone 51 (self-hosted fonts, depends on 50's static asset serving).
This milestone only replaces the single hardcoded literal in
`Stylesheet::finish`'s `body` rule with one of three hardcoded literals,
chosen by the author instead of fixed by the compiler.

## Explicitly out of scope

- **Arbitrary font strings or `@font-face`/external URLs** — milestone
  51, depends on static asset serving (50).
- **A per-widget `font` override** — one choice for the whole document,
  set once on `Page`, matching how `Theme` already works.
- **A separate radius-specific scale.**

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
