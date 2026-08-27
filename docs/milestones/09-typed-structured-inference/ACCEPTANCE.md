# Milestone 09 — Typed structured inference — acceptance

## Scope

See `SPEC.md`. `enum` declarations — AINT's first user-defined type —
plus runtime validation of an `infer` call's response against the
declared enum before it becomes a usable value.

## Acceptance criteria

- [x] `enum` lexes as a keyword; `enum Name { Variant1 Variant2 ... }`
      parses with no separators required between variants, same as
      every other AINT statement list.
- [x] `parse_type` accepts any identifier as a (speculative)
      `Type::Enum` reference instead of erroring immediately; the type
      checker rejects one that doesn't name a declared enum
      (`TypeError::UnknownType`), checked recursively through
      `List`/`Option`/`Task`/`Inference` and on every param and return
      type.
- [x] `enum Name { }` (no variants) is a positioned type error
      (`TypeError::EmptyEnum`).
- [x] `EnumName_Variant` (e.g. `Sentiment_Positive`) resolves as a
      plain identifier of type `Enum(Name)` — no new expression kind;
      the type checker and interpreter both derive these bindings from
      the `enum` declaration itself.
- [x] `enum` declarations are hoisted before `fn`/`infer` signatures
      are hoisted, so a signature earlier in the file can reference an
      enum declared later — same forward-reference support `fn`/`infer`
      already have with each other.
- [x] Two enum values compare with `==`/`!=` like any other type
      (already generic in `check_binary`, no changes needed there);
      comparing values of two *different* enums is a positioned type
      error, same as comparing any two mismatched types.
- [x] `infer` can declare an enum return type; calling and awaiting it
      still produces `Inference<Enum(Name)>` → `Enum(Name)`, reusing
      milestone 08's machinery unchanged.
- [x] The runtime validates an `infer` call's response against its
      declared return type before the caller sees it. For a
      `MockModel` configured with a valid variant, this succeeds. For
      one configured with an unlisted variant name (a simulated
      hallucination) or a value from the *wrong* enum entirely, this
      is a positioned `RuntimeError::SchemaViolation` — not a silent
      `false` from `==`.
- [x] `InferenceRequest` carries the declared return type now (the
      "structured-output request" half of this milestone), even though
      `MockModel` doesn't use it yet — real adapters will (milestone
      16).
- [x] `examples/enums.an` — enum declaration, construction, equality,
      and use as a function parameter and return type, including
      recursion over an enum-typed value — runs end to end through the
      real built binary. `infer` still has no example of its own, for
      the same reason as milestone 08 (unchanged: no AINT-level mock
      configuration yet, no real model backend yet).
- [x] `aint run` on all six existing examples is unaffected, verified
      against the actual built binary.
- [x] `cargo test --workspace` passes with no regressions: 161 tests
      total, up from 143 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — general structs/records, pattern matching/`match`,
`Distribution<T>` and uncertainty, real model backends actually
sending a structured-output request over the wire, and
duplicate/colliding enum or variant names are all deferred with
documented reasoning.

## Outcome

Satisfied by `crates/lexer/src/token.rs` (`enum` keyword),
`crates/ast/src/{stmt,ty}.rs` (`StmtKind::Enum`, `Type::Enum`),
`crates/parser/src/parser.rs` (`parse_enum_statement`, `parse_type`'s
relaxed fallback), `crates/typechecker/src/{checker,error}.rs` (the
`enums` registry, `validate_type`, `TypeError::UnknownType`/
`EmptyEnum`), `crates/runtime/src/{value,error,model,interpreter}.rs`
(`Value::Enum`, `RuntimeError::SchemaViolation`,
`InferenceRequest::return_type`, `Interpreter::validate_inference_result`),
`examples/enums.an`, and `crates/runtime/tests/enums.rs` /
`crates/cli/tests/examples.rs`. 161 tests total across the workspace,
all passing: 3 new parser tests (plus one converted from a parse-time
to a parse-success test, matching the layer move described above), 8
new typechecker tests, 7 new runtime tests (5 interpreter, plus the 2
existing `model.rs` tests updated for the new `InferenceRequest`
field), 1 new runtime integration test for `examples/enums.an`, and 1
new CLI integration test for the same.
