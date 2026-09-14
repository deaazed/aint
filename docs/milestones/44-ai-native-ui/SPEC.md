# Milestone 44 — a typed, AI-native way to build UI

## Scope

`ROADMAP.md`'s "Looking ahead" note named the real pain: AINT programs
that render a page do it by hand-nesting `join_lines`/`string_concat`
calls over raw HTML strings — `aint-website`'s own `layout.an` is a
25-item list just for `<head>`. The first framing of a fix (string
interpolation) was rejected directly: AINT isn't only for the web, and
a templating feature that only knows about HTML tags would have baked
that assumption into the language itself.

The scope that replaced it, decided directly across two rounds:

1. **Abstract roles, not HTML tags.** A UI tree is built from open-
   vocabulary role names (`Heading`, `Group`, `Button`, ...) that mean
   nothing to the language itself — the only concrete renderer today
   (`render_html`) maps a fixed set of them to markup and degrades
   gracefully on anything it doesn't recognize, but the tree itself
   never encodes HTML.
2. **New literal syntax, not a data-model.** `Identifier { ... }` is a
   new expression form the parser understands directly, not an enum
   and a handful of stdlib functions — real lexer/parser/typechecker/
   interpreter work, the same size class as closures or if-expressions.
3. **AI has to be the actual differentiator, not a bystander.** The
   first cut of this design was rejected for exactly this reason: it
   was a JSX clone with nothing AI-native about it. The fix isn't a
   special "AI-flavored" node syntax bolted onto the tree — it's
   letting `Node` be an ordinary type an `infer`/`tool` declaration can
   return, validated and governed (budget, permissions, effects)
   exactly like every other `infer` result already is. A hand-authored
   `Group { ... }` and an `await some_infer_call()` returning `Node`
   compose in the same tree because they're the same type — nothing
   about the language's existing AI story had to change to make that
   true. That composition is the actual pitch: a UI section can safely
   delegate to AI generation with a compile-time type guarantee on the
   result (never malformed markup, never raw untyped text an injection
   could hide in) and the same runtime governance every other `infer`
   call already gets — something no JSX-plus-an-LLM stack gives you,
   since there the model's output is just a string re-parsed (or not)
   at runtime.

## Grammar

