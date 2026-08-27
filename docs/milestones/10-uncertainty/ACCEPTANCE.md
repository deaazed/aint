# Milestone 10 — Uncertainty — acceptance

## Scope

See `SPEC.md`, including the required explicit written decision on
what "probability" means in AINT (structural validity only — every
probability in `[0.0, 1.0]`, summing to `1.0` within `1e-6` — no claim
about calibration, token probability, or empirical frequency).

## Acceptance criteria

- [x] `Distribution<T>` parses like `List<T>`/`Option<T>`, reusing
      `<`/`>`. The type checker rejects a non-enum `T` (e.g.
      `Distribution<Int>`) as a positioned type error.
- [x] `distribution_probability`, `distribution_argmax`,
      `distribution_entropy`, `distribution_sample`, and
      `distribution_require_confidence` are free functions gated behind
      `import distribution` — not dotted methods, a deliberate,
      documented departure from `LANGUAGE_DESIGN.md`'s illustrative
      syntax, consistent with every other "no dotted access" decision
      in the codebase.
- [x] All five are polymorphic over `Distribution<T>`'s `T`, using the
      same `Binding::PolymorphicListFunction`-style special-casing
      `collections_length` established in milestone 06 —
      `collections_length` itself untouched.
- [x] `argmax`/`sample` return the enum type; `entropy`/`probability`
      return `Float`; `require_confidence` returns `Option<T>`, using
      argument-count and argument-type checks matching every other
      function call in the language.
- [x] `Option<T>` — type-only since milestone 05 — is now constructible
      (`Value::Option`), and inspectable from AINT source via
      `option_is_some`/`option_unwrap` (`import option`).
      `option_unwrap` on `None` is a positioned runtime error, not a
      panic.
- [x] The runtime validates a `Distribution<T>` `infer` response before
      it becomes usable: right enum, every listed variant real,
      every probability in range, and the whole distribution summing
      to `1.0`. All three failure modes (unlisted variant, wrong enum,
      probabilities not summing to 1.0) are covered by tests using a
      `MockModel` configured to simulate a malformed response — no live
      model required, continuing the testability promise from
      milestone 08 into uncertainty specifically.
- [x] `distribution_sample` is genuinely random (`rand` crate, new
      dependency scoped to `aint-runtime`) but tested deterministically
      via a degenerate distribution (one variant at probability `1.0`)
      — no flaky statistical assertions.
- [x] `argmax`'s tie-breaking (first-encountered entry wins on an exact
      tie) is deterministic and documented, not an accident of
      iteration order.
- [x] No new `examples/*.an`: every `distribution_*` function requires
      a `Distribution<T>` value to operate on, and the only way to
      produce one is an `infer` call answered by a real model — the
      same gap milestones 08 and 09 already documented, inherited here
      with no partial workaround (unlike `enum`, which is usable
      standalone). Verified instead by runtime tests calling
      `distribution_*`/`option_*` against `MockModel`-backed `infer`
      results directly.
- [x] `aint run` on all six existing examples is unaffected.
- [x] `cargo test --workspace` passes with no regressions: 178 tests
      total, up from 161 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Notable finding during implementation

While extending `Interpreter::validate_inference_result` for
`Distribution<T>`, noticed the existing (milestone 09) `Enum` branch
used `.expect(...)` to look up a declared enum's variant list,
assuming the type checker always ran first and validated the name.
That assumption doesn't hold for runtime-only tests (`aint-runtime`
depends only on `aint-ast`, never `aint-typechecker` — by design, see
`docs/ARCHITECTURE.md`), so a hand-built program calling `infer` with
an undeclared enum name would panic instead of erroring. Fixed in
place (`Interpreter::known_variants`) as a small, directly-motivated
hardening while touching this function for `Distribution<T>` anyway,
rather than opening a separate milestone-09 patch for it.

## Explicitly out of scope

See `SPEC.md` — `Distribution<Bool>` or any non-enum distribution,
pattern matching / `match`, seeded/reproducible `sample()`, and real
model backends actually producing calibrated distributions are all
deferred with documented reasoning.

## Outcome

Satisfied by `crates/ast/src/ty.rs` (`Type::Distribution`),
`crates/parser/src/parser.rs` (`Distribution<T>` syntax),
`crates/typechecker/src/checker.rs` (`DistributionOp`/`OptionOp`,
`PolymorphicDistributionFunction`/`PolymorphicOptionFunction`,
`check_distribution_call`/`check_option_call`, the `Distribution<T>`
enum-only restriction in `validate_type`),
`crates/runtime/src/{value,stdlib,interpreter}.rs`
(`Value::Distribution`/`Value::Option`, the seven new
`NativeFunction` variants and their implementations,
`validate_distribution_result`/`known_variants`), and
`crates/runtime/Cargo.toml` (`rand`). 178 tests total across the
workspace, all passing: 1 new parser test, 9 new typechecker tests, 10
new runtime tests (13 counting the pre-existing tests' unaffected
pass), no CLI-level test (no example exists this milestone, per
above).
