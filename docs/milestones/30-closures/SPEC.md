# Milestone 30 — Closures

## Scope

Functions become values. Before this milestone, a function name could
only ever appear in call position (`name(...)`) — referencing one bare
was a type error, and a call's callee had to be a plain identifier
naming a known top-level `fn`/`infer`/`tool`. That's the whole reason
strategy/observer/dependency-injection-shaped patterns couldn't be
expressed in AINT without duplicating code per concrete case: there was
no way to pass *behavior* around.

This is deliberately the **smallest lever**, not a general type-system
expansion — no generics, no structs, no interfaces/traits. Those stay
out of scope; see `ROADMAP.md`'s Phase 2 framing.

## What this milestone actually builds

**A closure's type**: `Type::Function(Vec<Type>, Box<Type>)` —
parameter types, then the return type. Written in source as
`fn(Type, Type) -> Type`, the one type spelling that isn't a bare
identifier (`crates/parser`'s `parse_type` checks for a leading `fn`
token before falling back to its identifier-only path).

**A lambda expression**: `fn(params) -> ReturnType { body }` in
*expression* position — no name, no `async`, no `effects` clause. A
lambda is always synchronous and untracked, exactly like a top-level
`fn` with no `effects` clause:

```an
let add_one = fn(x: Int) -> Int {
    return x + 1
}
print(add_one(4))
```

**A plain, synchronous, non-`infer`/`tool` top-level `fn`, referenced
bare, now decays to the same closure value** a lambda would — the
existing "`name` is a function; call it with `name(...)`" rejection is
relaxed for this one case, kept for every other (`async fn`/`infer`/
`tool`/the four `Polymorphic*` stdlib bindings), since those would need
`Task<T>`/`Inference<T>`/`Tool<T>` to interoperate with closures, which
is out of scope here.

**Calling a closure value** works through any expression, not just a
bare identifier — an index into a `List<fn(...) -> T>`, an
immediately-invoked lambda, another call's result:

```an
fn apply_twice(f: fn(Int) -> Int, x: Int) -> Int {
    return f(f(x))
}
print(apply_twice(fn(x: Int) -> Int { return x * 2 }, 3))   // 12

let handlers = [fn(x: Int) -> Int { return x + 1 },
                fn(x: Int) -> Int { return x * x }]
print(handlers[0](5))   // 6
```

**Capture is by reference to the live scope, and that's sound because
nothing in AINT ever mutates.** `Value::Function` gains a
`captured_env: Rc<RefCell<Environment>>` field — the environment active
where the function was *defined*, not `globals` unconditionally as
before. For a top-level `fn`, that's still always `globals` (unchanged
behavior). For a lambda, it's whatever scope was active at the point
the lambda expression was evaluated — which is what makes a closure
returned from a function still see that function's own locals after it
has returned:

```an
fn make_adder(n: Int) -> fn(Int) -> Int {
    return fn(x: Int) -> Int {
        return x + n
    }
}
let add5 = make_adder(5)
print(add5(1))   // 6 — `n` is still 5, long after make_adder returned
```

Since `let` bindings never change after creation anywhere in AINT,
sharing the `Rc<RefCell<Environment>>` (not deep-copying it) is
equivalent to capturing by value — nothing captured can be mutated out
from under a closure. This is the load-bearing reason closures are safe
to add without revisiting the no-reassignment design.

## Design decisions

**Interpreter-only. The bytecode VM and IR compiler reject a closure
explicitly, not silently.** A `fn(...) -> T { ... }` lambda expression
fails immediately at IR lowering (`LowerError::UnsupportedLambda`) —
there's no AIR node for it at all. Calling a *named* variable that
happens to hold a closure (`let f = fn...; f(5)`) is, separately,
already safe by construction: `aint-vm`'s compiler resolves a call's
callee only against its compile-time table of actual top-level
functions and stdlib natives (`crates/vm/src/compiler.rs`'s
`AirExpr::Call` handling) — a local variable was never in that table,
so it fails with `CompileError::UndefinedName`, not a miscompilation.
Both are documented parity gaps, consistent with the VM's existing
"deterministic core only" scope.

**A closure is always untracked for effect-checking purposes** — same
rule an unannotated top-level `fn` already follows. A `pure` (or any
`effects [...]`-declared) function cannot call a closure value, even
one it received as a parameter; the closure's own body isn't
effect-checked against whatever the *caller* declared either. No new
concept: this reuses `EffectInfo::Untracked`'s existing incompatibility
rule.

**No privacy leak into `Function`'s equality.** `Value::Function` now
holds live environment state, which can't derive `PartialEq` the way it
used to (`Environment` isn't, and shouldn't be, comparable). `Function`
gets a manual `PartialEq` comparing only its declaration (name, params,
body, `is_async`) — the same notion of equality every top-level `fn`
already had implicitly (its capture was always `globals`, never part of
any comparison).

**Rc-cycle risk, flagged, not solved.** A true reference cycle needs a
closure's `Rc` to end up stored back into an environment it
transitively captures. v1 lambdas are anonymous with no self-recursion
mechanism, so this is structurally unreachable today — worth
re-checking if a future milestone gives closures a way to reference
themselves, not solved preemptively with `Weak` pointers now.

## Explicitly out of scope

- **Generics, structs, interfaces/traits.** See `ROADMAP.md`'s Phase 2
  framing — deferred until real framework-building shows what's
  actually needed.
- **`async` lambdas**, or any interaction between closures and
  `Task<T>`/`Inference<T>`/`Tool<T>`.
- **The bytecode VM/IR compiler executing closures at all** — a
  documented gap, not attempted here.
- **Diamond/shared-capture cycle handling** beyond what's structurally
  already impossible — see "Rc-cycle risk" above.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