`Identifier { item* }` in expression position. Previously, an
identifier immediately followed by `{` was always a parse error —
`parse_postfix` only ever continues on `(` or `[` — so this introduces
no ambiguity with anything that parsed before this milestone, on its
own. (It does collide with `if`'s own grammar — see "The dangling-brace
conflict" below.)

Each item inside `{ }` is either:

- `name: expr` — a **prop**. `expr` must type to `String`.
- a bare `expr` — a **child**. `expr` must type to `Node`, `String`
  (a text leaf), or `List<Node>` (spliced in as multiple children —
  the one way to render a dynamically-sized list of children without a
  loop construct, since AINT has none).

No commas between items — same as a block's statements, and for the
same reason (nothing else in AINT separates same-level items with
commas except call arguments and list literals, both of which have
their own bracket, not brace, delimiter).

```an
Button { href: "/docs/quickstart" "Quickstart" }

Group {
    Heading { title }
    Paragraph { tagline }
    Group { links }              // links: List<Node>, spliced in
}
```

## The dangling-brace conflict

`if <expr> { ... }` places a `{` immediately after the condition with
no separator. Once a bare identifier can itself start a node literal,
`if is_valid { ... }` is genuinely ambiguous: does `is_valid {` begin a
node literal, or does the bracket belong to `if`? This is exactly the
conflict Go's grammar has with composite literals in an `if`/`for`/
`switch` header, solved the same way here: the parser suppresses node-
literal parsing while it's parsing an `if` condition directly
(`Parser::suppress_node_literal`, `suppressing_node_literals`), and
re-enables it inside any explicitly-delimited sub-expression — `(...)`,
`[...]`, or a call's argument list — since those have their own
unambiguous closing token. `if (Flag { on: label }) { ... }` still
works; `if a == b { ... }` still means what it always did.

This was caught by the project's own dogfooding discipline, not by
review: the first implementation compiled clean and passed every new
test, then broke `examples/customer_support/server.an` (a real, shipped
program) with "expected expression, found `return`" — `field(user,
"email") == email {` had its trailing `email` misparsed as the start of
a node literal. Fixed, then verified with the full existing suite
(several tests already exercise a bare-identifier `if` condition, e.g.
`examples/website/layout.an`-style code and `crates/fmt`'s own
`formats_an_if_expression_on_one_line`) — all passed once the
suppression logic was corrected to actually gate on the flag it
introduced (an initial oversight: the flag existed but nothing checked
it).

## Type system

`Type::Node` — a new base type, no type parameter (the tree shape is
fixed: a role, string props, `Node`/`String` children). Flows through
every generic type position for free (`List<Node>`, a function
parameter or return type, an `infer`/`tool` return type) with zero
special-casing beyond `validate_type` accepting it trivially (nothing
to look up, unlike `Type::Enum`).

**`infer`/`tool` declarations can return `Node`.** No special grammar
for this — `return_type: Type` was never restricted to a closed list of
base types, so `infer tagline(topic: String) -> Node` just type-checks
today's way. This is the load-bearing design decision: the AI-native
story falls out of `Node` being an ordinary type, not from any
node-literal-specific AI feature.

## Interpreter

`Value::Node { role: String, props: Vec<(String, String)>, children:
Vec<Value> }` — children are `Value::Node` or `Value::String` only, by
construction (enforced at both evaluation sites: the literal's own
`eval_expr` arm, and the mock-value evaluator below).

Evaluating a node literal: props evaluate to `String` values directly;
children evaluate and are matched — a `Value::Node` or `Value::String`
becomes one child, a `Value::List` is flattened into the children list
(the `List<Node>`-splicing case; the type checker already guaranteed
every element is a `Node`).

**`mock` accepts node literals.** `aint test`'s `mock function -> value`
was already restricted to literals and `EnumName_Variant` references,
evaluated by a small standalone evaluator (`test_runner::
eval_mock_value`) with no running interpreter. Extended with a
`NodeLiteral` arm that recurses through the same restricted evaluator
for its props and children — the natural way to mock an `infer`/`tool`
declared `-> Node`. `List<Node>`-splicing isn't accepted here: no list
literal was an accepted mock value before this milestone either, a
pre-existing restriction left as-is.

## The real model adapter (`HttpModel`)

`infer`-returning-`Node` needed to work against a real model, not just
`MockModel`, for the composition claim above to mean anything.
`HttpModel` doesn't do real structured-output/JSON-schema requests —
it prompts in plain text and parses the free-text response per return
type (`expected_shape`/`parse_response_text`), the same simple design
documented in milestone 16's SPEC.md. `Type::Node` fits this pattern
without needing a bigger redesign: the prompt asks for a single JSON
object (`role`/`props`/`children`, recursively), and the response is
parsed with a small `serde`-derived `RawNode`/`RawChild` shape
converted into `Value::Node`. Malformed JSON is a `RuntimeError::
ModelError`, the same error shape every other type's parse failure
already produces.

**`List<Node>` isn't supported over `HttpModel`** — not a new gap this
milestone opens, but an existing one it doesn't close: no `Type::List(_)`
case exists in `expected_shape`/`parse_response_text` for any element
type today (confirmed by reading the file before touching it), so
`infer -> List<Node>` type-checks and works under `MockModel` but hits
the pre-existing "does not support responses of type" error under a
real model. Out of scope here for the same reason `Distribution<T>` and
tool-calling already are for `HttpModel`.

## Renderer

`render_html(node: Node) -> String` (`import ui`) — the one renderer
that exists, since `http_serve` is AINT's only real output surface
today. A fixed table maps a handful of roles to markup (`Heading` →
`<h2>`, `Paragraph` → `<p>`, `Group` → `<div>`, `Button`/`Link` → `<a
class="button">` when an `href` prop is present else `<button>`, `List`
→ `<ul>` wrapping each child in `<li>`, `Image` → a self-closing `<img
src alt>`, `Text` → `<span>`); an unrecognized role — the expected case
for a role an `infer` call invented that isn't in the table — degrades
to `<div data-role="...">` rather than erroring, since the tree is an
open vocabulary by design. Every piece of text content and every prop
value used as an attribute is HTML-escaped; nothing else about the
tree (prop names, which attribute a role maps a prop to) is
attacker/model-controlled the same way, so escaping stops there.

A second renderer for a non-web target is a new native function over
the same `Value::Node`, not a redesign — the entire point of keeping
the tree itself free of HTML assumptions.

## Explicitly out of scope

- **`List<Node>` returned by a real (`HttpModel`) `infer`/`tool` call**
  — see above; works under `MockModel`/`aint test` today.
- **A second renderer** (anything non-HTML). Nothing exists yet for
  AINT to target beyond `http_serve`.
- **User-extensible/registered roles**, styling, event handling,
  pattern-matching over a `Node` tree. The vocabulary `render_html`
  understands is fixed in this milestone; nothing about `Node`/the
  literal syntax prevents a later milestone from adding to it.
- **Migrating `examples/website`** off hand-built HTML strings onto
  this. A real, separate follow-up once this ships, not bundled here.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
