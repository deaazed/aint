# Milestone 22 — Bytecode VM

## Scope

`ROADMAP.md`:

> `AST -> AIR -> Bytecode -> AINT VM`, for startup time, execution
> speed, sandboxing, and portability. Still no LLVM.

AIR (milestone 18) has had no consumer since it shipped — nothing
executed it, nothing optimized-then-ran it (19's `optimize` is
AIR-to-AIR, still unconsumed downstream). This milestone gives it its
first real one: a genuine stack-based bytecode compiler and VM, in a
new crate, `aint-vm`.

## What's actually covered

**AINT's full deterministic core**: arithmetic, comparisons, `let`,
`if`/`else`, function calls and recursion (including mutual/forward
references between top-level functions), lists and indexing,
user-declared `enum`s, top-level `assert`, and every stdlib native
gated behind `import` (`math`, `string`, `time`'s synchronous half,
`collections`, `option`). Verified against the exact same example
programs and expected output `aint-runtime`'s own tests use —
`examples/fibonacci.an`, `examples/showcase.an` (trial-division
primality, a 111-step Collatz sequence, both math/string modules, list
indexing), and `examples/enums.an` — proving byte-identical output to
the tree-walking `Interpreter`, not just "it runs."

AINT's own minimalism is what makes this tractable in one milestone
where it wouldn't be for a general-purpose language: no loops (the
only iteration mechanism is recursion), no mutation or reassignment,
no closures. Control flow really is just `if`/`else` and calls: a
compiler doesn't need to solve anything harder than straight-line
bytecode with two flavors of jump.

## What's explicitly not covered, and why

- **`infer`/`tool` calls, `await`, `async fn`, `Distribution<T>`
  operations.** All of them need the VM's dispatch loop to suspend
  mid-execution and resume once a `Model`/tool call answers — real,
  buildable work, but a second, separate engineering effort (an async
  dispatch loop, or a coroutine-style bytecode interpreter), not
  something that falls out of the deterministic-core design here. The
  compiler rejects every one of these with a specific
  `CompileError::Unsupported` naming exactly what's missing, at
  compile time — never a silent wrong answer, a panic, or a vague
  error. `test`/`budget` blocks are the one exception: they're
  inert during `aint run` on the tree-walker too (see milestone
  15/17), so `--vm` matches that by treating them as no-ops rather
  than rejecting them, keeping `--vm`'s behavior identical to plain
  `aint run` wherever the tree-walker's own behavior is itself a
  no-op.
- **Real-world impact of the gap**: every currently-shipped example
  that uses `infer`/`tool`/`async` (`testing.an`, `async.an`) fails
  clearly under `--vm`, verified directly through the built binary.
  `aint run` (no `--vm`) is completely unaffected — the VM is a
  second, opt-in engine, not a replacement.

## Design decisions

**A new crate, `aint-vm`**, not a module inside `aint-runtime` or
`aint-ir` — matching this codebase's one-crate-per-pipeline-stage
shape (`ast`, `lexer`, `parser`, `typechecker`, `ir`, `runtime`).
Depends on `aint-ir` (for `AirProgram`, its input) and `aint-runtime`
(for `Value`, `RuntimeError`, and — see below — the stdlib dispatch
table).

**Locals live directly on the value stack; there is no `Environment`,
no `HashMap` lookup, anywhere in the hot path.** A `let`'s value is
computed and left exactly where it lands on the stack; its "slot" is
just that position, resolved to a plain array index at compile time.
Reading a local is `stack[frame_base + slot]`, an offset into a
`Vec`, not a name-keyed lookup — the actual, concrete form of
`ROADMAP.md`'s "execution speed" claim over the tree-walker's
`Environment::get`.

