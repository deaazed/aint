# Milestone 21 — Memory model — acceptance

## Scope

See `SPEC.md`. A decision milestone, same shape as milestone 00: does
`Rc<RefCell<_>>` (in use since milestone 04) still hold up after
twelve milestones of AI-specific value types, or has something since
introduced a reference-cycle risk it can't handle? Decision: it still
holds, for a specific, checkable, structural reason — not by default
or by not having looked.

## Acceptance criteria

- [x] Every `Value` variant as of milestone 20 (thirteen of them) was
      checked for whether it carries an `Environment` reference — none
      do. `Function` carries `body: Block` (an AST fragment), not a
      captured scope, tying the guarantee directly to milestone 04's
      "no real closures, every call frame parents to globals"
      decision.
- [x] `Environment::parent` was confirmed to only ever point upward —
      nothing in the codebase holds a reference from a parent back
      down to a child.
- [x] The one remaining theoretical path to a cycle — a self-containing
      `List`/`Option` — was checked against AINT's no-mutation,
      no-reassignment guarantee and confirmed unconstructible
      (`let x = [x]` isn't expressible; nothing can mutate a list in
      place afterward to insert itself).
- [x] Decision recorded in `SPEC.md`: reference counting stays, no GC,
      no ownership-model rewrite, no arena allocator — none is
      motivated by anything that actually exists in the codebase.
- [x] The three specific, named conditions that would force revisiting
      this (real closures capturing an environment, in-place mutation
      of container values, measured allocation pressure from
      milestone 26) are stated explicitly, so the decision is
      re-openable against a concrete trigger rather than closed
      forever by assumption.
- [x] The invariant is echoed at the code site that would break first
      if violated: `crates/runtime/src/environment.rs`'s doc comment
      on `Environment` states the acyclic argument directly, pointing
      at `SPEC.md`, so a future contributor adding closures finds the
      constraint before violating it.
- [x] No behavioral code change — `cargo build`, `cargo test
      --workspace` (306 tests, unchanged from milestone 20), `cargo
      fmt --check`, and `cargo clippy --workspace --all-targets -- -D
      warnings` all confirm nothing about how AINT programs run was
      touched.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — building any GC/arena/
ownership alternative (unmotivated), measuring current allocation
performance (milestone 26's job), and optimizing `Value::clone()`'s
cost on `Environment::get` (a possible future target, not evaluated
here).

## Outcome

Satisfied by `docs/milestones/21-memory-model/SPEC.md` (the decision
and its argument) and a doc-comment addition to
`crates/runtime/src/environment.rs` (`Environment`'s acyclic invariant,
stated at the type itself). No other files changed; 306 tests total
across the workspace, unchanged from milestone 20, confirming this
milestone altered no runtime behavior.
