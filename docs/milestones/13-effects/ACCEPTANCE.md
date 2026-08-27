# Milestone 13 — Effects — acceptance

## Scope

See `SPEC.md`. An optional `effects [ ... ]` clause on `fn`/`async fn`
declarations, checked so that a function's declared effects bound what
it can call — opt-in, not retroactive, so every pre-existing
unannotated program keeps compiling and behaving identically.

## Acceptance criteria

- [x] `effects [ Effect, Effect, ... ]` parses as an optional clause
      after a `fn`/`async fn`'s return type, before its body. Absent
      entirely, `StmtKind::Fn.effects` is `None` (untracked), not an
      implicit `pure`.
- [x] All five words parse: `pure`, `inference`, `tool`, `network`,
      `filesystem`. `tool` needed its own parsing path since it's
      already a keyword (milestone 11) and never lexes as a plain
      identifier the other four do.
- [x] `effects [pure, tool]` (or any combination including `pure`) is
      rejected — `pure` must be alone.
- [x] A function with a declared, non-empty-incompatible effect set can
      only call: (a) stdlib/native functions and `print` (exempt,
      always compatible — see SPEC.md for why retrofitting stdlib
      purity is out of scope), (b) other declared-effect functions
      whose set is a subset of its own, (c) `infer` (intrinsically
      `{inference}`) or `tool` (intrinsically `{tool}`) declarations,
      if its own set includes that effect.
- [x] Calling an *unannotated* user function from inside a
      declared-effect one is rejected (`TypeError::EffectMismatch`) —
      untracked isn't treated as harmless. Verified directly.
- [x] `effects [pure]` rejects calling `infer` and `tool` declarations
      specifically (the concrete, AI-relevant case the roadmap's
      "Effects" section exists for).
- [x] `effects [inference, tool]` (a function declaring both) can call
      either an `infer` or a `tool` declaration — verified with a
      function that calls both in sequence.
- [x] Every pre-13 test across the whole workspace (lexer through CLI)
      passes completely unmodified — confirmed by running the full
      suite before writing a single new test. Opt-in was not just a
      design intention; it's the observed result.
- [x] `infer`/`tool` declarations do not get `effects [...]` syntax
      extended to them — a deliberate, documented simplification versus
      `LANGUAGE_DESIGN.md`'s illustrative sketch (see SPEC.md).
- [x] No new `examples/*.an` — effects are erased after type checking
      and never read by the interpreter, so a "successful" example
      would look identical to an existing one; see SPEC.md.
- [x] `cargo test --workspace` passes with no regressions: 219 tests
      total, up from 202 before this milestone.
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace.

## Explicitly out of scope

See `SPEC.md` — effect classification for the standard library,
checking `network`/`filesystem` against anything (nothing produces
those effects yet — accepted vocabulary, currently vacuous
constraints), and effect polymorphism/generics.

## Outcome

Satisfied by `crates/lexer/src/token.rs` (`effects` keyword),
`crates/ast/src/{stmt,lib}.rs` (`Effect` enum, `StmtKind::Fn.effects`),
`crates/parser/src/parser.rs` (`parse_effects_clause`,
`parse_effect_word`), and
`crates/typechecker/src/{checker,error}.rs` (`EffectInfo`,
`current_effects`, the `check_call` subset check,
`TypeError::EffectMismatch`). `crates/runtime/src/interpreter.rs`
needed one mechanical update (`StmtKind::Fn`'s new field, ignored —
effects are erased before runtime). 219 tests total across the
workspace, all passing: 6 new parser tests, 11 new typechecker tests,
and every one of the 202 pre-existing tests passing without
modification.
