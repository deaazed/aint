# Milestone 34 — Real tool execution

## Scope

Not part of the original five-milestone Phase 2 plan — added after a
direct look at what was still missing for the language's core pitch to
hold up. `infer` got a real backend at milestone 16 (`HttpModel`,
verified against a real vendor in this milestone's own testing —
Mistral, over the actual chat completions API). `tool` never did:
`MockTool` has been the *only* tool executor that has ever existed,
whether a tool was called directly or requested by a model mid-
conversation. That's a real gap in the language's own governance
story — `permissions`/`budget` are supposed to be about what a real
system lets an AI touch, but nothing an AI could touch was ever real.

## What this milestone actually builds

**`tool name(params) -> Type` may now have a body**, exactly like `fn`:

```an
tool square(x: Int) -> Int {
    return x * x
}
```

`await square(7)` now runs the body for real — ordinary AINT source,
which can call stdlib functions (`db`, `http`, `string`, anything else
already gated behind `import`) exactly the way a plain `fn` can. A
`tool` declared without a body (`tool name(params) -> Type`, no `{
... }`) keeps every bit of its old behavior: signature-only, routed
through `MockTool` whether called directly or requested by a model,
exactly as before this milestone. Nothing already written breaks.

**Precedence: an explicit `mock` always wins, even over a tool with a
real body.** A test that writes `mock square -> 999` is stating it
doesn't want `square`'s real implementation to run for that test, not
asking permission for it to run anyway. `call_tool_traced` (the one
place a tool call is ever actually answered, whether direct or
model-requested) checks `MockTool` first; only falls back to a real
body when nothing's mocked; falls back to `MockTool`'s own "no mock
configured" error when neither exists. This was a real bug caught
during implementation, not a hypothetical: the first version ran a
real body unconditionally, which would have silently broken every
existing test for a tool that later gained an implementation.

## Design decisions

**A tool body is type-checked exactly like a `fn` body** — its own
scope, parameters bound, `MissingReturn` checked the same way,
*untracked* for effect-checking purposes (the same treatment a lambda's
body gets): a tool is already its own effect boundary, so its body
doesn't inherit whatever `effects` clause a surrounding function (if
any) declared.

**`ToolFn` (the runtime value backing a declared tool) gains an
`Option<ToolBody>`, not a required one** — `body: None` is the
zero-cost, fully-compatible case for every tool declared before this
milestone. `ToolBody` captures its defining environment exactly the
way `Function::captured_env` does (milestone 30's same argument: sound
because nothing in AINT mutates a binding after creation), even though
in practice every tool is declared at the top level today, so it's
always `globals` — kept general rather than hardcoded, matching how
`Function` already handles this.

**The bytecode VM's relationship to tools is unchanged, not newly
broken.** `await` — the only way to ever actually invoke a tool call,
mocked or real — was already unconditionally rejected under `aint run
--vm` (`CompileError::Unsupported`, needs an async dispatch loop
nothing in the VM has). A tool with a real body still can't run under
`--vm`, for the same pre-existing reason a mocked one never could;
`aint-ir`'s own lowering already drops a tool declaration's body when
building `AirStmt::Tool` (via its existing `..` pattern) — inert, not a
silent miscompilation, since nothing that would observe the dropped
body can ever reach it: any attempt to `await` the tool fails first,
at the same wall every tool call already hit.

**Verified against a real model for the first time, live, in this
session** — not just against a mock server the way milestone 32's
`ChatClient` was. `infer` (unrelated to this milestone but exercised
alongside it) and `aint scaffold` were both run against the real
Mistral chat completions API, closing out a known gap milestone 32's
own `ACCEPTANCE.md` named directly ("never verified against a live
LLM"). `aint scaffold`'s own safety mechanism — write the file either
way, only report success if it actually type-checks — worked exactly
as designed against a real, imperfect model response.

## Explicitly out of scope

- **`async` tool bodies.** A tool body is always synchronous, same
  restriction closures (milestone 30) already accepted for the same
  reason — no async dispatch loop exists for either yet.
- **Any change to `permissions`/`budget` enforcement.** Both already
  apply identically regardless of whether a tool has a real body —
  nothing about *which* tools an inference may reach, or how many
  calls it can make, changes here.
- **Running a real tool body under `aint run --vm`.** Blocked by the
  pre-existing `await` rejection wall, not attempted or worked around.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
