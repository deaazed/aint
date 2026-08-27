# Milestone 21 — Memory model

## Scope

`ROADMAP.md`:

> Decide GC vs. reference counting vs. ownership vs. arena allocation,
> if and when the Rust-managed runtime objects from earlier milestones
> stop being sufficient. Don't invent this early.

A decision milestone, not a build-something milestone — same shape as
milestone 00. The runtime has used `Rc<RefCell<_>>` for `Environment`
since milestone 04, and every value type added by the twelve
milestones since (`Function`, `Task`, `InferenceFn`, `PendingInference`,
`ToolFn`, `PendingToolCall`, `Distribution`, `Enum`, `Option`) has kept
using it without the question ever being revisited in writing. This
milestone is that revisit: does anything built since still fit inside
what reference counting can actually guarantee, or has something
quietly introduced a case Rc can't handle?

## The actual question reference counting lives or dies on

Rc's one real weakness is reference cycles: two objects each holding a
strong reference to the other never get freed, because their counts
never reach zero. Everything else `ROADMAP.md` lists — GC, ownership,
arenas — is a response to that weakness (or to raw allocation
throughput, a separate and, per milestone 26, not-yet-measured
concern). So the question this milestone actually has to answer is
narrow and checkable: **can anything in this codebase form a reference
cycle?**

## What was checked

`Environment` (`crates/runtime/src/environment.rs`) is the only type
in the runtime holding an `Rc<RefCell<_>>` to another instance of
itself:

```rust
pub struct Environment {
    values: HashMap<String, Value>,
    parent: Option<Rc<RefCell<Environment>>>,
}
```

`parent` points strictly upward — a child never appears in its own
parent's fields, and nothing anywhere holds a `Weak`/`Rc` pointing back
down from a parent to a child. Taken alone, this makes the environment
graph a tree by construction, and trees can't cycle. The only way a
cycle could still form is if a `Value` stored inside `values` could
itself hold a reference back to an `Environment` — a closure capturing
its defining scope and then getting `let`-bound back into that same
scope is the textbook version of this.

That doesn't happen here, and the reason is structural, not
incidental: `Value` (`crates/runtime/src/value.rs`) has thirteen
variants as of milestone 20, and not one of them — not `Function`, not
`Task`, not `InferenceFn`/`PendingInference`,
not `ToolFn`/`PendingToolCall` — carries an `Environment` reference.
`Function` holds `name`, `params`, `body: Block`, `is_async`: an AST
fragment and metadata, nothing runtime-shared. This is milestone 04's
original "no real closure semantics yet" decision
(`run_function` parents every call frame to `self.globals`, never to
the caller's or a lexically-enclosing environment) — a decision that
was never about memory management at the time, but turns out to be
exactly what keeps the object graph acyclic five milestones into the
AI-specific surface area. `List(Vec<Value>)` and `Option(Option<Box<Value>>)`
are the only self-referential-*shaped* variants, and they can only
nest other `Value`s — which, transitively, still can't reach an
`Environment` — and AINT's lack of mutation/reassignment means there's
no way to construct a value that contains itself even at the `List`
level (`let x = [x]` isn't expressible: `x` doesn't exist yet when the
list literal referencing it evaluates, and nothing can mutate a list
in place afterward to insert itself).

## Decision

**Reference counting stays. No garbage collector, no ownership-model
rewrite, no arena allocator.** The environment/value object graph is
acyclic by construction, for reasons tied to specific, load-bearing
design choices already made and tested (no closures capturing
environments since milestone 04; no mutation/reassignment since the
language's inception) rather than by luck. Building a cycle-collecting
GC to solve a problem that provably can't occur here would be exactly
the kind of early invention `ROADMAP.md` warns against.

**What would actually force revisiting this:**

- **Real closures** — a `fn` or `infer` value capturing its defining
  `Environment` (not just parenting to globals) is the one change that
  would let a `Value` reach back into an `Environment`, and combined
  with a `let` binding a closure into its own enclosing scope, would
  create a genuine cycle. If closures are ever added, *that* milestone
  needs to re-open this decision, not silently inherit it.
- **In-place mutation of container values** — if `List`/`Option` (or
  anything added later) ever gained a mutating operation, "no way to
  construct a self-containing value" stops being true, and the
  acyclic argument above needs to be re-checked against whatever
  mutation was added.
- **Measured allocation-throughput pressure** — arena allocation is a
  performance answer, not a correctness one; nothing about it is
  ruled out by this decision, it's just unmotivated until milestone 26
  (benchmarking) produces an actual number showing `Rc`/`RefCell`
  churn matters relative to the AI-call latency that dominates any
  realistic AINT program today.

None of these exist yet. This decision holds until one of them does.

## Design decisions

**A decision milestone gets a decision, backed by a specific,
checkable argument — not a survey of GC algorithms nobody's going to
build.** Same shape as milestone 00: the deliverable is the reasoning
being written down somewhere findable, not new runtime code.
`crates/runtime/src/environment.rs` gets a doc comment stating the
acyclic invariant directly at the type that would break first if it
ever stopped holding, so a future contributor adding closures finds
the constraint before they violate it, not after.

**Checked against `Value` as it exists today, not as a permanent
guarantee.** The argument above is only as good as the enumeration of
`Value` variants it's checked against; it's explicitly re-openable,
not a proof that holds forever regardless of what's added later.

## Explicitly out of scope

- Building any actual GC, arena, or ownership-model alternative —
  none is motivated.
- Performance measurement of the current `Rc`/`RefCell` approach —
  that's milestone 26.
- Revisiting `Value::clone()`'s cost (every `Environment::get` clones
  the stored `Value` out — cheap for `Rc`-wrapped variants, a real
  deep copy for `List`/`Option`) — a possible future optimization
  target, not a correctness question, and not measured yet.

## Outcome

To be filled in `ACCEPTANCE.md` once the code-level doc comment lands.
