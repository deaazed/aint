# AINT — Backward compatibility policy

What's guaranteed to keep working across versions, starting at 1.0,
and what isn't yet. Written at milestone 28 alongside
`docs/SPECIFICATION.md`, which this policy protects.

## Versioning

AINT follows semantic versioning: `MAJOR.MINOR.PATCH`.

- **PATCH** — bug fixes that make behavior match `SPECIFICATION.md`
  more closely, or fix a defect with no correct program that could
  have depended on the old (wrong) behavior. The path-traversal fix
  to `db` (milestone 28) is the shape of a patch release: no correct,
  intended usage of `db_insert`/`db_get`/etc. relied on table names
  being unvalidated.
- **MINOR** — new, additive language or stdlib surface: a new stdlib
  module, a new statement form, a new `RuntimeError`/`TypeError`
  variant for a genuinely new failure mode. Existing, correct programs
  keep compiling and running the same way.
- **MAJOR** — anything that changes the meaning of an existing,
  correct program, or removes something `SPECIFICATION.md` documents
  as present.

## What's covered

Once tagged 1.0, these are the compatibility surface — changing any
of them without a major version bump is a breaking change, full stop,
whether or not it looks small:

- **Grammar and syntax** — every construct in `SPECIFICATION.md` §2–5.
  A program that parses today parses the same way tomorrow.
- **Type-checking rules** — a program that type-checks today
  type-checks tomorrow; a program that's rejected today may start
  being *accepted* in a minor release (a checker becoming more
  permissive without changing accepted programs' meaning is additive),
  but never the reverse without a major bump.
- **Stdlib function signatures and behavior** — every function listed
  in `SPECIFICATION.md` §9: its parameter types, return type, and
  documented behavior. `db_insert`'s "false on a duplicate id, not an
  overwrite" is part of the contract, not an implementation detail.
- **CLI subcommands and flags** — `aint run`/`aint run --vm`/
  `aint test`/`aint check`/`aint fmt`/`aint fmt --check`/`aint init`/
  `aint add`, and the environment variables `aint run` reads
  (`AINT_MODEL_URL`/`AINT_MODEL_NAME`/`AINT_MODEL_API_KEY`).
- **File formats** — `aint.toml`'s schema, `aint.lock`'s schema, the
  `.aintdb/<table>.jsonl` row shape (`{"id": ..., "record": ...}`).
- **Error variant *names*, not their exact `Display` text.** Code
  matching on `RuntimeError::PermissionDenied` keeps working; the
  exact wording of the message it formats may still improve.

## What's explicitly not covered

Everything `SPECIFICATION.md` §11 lists as a known gap is, by
definition, not something a correct program can depend on today —
there's no existing behavior to preserve:

- Cross-file `import` doesn't exist, so its eventual syntax and
  semantics aren't constrained by anything shipped yet.
- `Option<T>`/`Distribution<T>` construction syntax, list
  concatenation, and `Int`/`String` conversion are all absent; adding
  any of them is new surface, not a compatibility question.
- The bytecode VM's scope (§6) is expected to grow — supporting
  `infer`/`tool`/`await` under `--vm` someday is additive, not
  breaking, since a program that already fails clearly under `--vm`
  has no behavior to preserve.
- **Internal Rust crate APIs are not covered at all.**
  `aint-ast`/`aint-lexer`/`aint-parser`/`aint-typechecker`/`aint-ir`/
  `aint-runtime`/`aint-vm`/`aint-package`/`aint-fmt` are implementation
  details of the `aint` toolchain (`publish = false` in every crate's
  `Cargo.toml` — none of them are, or are intended to become, a public
  Rust library). Their types and function signatures may change in any
  release. Only the `.an` language and the `aint` CLI surface are
  covered by this policy.
- **Trace/tracing output shape** (`TraceRecord`'s exact fields) is not
  yet covered — `TokenUsage` being always-zero is a known, stated
  placeholder (`docs/milestones/14-ai-execution-tracing/SPEC.md`), and
  the whole shape is expected to change once a `Model` implementation
  reports real usage.

## Deprecation process

1. A deprecated construct keeps working, with a compiler warning
   (once warnings-that-aren't-errors exist as a category — today,
   `aint check`/`aint run` only report hard errors) for at least one
   MINOR release.
2. Removal happens only at a MAJOR release, documented in that
   release's own changelog entry with the MINOR release it was first
   deprecated in.
3. No silent behavior change ever ships as a patch — this is already
   `LANGUAGE_DESIGN.md`'s design principle 10 ("established semantics
   don't change silently"), restated here as a release-process
   commitment, not just a code-review one.

## Where this policy doesn't yet have teeth

Stated plainly rather than implied: there is no released 1.0 build
artifact yet, no changelog automation, and no compatibility test
suite that runs old example programs against new compiler versions to
catch an accidental break. This document is the *policy* milestone 28
commits to going forward; enforcing it mechanically (a corpus of
frozen example programs + expected output, checked on every change)
is real, future work, not claimed as done here.