**Calls resolve to a chunk index or a `NativeFunction` at compile
time, never a name at runtime.** AIR's own `Call{callee: String, ..}`
only ever names a plain identifier (never an arbitrary callee
expression — first-class/higher-order function values are already
outside what AIR represents a call as), so every call site is
statically resolvable: a pre-pass hoists every top-level `fn` to a
chunk index exactly like the type checker hoists their signatures
(supporting forward references and mutual recursion the same way);
`import` statements are walked in source order to build a
name-to-`NativeFunction` table, mirroring `Interpreter`'s own
`StmtKind::Import` handling exactly — same table, not a second one
that could drift (`aint_runtime::stdlib::module_bindings` and
`stdlib::call` were promoted from `pub(crate)` to `pub` specifically
so `aint-vm` reuses them instead of re-deriving stdlib semantics).

**Top-level `let` bindings are real globals, resolved incrementally in
source order — not hoisted, matching the type checker exactly.** A
function body compiled at a given point in a single top-to-bottom pass
only sees global names bound *so far*, the same restriction the type
checker already enforces when it checks each `fn`'s body at its own
declaration point using whatever's been `define`d up to there. This
was a deliberate design choice, not an oversight: hoisting all
top-level `let`s upfront would have been *more* permissive than the
type checker already is, silently accepting programs the tree-walker
(and the type checker) would reject.

**An explicit, heap-allocated frame stack (`Vec<Frame>`), not a
recursive Rust function call per AINT-level call.** This is what
actually delivers `ROADMAP.md`'s "startup time, execution speed"
framing rather than just being "the tree-walker with extra steps": the
tree-walking `Interpreter` needs a dedicated 64 MiB OS thread because
deep AINT recursion (no loops, remember) costs real Rust stack per
level — five-ish mutually recursive `async fn`s per level, once
`async`/`await` entered the picture in milestone 07. The VM's own call
frames live on the heap; `deep_recursion_does_not_need_a_bigger_rust_stack`
(5,000 levels) and the whole `showcase.an` test (Collatz(27), 111
levels) both run on an *ordinary* thread stack, unmodified, in
`aint-vm`'s own test suite — proof, not assertion.

**Binary/unary/index evaluation logic is duplicated from
`Interpreter`, not shared across the crate boundary.** AINT has nine
binary operators and one unary operator, permanently (see
`CONTRIBUTING.md`'s design constraints — no more are coming); a
handful of match arms is the same scale of accepted duplication
`aint-ir`'s `lower.rs` already has against the type checker's internal
`Binding`/`CallMode`/`EffectInfo` types (see milestone 06/18's
established precedent). Stdlib natives are the opposite case — a much
larger, actively-growing body of logic — so those *are* shared
(previous paragraph).

**`aint run --vm`, opt-in, not a new default.** `Command::Run` gains a
`--vm` flag; without it, `aint run` behaves exactly as it always has.
Fails clearly at compile time (a `CompileError`, printed and a
non-zero exit) on anything outside scope, before running anything —
never a partial run, a wrong answer, or a panic.

## Known limitation

**AIR carries no source positions.** Nothing needed them before this
milestone gave AIR its first executor (18's `AirExpr`/`AirStmt` are
span-free by design, and 19's optimizer never needed one either).
Every VM-produced `RuntimeError` uses a fixed placeholder span
(`1:1`) instead of the real source location the tree-walker reports.
This is a real, honest regression in error *positioning* specifically
for programs run via `--vm` — the error *kind* and *message* are
otherwise identical to the tree-walker's. Adding real spans to AIR is
future work if the VM is ever meant to fully replace `aint run` rather
than exist alongside it; not attempted here, since nothing in this
milestone's actual acceptance bar needs it.

## Explicitly out of scope

- Everything in "What's explicitly not covered, and why," above.
- Sandboxing and portability, `ROADMAP.md`'s other two named
  motivations — both are properties of *deploying* compiled bytecode
  somewhere (a restricted execution environment, a non-Rust target),
  neither of which exists yet. This milestone builds the bytecode and
  the VM that runs it in-process; it doesn't build a sandbox or a
  portable bytecode *format* (serialization) around it.
- Real source-position tracking through AIR (see "Known limitation").
- Any change to the tree-walking `Interpreter` or to plain `aint run`'s
  default behavior.
- LLVM, or any native-code compilation — `ROADMAP.md` says so
  explicitly ("Still no LLVM"), and that's milestone 28's territory if
  it ever happens at all.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
