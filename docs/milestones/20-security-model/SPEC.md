# Milestone 20 — Security model

## Scope

`ROADMAP.md`:

> Permissions, sandboxing, filesystem/network restrictions, tool
> authorization, secret management, resource limits. Non-optional once
> a model can call tools.

Six names. Two are already done elsewhere: "resource limits" is
milestone 17's `budget` block, in full. The rest need to be checked
against what actually exists in the language today, not built on
spec.

## What actually exists to secure, right now

- **Tool calling** (milestone 12) is real and live: a model can
  request any declared `tool` mid-inference, and the runtime executes
  whatever it asks for. `available_tools()` currently hands the model
  *every* declared tool in the program, with no way to scope that
  down per `infer` function. This is the one genuine, live gap the
  roadmap's "non-optional once a model can call tools" line is talking
  about — it exists today, in code already shipped.
- **Filesystem access** doesn't exist in the language at all. There is
  no `io.read`, no file-open primitive, nothing in `crates/runtime`
  that touches the filesystem beyond the interpreter's own stdout
  writer. There is nothing to restrict.
- **Network access** exists in exactly one place: `HttpModel`
  (milestone 16), and it's Rust-level deployment configuration (base
  URL, API key), not something an AINT program directs at runtime. An
  AINT program cannot make an outbound request of its choosing.
- **Secrets** have no representation anywhere — no env var access, no
  credential type, no `secret` keyword. Nothing to manage.
- **Sandboxing** in the OS-process-isolation sense has no execution
  primitive to sandbox against. AINT programs run in-process, as Rust
  code interpreting an AST; the only "untrusted execution" concern
  that exists yet is a model choosing which declared tool to invoke,
  which is exactly the tool-authorization problem above.

## What this milestone actually builds

**Tool authorization**: a `permissions [...]` clause on `infer`
declarations, restricting which specific `tool`s that inference is
allowed to request.

```
tool database_get_email(id: String) -> String
tool send_email(to: String, body: String) -> Bool

infer summarize(id: String) -> String permissions [database_get_email]
```

`summarize` can request `database_get_email` but not `send_email`,
even though `send_email` is declared elsewhere in the same program.
Enforced in two places, on purpose:

1. **What's offered.** The `InferenceRequest.available_tools` list
   sent to the model is filtered down to the permitted set before the
   model ever sees it — a well-behaved model adapter won't even
   suggest a tool it wasn't told about.
2. **What's allowed to execute.** Independent of what was offered,
   the runtime checks every `InferenceOutcome::CallTool` the model
   actually returns against the same permitted set before running it.
   A model implementation that ignores `available_tools` and asks for
   something else anyway gets `RuntimeError::PermissionDenied`, not a
   free pass. "We didn't advertise it" is not a security boundary by
   itself — the enforcement has to live at the point of execution,
   which is the actual `call_requested_tool` path, not the request
   construction.

**No clause at all means unrestricted** — every declared tool remains
available, exactly like before this milestone. This is a deliberate
break from the roadmap's "non-optional" framing: making `permissions`
mandatory by default would silently break every existing agentic
program from milestone 12 onward, none of which declare it. The same
tension exists in miniature with milestone 13's `effects` (also
opt-in, also documented as a conscious deviation at the time).
`permissions` is the mechanism a program *reaches for* to lock an
`infer` function down; it doesn't retroactively lock down programs
that never asked for it.

## Design decisions

**Attached to the `infer` declaration, not the calling `fn`.** A
`tool`'s availability to a model is a property of *which inference is
running*, not of what ordinary AINT code happens to be on the call
stack above it — there's no "ambient" security context to inherit the
way `effects` inherits nothing either. `permissions` sits next to
`infer`'s own signature for the same reason `effects` sits next to
`fn`'s: it's checked against calls made *from inside* that specific
declaration's execution, and an `infer` declaration's only calls are
the tool requests its model conversation makes.

**A flat list of tool names, not a richer policy language.** No
wildcards, no groups, no per-argument restrictions (e.g. "may call
`database_get_email` only for this caller's own `id`"). AINT has no
tool-name namespacing to make wildcards meaningful, and per-argument
policy would need a real expression sublanguage evaluated against
runtime values — a different, larger feature. A flat allowlist is the
smallest thing that actually closes the gap.

**A new `permissions` keyword, not an identifier reused
contextually.** `effects`, `budget`, `test`, `mock`, `assert` are all
real keywords that introduce a clause or statement; milestone 13 hit
a real bug (`tool` colliding with `TokenKind::Tool` inside an
identifier-based word list) from trying to save a keyword. `tool`
names inside `permissions [...]` are themselves already keywords in
some cases (nothing stops a tool from being named the same as a
future keyword), so the list holds identifiers naming *tools*, parsed
the same way `tool`/`infer` declaration names already are.

**Validated against declared tools at type-check time.** A
`permissions` clause naming something that isn't a declared `tool` (a
typo, an `infer`, a plain `fn`, nothing at all) is rejected before the
program runs — the same "don't let this fail silently or at the worst
possible runtime moment" reasoning behind `mock`'s target validation
in milestone 15.

**`crates/ir` is untouched.** AIR's `AirStmt::Infer` doesn't carry
`return_type` today either — nothing consumes AIR at all yet (18, 19),
so there's no execution path there to enforce anything against.
Adding `permissions` to AIR would be plumbing with no consumer, the
same reasoning `lower.rs` already applies to dropping `return_type`.

## Explicitly out of scope

- **Sandboxing** — no untrusted-execution primitive exists beyond
  model-directed tool selection, which `permissions` already covers.
  Process isolation, capability-based OS sandboxing, and similar are
  meaningless without something to isolate; revisit once milestone 22
  (bytecode VM) or a real deployment story gives AINT something to
  actually run untrusted.
- **Filesystem/network restrictions** — no filesystem primitive exists
  in the language to restrict, and the only network-capable component
  (`HttpModel`) is deployment configuration, not something an AINT
  program can direct. Nothing to build a restriction on top of.
- **Secret management** — no secret-shaped value exists anywhere in
  the language (no env var access, no credential type). A `secret`
  primitive is a prerequisite this milestone doesn't have and
  shouldn't invent as a side effect of tool authorization.
- **Per-argument tool policy** (e.g. restricting *which* IDs a
  permitted tool may be called with) — needs runtime value inspection
  against a policy, not just a name check. Not attempted.
- **Effects/permissions unification** — `effects` (13) stays a static,
  compiler-checked property of what *kinds* of side effects a function
  may have; `permissions` (20) is a runtime-enforced allowlist of
  *specific tools* one `infer` declaration's model conversation may
  invoke. Different axes, deliberately not merged into one clause.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
