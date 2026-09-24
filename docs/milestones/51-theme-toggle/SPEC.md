# Milestone 51 — `ThemeToggle`: a manual dark-mode override

*(Corresponds to half of `FEEDBACK.md`'s proposed milestone 60 — its
"mobile-nav toggle" half turned out, once milestone 50 actually shipped
first, to need no new code at all; see "A finding, not just a build"
below. Its "copy-to-clipboard on `Code`" half needs real generated JS,
a different technique, and is deferred to its own milestone. Renumbered
to 51, the next free slot, per this repo's strictly-sequential-by-
completion-order convention — see milestone 50's own `ACCEPTANCE.md`
for why.)*

## Scope

Milestone 46 dropped a manual light/dark toggle deliberately — "the
in-page manual toggle button the current site has ... is a CSS-specific
pattern with no obvious widget-shaped equivalent; it's dropped for v1
rather than smuggling `:has()` back in under a different name." That
reasoning held while the toggle would've been an author-facing CSS
trick. It no longer applies once the trick is fully compiler-generated,
the same reframing milestone 50 already used to justify reintroducing
the radio-sibling-selector pattern for `Tabs`. This milestone: one new
widget, `ThemeToggle {}`, using `:has()` the same well-supported way
(Chromium, Safari, Firefox all shipped it years ago) this exact
project's own pre-widget-system site already relied on before milestone
46 replaced it.

## Design

**One-directional forcing, stated as a real, known scope cut, not
hidden**: `ThemeToggle` can force the page *into* dark mode when the
system prefers light; it cannot force light when the system prefers
dark. True bidirectional toggling needs either JS or a 3-state control
(system/light/dark), neither of which a single checkbox gives a
zero-JS design. This is the exact behavior this project's own site
shipped with the last time it had a manual toggle at all — not a new
limitation, a preserved one.

**Why this is safe as pure CSS specificity, not document-order
sequencing**: `:root:has(#w-theme-toggle:checked)` has one more
pseudo-class than either the baseline `:root{...}` rule or the
`@media(prefers-color-scheme:dark){:root{...}}` rule (a media query
adds no specificity of its own) — so it wins whenever it matches,
regardless of where in the stylesheet it's written or what the system
preference says, and simply has no effect when unchecked. No
`:not(:has(...))` wrapping needed anywhere, unlike the pre-46 version
of this exact trick.

`ThemeToggle {}` takes no props — a fixed built-in look, the same
"nothing to configure" posture `Button`'s default and `Code`'s
formatting already have. Renders one fixed, adjacent-sibling pair:

```
<input type="checkbox" id="w-theme-toggle" class="w-theme-toggle-input">
<label for="w-theme-toggle" class="w-theme-toggle-label" aria-label="Toggle dark mode"></label>
```

`Stylesheet` gains a `used_theme_toggle` flag (the same shape
`used_responsive`/`used_tabs` already are): if set, `finish()` emits one
fixed CSS block (a pill switch, `::after` sliding circle) plus — this is
the one piece that isn't purely a fixed block — the theme-specific
`:root:has(#w-theme-toggle:checked){...}` override, built from
`Palette::as_pairs()` on the *dark* palette, the exact same declaration-
building logic `Theme::css()` already has for its `@media` block (no
new formatting code, `Theme::css()` takes the flag and appends the
extra block itself).

At most one `ThemeToggle` is meaningful per page (there's exactly one
`:root`) — not structurally enforced, the same way nothing enforces at
most one `Page`-worth of chrome elsewhere; an author placing two just
gets two checkboxes that happen to share an id.

## A finding, not just a build

`FEEDBACK.md` proposed a *separate* mobile-nav-toggle mechanism
alongside the theme toggle, both "via 55's checkbox/radio-plus-sibling-
selector technique" — written before milestone 50 (its own 55) existed
to check that claim against. It didn't need checking against anything
new: `Accordion`/`AccordionItem` (native `<details>`/`<summary>`)
*already is* a zero-JS disclosure toggle, and `Responsive { Narrow {
AccordionItem { title: "Menu" ...nav links } } Wide { ...full nav row
} }` already gives an author a working mobile-nav toggle purely by
composing two widgets that shipped for unrelated reasons. No new widget,
no new `Stylesheet` state, nothing to build here — recorded as a real
finding, the same way this whole roadmap treats anything discovered by
actually building on top of what came before rather than assumed up
front.

## Explicitly out of scope

- **Bidirectional forcing** (force light when system prefers dark) —
  needs JS or a 3-state control; see above.
- **Any prop on `ThemeToggle`** — fixed look and fixed `aria-label`,
  same posture as `Button`'s/`Code`'s.
- **Persisting the toggle's state across page loads** — no cookies, no
  `localStorage`, nothing JS-shaped; every fresh page load starts
  unchecked (deferring to system preference) again, exactly like the
  pre-46 site's own version of this toggle did.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
