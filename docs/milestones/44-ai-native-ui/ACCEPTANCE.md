# Milestone 44 — a typed, AI-native way to build UI — acceptance

## Scope

See `SPEC.md`. `Type::Node`/`Value::Node`, `Identifier { ... }` node-
literal syntax, `infer`/`tool` declarations allowed to return `Node`
(validated and rendered exactly like any other type), and
`render_html` as the first renderer.

## Acceptance criteria

- [x] `Type::Node` (`crates/ast/src/ty.rs`) — a new base type, accepted
      trivially by `validate_type`, flowing through `List<T>`/function
      types/`infer`/`tool` return types with no special-casing.
- [x] `ExprKind::NodeLiteral { role, props, children }`
      (`crates/ast/src/expr.rs`) — parsed by
      `Parser::parse_node_literal`, hooked into `parse_primary`'s
      `Identifier` arm. No new tokens: reuses `{`, `}`, `:`, already
      meaningful elsewhere in the grammar.
- [x] **The dangling-brace conflict with `if` is real and fixed** —
      verified by breaking a real, shipped program
      (`examples/customer_support/server.an`) first, not just by
      inspection. `Parser::suppress_node_literal` suppresses node-
      literal parsing while parsing an `if` condition directly
      (`suppressing_node_literals`) and re-enables it inside any
      explicitly-delimited sub-expression — `(...)`, `[...]`, a call's
      argument list (`allowing_node_literals`). Regression-tested at
      three layers: `crates/parser` (an `if` condition ending in a bare
      identifier, both statement and expression form, plus a node
      literal explicitly parenthesized inside a suppressed condition),
      `crates/fmt` (round-trips `if is_valid { ... }` correctly), and
      the full pre-existing suite passing unchanged — several tests
      already exercised a bare-identifier `if` condition and would have
      caught this on their own if the fix had been wrong.
- [x] Typechecker (`crates/typechecker/src/checker.rs`): a node
      literal's props must be `String`; children must be `Node`,
      `String`, or `List<Node>`; the literal itself types as `Node`.
- [x] Interpreter (`crates/runtime/src/interpreter.rs`): a node literal
      evaluates to `Value::Node`; a `List<Node>` child splices in as
      multiple children.
- [x] `infer`/`tool` can declare `-> Node` (and `-> List<Node>`, type-
      checked, but see the `HttpModel` gap below) — no grammar/checker
      change needed, since `return_type: Type` was never restricted to
      a closed list.
- [x] `render_html(node: Node) -> String` (`import ui`,
      `crates/runtime/src/stdlib.rs`) — a fixed role→markup table
      (`Heading`/`Paragraph`/`Group`/`Button`/`Link`/`List`/`Image`/
      `Text`), an unrecognized role degrading to `<div
      data-role="...">` rather than erroring, every text/attribute
      value HTML-escaped.
- [x] **A hand-authored tree and an `infer`-returned `Node` compose in
      the same tree** — the actual "AI is the differentiator" claim,
      verified two ways: a Rust-level interpreter test
      (`infer_returning_a_node_composes_with_a_hand_authored_tree`,
      `MockModel`) and the shipped example
      (`examples/ui_nodes.an`'s first `test` block, real `aint test`).
- [x] `HttpModel` (`crates/runtime/src/http_model.rs`) supports a real
      `infer -> Node` call: `expected_shape`/`parse_response_text`
      prompt for and parse a JSON object
      (`role`/`props`/`children`, recursively via a small `RawNode`/
      `RawChild` deserializer), malformed JSON producing the same
      `RuntimeError::ModelError` shape every other type's parse failure
      does. Verified against a hand-rolled mock HTTP server (same
      pattern as every other `HttpModel` test), not just `MockModel`.
- [x] `aint test`'s `mock` accepts a node literal built from other
      accepted mock values (`crates/runtime/src/test_runner.rs`'s
      `eval_mock_value`) — needed for `examples/ui_nodes.an`'s own test
      block to mock an `infer -> Node` declaration; caught by actually
      running the example, not assumed to work.
- [x] `crates/loader` (cross-file `import`'s renaming pass) and
      `crates/fmt` (pretty-printing) both handle `NodeLiteral` —
      `role` is left unrenamed/unquoted either way (an open-vocabulary
      tag, not a declared name); `fmt` never prints a comma between
      items, since the parser doesn't consume one.
- [x] `crates/ir`/`crates/vm`: a node literal (or anything reaching a
      `Type::Node` value) fails lowering with `LowerError::
      UnsupportedNode` — a documented parity gap, the same shape as
      `UnsupportedLambda`/`UnsupportedIfExpr`, not a silent
      miscompilation. Verified through the real binary:
      `examples/ui_nodes.an` under `aint run --vm` fails clearly before
      the VM compiler ever runs.
- [x] `examples/ui_nodes.an` (new) — a hand-authored `Group`/`Heading`/
      `Button` tree, an `infer tagline(...) -> Node` composed into the
      same tree via `await`, `render_html`, and a second test proving
      an unrecognized role renders gracefully. Passes `aint check`/
      `run`/`test` through the real built binary; `--vm` fails with the
      documented `UnsupportedNode` gap, not a panic or wrong output.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **A live `infer -> Node` call against a real model (Mistral) was
  attempted, not just mocked** — `HttpModel`'s `Type::Node` path was
  exercised for real, not only through the hand-rolled mock HTTP server
  test. Every attempt returned `429 Too Many Requests` from Mistral
  itself, which is positive evidence in one respect (the request,
  auth, and connection all reached the real endpoint correctly — a
  request-construction bug would show as `400`/`401` instead) but
  doesn't independently confirm a real completion parses correctly
  end-to-end the way milestone 42's real self-upgrade test or
  `http_model.rs`'s other live-server tests did. The mock-HTTP-server
  test (`parses_a_node_answer`) exercises the identical code path —
  real TCP/HTTP parsing, a real `serde_json` round-trip — against a
  local server instead of Mistral, which is the load-bearing
  verification for this milestone; a live-model completion is a
  follow-up to re-attempt once quota resets, not blocking.
- **`List<Node>` isn't supported by `HttpModel`** — a pre-existing gap
  (no `Type::List(_)` case exists there for any element type) this
  milestone doesn't close. Type-checks and works under `MockModel`/
  `aint test`; a real model call with that return type gets the same
  "does not support responses of type" error `Distribution<T>` and
  tool-calling already produce.
- **No second renderer.** `render_html` is the only one, since
  `http_serve` is the only real output surface AINT has. Nothing about
  `Value::Node`/the literal syntax encodes HTML, so this is additive
  work later, not a redesign.
- **The role vocabulary `render_html` understands is fixed**, not
  user-extensible. An unrecognized role still renders (gracefully), so
  this is a real but non-blocking limitation.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `Node` is an ordinary AINT type — constructible by a new
`Identifier { ... }` literal, or returned by an `infer`/`tool`
declaration and validated the same way every other `infer` result
already is — so a hand-authored UI tree and an AI-generated one compose
in the same value with a compile-time type guarantee and the language's
existing budget/permissions/effects governance, without any
node-literal-specific AI machinery. A real, live-tested grammar
conflict with `if` was found and fixed via the project's own
dogfooding discipline (breaking a real shipped program before it broke
anything else), not caught by inspection. `render_html` is the first
of what can be many renderers, since the tree itself carries no HTML
assumptions.
