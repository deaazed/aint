# Milestone 01 — Project bootstrap — acceptance

## Scope

Repository and compiler architecture: a Cargo workspace with the crate
boundaries described in `docs/ARCHITECTURE.md`, and a CLI that at least
runs.

## Acceptance criteria

- [x] `cargo build` succeeds across the whole workspace.
- [x] `cargo fmt --check` passes.
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes.
- [x] `aint --version` prints a version.
- [x] `aint run examples/hello.an` fails with a clear, honest message
      (the interpreter doesn't exist yet) rather than pretending to work
      or panicking.
- [x] Crate layout matches `docs/ARCHITECTURE.md`: `ast`, `lexer`,
      `parser`, `typechecker`, `ir`, `runtime`, `cli`, dependency
      direction only pointing "down" the pipeline.

## Explicitly out of scope

Anything from milestone 02 onward — lexing, parsing, interpretation.
This milestone is the skeleton, not the first feature.

## Outcome

Satisfied by the current state of the repository.
