# Milestone 22 — Bytecode VM — acceptance

## Scope

See `SPEC.md`. A new `aint-vm` crate: `AirProgram -> CompiledProgram`
(compiler) and `CompiledProgram -> program output` (VM), covering
AINT's full deterministic core, with `infer`/`tool`/`await`/`async
fn`/`Distribution<T>` explicitly rejected at compile time rather than
half-supported. Wired into the CLI as `aint run --vm`, opt-in.

## Acceptance criteria

- [x] New crate `aint-vm` (`crates/vm`), added to the workspace:
      `bytecode.rs` (`Instruction`, `Chunk`), `compiler.rs`
      (`compile(&AirProgram) -> Result<CompiledProgram, CompileError>`),
      `vm.rs` (`Vm<W: Write>`, `.run(&CompiledProgram)`).
- [x] Locals resolve to stack-relative indices at compile time; no
      `HashMap`/`Environment`-style lookup anywhere in the VM's
      dispatch loop.
- [x] Every call site (`fn`, stdlib native, `print`) resolves to a
      chunk index or `NativeFunction` at compile time, reusing
      `aint_runtime::stdlib::module_bindings`/`call` (promoted from
      `pub(crate)` to `pub` for exactly this) rather than
      re-deriving stdlib semantics.
- [x] Forward references and mutual recursion between top-level
      functions work, via the same two-pass hoist-then-compile
      structure the type checker itself uses — verified directly
      (`a_function_can_forward_reference_a_later_function`).
- [x] Top-level `let` becomes a real global slot, resolved
      incrementally in source order (not hoisted) — matching, not
      exceeding, what the type checker already accepts.
- [x] `if`/`else` block scoping matches the tree-walker exactly: a
      `let` inside an `if` doesn't leak out, verified directly
      (`let_inside_if_does_not_leak_out`), via compile-time-tracked
      local scopes emitting the right number of `Pop`s on block exit.
- [x] `examples/fibonacci.an`, `examples/showcase.an`, and
      `examples/enums.an` all produce byte-identical output through
      the VM as through the tree-walking `Interpreter` — verified via
      integration tests reusing the exact same expected-output
      strings as `aint-runtime`'s own tests for the same files, and
      via the real `aint` binary (`aint run --vm`) for `showcase.an`
      specifically.
- [x] Deep AINT-level recursion runs on an *ordinary* thread stack,
      not a dedicated big-stack one: `showcase.an`'s Collatz(27) (111
      levels) and a dedicated 5,000-level recursion test both pass
      without `aint-runtime`'s 64 MiB thread workaround — the actual,
      demonstrated payoff of the VM's heap-allocated frame stack.
- [x] `infer`/`tool` calls, `await`, `async fn`, and
      `Distribution<T>`/`distribution_probability` operations are all
      rejected with a specific `CompileError::Unsupported` naming
      what's missing and why — verified for all five, plus that
      merely *declaring* (not calling) `infer`/`tool` compiles fine.
- [x] `test`/`budget` blocks compile as no-ops, matching their
      already-inert behavior under plain `aint run` — verified
      directly.
- [x] `aint run --vm <path>` wired into the CLI: same
      parse-and-type-check gate as `aint run`, then
      `lower -> compile -> Vm::run`, each failure reported clearly and
      distinctly. Verified through the real binary for a passing
      program (`showcase.an`) and two failing ones (`testing.an`'s
      `infer`, `async.an`'s `async fn`), confirming clean rejection
      with no partial output.
- [x] Plain `aint run` (no `--vm`) is completely unaffected — the
      existing tree-walker test suite passes unmodified.
- [x] `cargo test --workspace` passes with no regressions: 332 tests
      total, up from 306 before this milestone (26 new: 20 unit tests
      in `aint-vm`, 3 whole-program integration tests in `aint-vm`
      reusing existing example files, 3 new CLI integration tests).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are
      clean across the whole workspace, including the new crate.

## Known, honestly-stated gap

AIR carries no source positions (true since milestone 18; this
milestone is just the first to need them and not have them). Every
`RuntimeError` the VM produces uses a fixed placeholder span instead
of a real one — the error *kind* and *message* match the tree-walker
exactly, but `aint run --vm`'s error output loses the line:column
information `aint run` gives. Stated in `SPEC.md`, not hidden; not
fixed here, since nothing in this milestone's acceptance bar needed
real positions to prove the VM executes AINT correctly.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — sandboxing and
portability (properties of deployment, not of having a bytecode
format), real source-position tracking through AIR, any change to the
tree-walking `Interpreter`, and LLVM/native compilation
(`ROADMAP.md` says so explicitly).

## Outcome

Satisfied by the new `crates/vm` crate (`bytecode.rs`, `compiler.rs`,
`vm.rs`, `lib.rs`), `crates/runtime/src/stdlib.rs`'s visibility change
(`pub(crate)` -> `pub` for `call`/`module_bindings`), and
`crates/cli/src/main.rs`'s new `run_vm` function and `--vm` flag on
`Command::Run`. 332 tests total across the workspace, all passing: 26
new, covering compiler scope (arithmetic, recursion, forward
references, all five rejected AI operations, inert `test`/`budget`),
VM execution semantics (arithmetic, control flow, block scoping,
lists, globals, deep recursion, runtime error parity with the
tree-walker), and the real `aint run --vm` CLI path end to end.
