# Milestone 39 — String stdlib: replace

## Scope

`string` has offered `string_length`/`string_to_upper`/`string_to_lower`/
`string_trim`/`string_contains`/`string_concat`/`string_split` since
milestones 06 and 31 — nothing that replaces one substring with
another. Building `aint-website`'s one page that puts real user input
back into HTML (`/try`'s echoed message) needed exactly that, for
`escape_html`, and had to hand-roll it from `string_split` plus a
recursive join instead. See `ROADMAP.md`'s Phase 3 framing.

## What this milestone actually builds

**`string_replace(s: String, target: String, replacement: String) ->
String`** — every occurrence of `target` replaced with `replacement`:

```an
import string
print(string_replace("a-b-c", "-", "_"))   // "a_b_c"
```

An empty `target` leaves `s` unchanged, matching the precedent
`string_split` already set for an empty separator — not Rust's own
`str::replace("", ...)`, which would insert `replacement` between
every character. `target`/`replacement` can differ in length freely
(the result can shrink or grow relative to `s`).

A native function, not something buildable from `string_split` alone
without the recursive-join workaround `aint-website` needed — same
motivation as `string_split` itself (milestone 31's own SPEC.md: "the
one primitive AINT was missing to write its own ... parsing in source
... rather than needing a dedicated native per parsing need").

## Design decisions

**No VM parity gap.** `string_replace` is a plain native function call
— `aint-vm`'s bytecode compiler already resolves every native call
through the exact same `stdlib::module_bindings` table `aint-runtime`
uses (`crates/vm/src/compiler.rs`), and its VM loop calls
`aint_runtime::stdlib::call` directly rather than duplicating
execution logic the way `eval_binary`/`eval_unary` are duplicated (see
milestone 38's `SPEC.md` for why *those* are different). Adding one
native function and its table entry is enough for both the tree-
walking interpreter and the bytecode VM to support it identically,
with zero VM-specific code.

**Only `string_replace` — not a larger string-stdlib expansion.**
`string_starts_with`/`string_ends_with`/others were named as candidates
in the original Phase 3 framing but weren't things `aint-website`
actually needed; adding them here without a real call site would be
guessing at a surface, not fixing a found gap. Real, separate,
additive if a future program needs them.

## Explicitly out of scope

- **A general string-formatting/interpolation feature.** Not what this
  gap was about — see `ROADMAP.md`'s Phase 3 framing for that
  (`if`/`else`-as-expression, already shipped in milestone 37, is the
  closest thing attempted so far to reducing string-building
  boilerplate).
- **Regex or pattern-based replacement.** `target` is a literal
  substring, matching `string_split`'s own literal-separator behavior.
- **`string_starts_with`/`string_ends_with`/other string natives.**
  See "Design decisions" above.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
