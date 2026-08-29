# Milestone 29 — Modularity — acceptance

## Scope

See `SPEC.md`. `import "path" as alias`, a new `aint-loader` crate that
resolves the whole cross-file import graph into one flat `Program`
before any other crate sees it, and the parser/lexer support that
syntax needs — minus diamond-shared imports and accurate cross-file
error positions, both named directly as out of scope.

## Acceptance criteria

- [x] Lexer: `as` recognized as a keyword (`crates/lexer/src/token.rs`).
- [x] AST: `StmtKind::ImportFile { path, alias }`, distinct from
      `Import(String)`'s stdlib form (`crates/ast/src/stmt.rs`).
- [x] Parser: `import "path" as alias` parses to `ImportFile`; `import
      name` is completely unaffected. Both covered by direct parser
      tests.
- [x] New crate `aint-loader` (`crates/loader`): `load(&Path) ->
      Result<Program, LoadError>` — parses the entry file, recursively
      resolves every `import "path" as alias` it reaches relative to
      *its own* importing file's directory, renames every declaration
      (and every reference to it) with the importing alias, and splices
      the result in at the import statement's original position.
- [x] Renaming covers every declaration kind and every place its name
      can appear: `fn`/`tool`/`infer` names, `enum` names and their
      `EnumName_Variant` identifier forms, parameter and return types
      (`Type::Enum` positions), call expressions, and `permissions`
      lists — verified directly, including a case asserting the
      renamed `EnumName_Variant` identifier appears correctly inside a
      renamed function body.
- [x] Renaming cascades: a name arriving from an already-resolved
      sub-import keeps getting the outer alias prepended too, so every
      name in the final flattened program is unique by construction.
- [x] A file reached via `import "..." as ..."` may only contain
      `fn`/`enum`/`tool`/`infer`/`import` at its top level; anything
      else (`let` verified directly) is a clear
      `LoadError::IllegalTopLevelStatement`. The entry file keeps its
      unrestricted top level.
- [x] A direct import cycle is rejected as `LoadError::Cycle`, naming
      the full chain — verified directly and through the real CLI.
- [x] The same file imported from two different places in the graph is
      rejected as `LoadError::DuplicateImport` (v1's stated "no diamond
      imports" restriction) — verified directly.
- [x] Two imports in the same file sharing an alias is rejected as
      `LoadError::DuplicateAlias` — verified directly.
- [x] A missing import path is a clear `LoadError::Io`, not a panic —
      verified directly and through the real CLI.
- [x] `aint-typechecker`, `aint-runtime`, and `aint-ir` each gained
      exactly one defensive match arm for `StmtKind::ImportFile` and no
      other change — confirmed by reading each diff.
- [x] `aint-fmt` prints `import "path" as alias` back out correctly;
      `aint fmt --check` on the new example confirms a clean round-trip.
- [x] `crates/cli`'s `parse_and_check` (shared by `run`/`test`/`check`/
      `run --vm`) now calls `aint_loader::load` instead of reading and
      parsing the file directly — one change point, all four commands
      updated for free.
- [x] New example `examples/modularity/` (`main.an` importing
      `util.an`): a shared `enum`, a `pure` function, and a recursive
      function, all called through their `util_`-prefixed names.
      Verified through the real binary: `aint check`, `aint run`,
      `aint test` (including asserting on the imported enum variant and
      the imported recursive function's result), and `aint run --vm` —
      the bytecode VM runs a multi-file program correctly with zero
      VM-specific changes, since resolution happens entirely before
      lowering.
- [x] Real CLI error verification (not just unit tests): an induced
      import cycle, a missing import path, and an illegal top-level
      `let` in an imported file each produce a clear message and a
      non-zero exit code.
- [x] `cargo test --workspace` passes with no regressions: 399 tests
      total, up from 390 before this milestone (9 new: 8 in
      `aint-loader`, 1 new parser test for the `ImportFile` form).
- [x] `cargo build`, `cargo fmt --check`, and
      `cargo clippy --workspace --all-targets -- -D warnings` are clean
      across the whole workspace, including the new crate.

## Known, honestly-stated gaps

- **No diamond imports.** The same file imported from two different
  places in the graph is a hard error, not a legitimate shared
  dependency resolved once. See `SPEC.md`'s "No diamond imports in v1"
  for why this is the restriction that makes cascading-prefix renaming
  sound, and what solving it properly would require.
- **Cross-file error positions are approximate.** `Span` still carries
  no file identity — an error inside spliced-in code from an imported
  file reports the *entry* file's path with a line/column that's
  actually relative to the imported file's own source.
- **`aint-package`'s resolved dependencies still don't feed into
  `aint-loader`.** `aint add`'s lockfile and `import "path" as alias`
  are still two unconnected systems.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — diamond-shared imports,
privacy/visibility, re-exports, accurate cross-file error positions,
wiring `aint-package` into `aint-loader`, and any change to the type
checker/interpreter/VM/IR compiler's actual logic beyond one defensive
match arm each.

## Outcome

Satisfied by a new crate `crates/loader` (`aint-loader`: `lib.rs`,
`LoadError`, `load`, the recursive resolve-and-rename pass, 8 unit
tests against real temporary file trees), lexer/AST/parser support for
`import "path" as alias`, one defensive match arm each in
`aint-typechecker`/`aint-runtime`/`aint-ir`, real `aint-fmt` printing
support, `crates/cli/src/main.rs`'s `parse_and_check` now routed through
the loader, and `examples/modularity/` (`main.an` + `util.an`) verified
end to end through `aint check`/`run`/`test`/`run --vm` and three real
CLI error cases. 399 tests total across the workspace, all passing.
