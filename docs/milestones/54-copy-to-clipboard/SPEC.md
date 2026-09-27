# Milestone 54 — copy-to-clipboard on `Code`

*(Corresponds to the JS-generating third of `FEEDBACK.md`'s proposed
milestone 60 — its other two thirds, the theme toggle and the mobile-
nav toggle, shipped as milestones 51 and 50's own finding respectively.
Renumbered to 54, the next free slot, per this repo's strictly-
sequential-by-completion-order convention.)*

## Scope

Every other milestone in this roadmap generates HTML and CSS only.
This one generates the compiler's first-ever JavaScript — deliberately
the smallest possible real case, chosen for exactly that reason:
`FEEDBACK.md` frames it as "the smallest real proof of 'one small
fixed generated JS asset, referenced not authored'... ship it before
[the real interactivity core]." One `Code { copyable: true }` prop; one
fixed, non-templated `<script>` block, identical byte-for-byte across
every program that uses it, emitted at most once per page regardless of
how many copyable code blocks it has.

## Design

`Code` gains `copyable` (`Bool`, default `false`). When set,
`render_code` (replacing the previous direct `wrap_tag("pre", ...)`
call) wraps the `<pre>` in a small container with a button:

```
<div class="w-code-wrap">
  <pre class="wN">...</pre>
  <button type="button" class="w-code-copy" aria-label="Copy code">Copy</button>
</div>
```

`Stylesheet` gains `used_copy_button` (the same shape `used_tabs`/
`used_theme_toggle` already are), gating a fixed CSS block in
`finish()` — positioning, a small border/background button matching the
rest of the built-in-look widgets, nothing author-configurable.

**The genuinely new piece**: unlike every prior `used_*` flag, this one
also gates a fixed `<script>` block — the first time anything in
`render`'s output isn't HTML or CSS. It's spliced into the final
document just before `</body>`, not into the `<style>` block `Stylesheet
::finish` owns, since it isn't CSS. `render` reads `sheet.
used_copy_button` the same way it already reads `sheet.
used_theme_toggle`, after the tree walk completes.

**The script itself is one `const` string, never built with
`format!`/interpolation from anything author- or model-controlled** —
the load-bearing security property here isn't an escaping/validation
question the way `resolve_color`/`safe_asset_path`/`is_plausible_font_url`
all are, it's simpler than that: there is nothing to validate, because
there is no variable content in it at all. Golden-string-tested
(asserted verbatim, not just "a script tag exists somewhere") the same
way `Stylesheet`'s fixed CSS blocks already are.

```js
document.querySelectorAll('.w-code-copy').forEach(function(b){
  b.addEventListener('click', function(){
    var text = b.previousElementSibling.textContent;
    navigator.clipboard.writeText(text);
    var original = b.textContent;
    b.textContent = 'Copied';
    setTimeout(function(){ b.textContent = original; }, 1500);
  });
});
```

Reads `.textContent` off the adjacent `<pre>` — the DOM's own decoded
text, not the escaped HTML source `render` produced it from, so the
copied text is exactly what's visibly displayed, entities and all
resolved the way a browser already does that for free.

## Explicitly out of scope

- **Any other interactive behavior.** This is deliberately the smallest
  possible real JS case, not a general "add scripting" milestone —
  milestone 61 (still ahead, still its own dedicated session per this
  roadmap's own sequencing) is where a real interaction core belongs.
- **A visible "copied" toast/animation beyond the button's own text
  change** — the plainest possible feedback, nothing to configure.
- **Feature-detecting `navigator.clipboard`** — every browser this
  project's own toolchain targets already has it; a graceful fallback
  for one that doesn't isn't worth the added script size for a
  convenience feature.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
