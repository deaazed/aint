# Milestone 29 — Modularity

## Scope

The first milestone of a new phase. Milestones 0–28 shipped a complete,
self-consistent 1.0 — but every one of them, without exception, is a
single `.an` file. `import <name>` (milestone 06) has only ever resolved
to one of a handful of fixed stdlib module names; nothing lets one
AINT file reference another AINT file's declarations. That's the actual
floor a real framework or a multi-page application needs, and it's what
this milestone builds: `import "./path.an" as alias`.

## What this milestone actually builds

**A new keyword form for `import`.** The existing bare-identifier form is
untouched — `import math` still means exactly what it always has. A
string literal after `import` means a file:

```an
import "./util.an" as util

fn main() -> Unit {
    print(util_greet("world"))
}
```

**A new crate, `aint-loader`**, that turns a multi-file program into the
one flat `Program` every other crate already knows how to handle.
Concretely:

1. Parse the entry file. Walk its top-level statements. For each
   `import "path" as alias`, resolve `path` relative to the *importing
   file's own directory* (not the entry's), and recursively load it the
   same way.
2. Every declaration in an imported file — `fn`, `enum` (and its
   `EnumName_Variant` identifiers), `tool`, `infer` — gets its name
   prefixed with that file's alias: `util.an`'s `greet` becomes
   `util_greet` everywhere it's declared *and* everywhere it's
   referenced, inside that file's own source.
3. Renaming is applied **after** a file's own imports are already
   resolved, to that file's *entire* resulting statement list — so a
   name that arrived from a sub-import (already carrying its own
   sub-alias prefix) gets the outer alias prepended too. A three-level
   import chain produces a three-segment flat name
   (`entry`→`routes`→`db`'s `save` becomes `routes_db_save`). Verbose,
   but it guarantees every name in the final flattened program is
   globally unique without needing real namespacing anywhere downstream
   — the type checker, interpreter, VM, and IR compiler all still see
   one ordinary flat `Program`, unmodified, exactly as they did before
   this milestone.
4. `import "path" as alias` statements are removed once resolved; the
   imported file's (now-renamed) statements are spliced in at exactly
   the position the import statement occupied.

**A file being imported may only declare things** — its top level is
restricted to `fn`, `enum`, `tool`, `infer`, and further `import`
statements (both forms). A `let`, `if`, bare expression, `test`, `mock`,
`assert`, or `budget` at an imported file's top level is a clear
`aint-loader` error, not a silent no-op or a surprising side effect that
fires just because the file was imported. The entry file keeps today's
unrestricted top level — this restriction only applies to files reached
*through* an `import "..." as ...`.

**Every crate downstream of the loader is unchanged.** `aint-typechecker`,
`aint-runtime`, `aint-ir`, and `aint-vm` gain exactly one defensive match
arm each for the new `StmtKind::ImportFile` — reachable only if something
calls them directly on unresolved source, bypassing `aint-loader`
entirely, which none of `aint check`/`run`/`test`/`run --vm` ever do
after this milestone. `aint-fmt` is the one exception: it must print
`import "path" as alias` back out correctly, since formatting a file
never resolves its imports.

## Design decisions

**No diamond imports in v1 — every file may be imported from exactly one
place in the whole program.** A second `import` of the same canonical
path, reached from anywhere else in the graph, is a clear
`aint-loader::LoadError::DuplicateImport`, not a silent second copy.
This is the single restriction that makes the cascading-prefix renaming
scheme above sound: with at most one importer per file, the chain of
aliases from the entry down to any given file is unique, so cascaded
renaming can't produce two different flat names for what should be "the
same" declaration reached two different ways (the alternative — sharing
one file's declarations across two importers — needs each shared
declaration to keep one canonical identity regardless of import path,
which cascading-by-alias doesn't give you; solving that properly is a
real, separate design problem, deferred rather than gotten wrong here).

**Cycles are rejected with the full chain**, reusing the same
stack-based detection shape `aint-package`'s `resolve.rs` already uses
for manifest dependency cycles — proven, simple, easy to test the same
way.

**No privacy/visibility.** Every top-level declaration in an imported
file is implicitly available to its importer once renamed — consistent
with AINT having no privacy concept anywhere else yet.

**`aint-package` stays disconnected from this.** `aint.toml`/`aint.lock`
still describe a dependency graph of *packages*; nothing here teaches
`aint add`'s resolved dependencies to feed into `aint-loader`. Wiring a
package dependency's directory into an importable path is a natural
follow-up, not required to make same-project multi-file programs work,
which is what this milestone actually targets.

## Explicitly out of scope

- **Diamond-shared imports** (the same file legitimately imported from
  two different places) — see above.
- **Privacy/visibility modifiers.**
- **Re-exports** (a file re-exposing an alias it imported under its own
  name).
- **Accurate cross-file error positions.** `Span` carries no file
  identity today. An error inside spliced-in code from an imported file
  is reported using the *entry* file's path with a line/column number
  that's actually relative to the imported file's own source — a real,
  known limitation for v1, not silently pretended away. Fixing this
  needs threading a file id through every `Span`, which is a bigger,
  separate change.
- **Wiring `aint-package`'s resolved dependencies into `aint-loader`.**
- **Any change to the type checker, interpreter, VM, or IR compiler's
  actual logic** — they gain one defensive match arm each and nothing
  else; see "What this milestone actually builds."

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
