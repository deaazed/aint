# Milestone 41 — `aint` loads `.env` automatically

## Scope

`aint` has never read a `.env` file — every real model call needed
`AINT_MODEL_URL`/`AINT_MODEL_NAME`/`AINT_MODEL_API_KEY` exported into
the shell by hand first. That's why the standalone `aint-website`
project needed its own `run.ps1` wrapper script just to start the site
with real credentials, even though `.env`/`.env.example` was already
this project's own established convention for holding them
(`CONTRIBUTING.md`'s "Live verification against a real model"). See
`ROADMAP.md`'s Phase 3 framing.

## What this milestone actually builds

**`aint` loads `.env` from the current directory once, at the very
start of `main`, before dispatching to any subcommand.** Not scoped to
`run`/`test` specifically — `aint scaffold` needs `AINT_MODEL_URL` just
as much as `aint run` does, and loading it once, unconditionally, up
front costs nothing for the commands that never touch a model
(`check`/`fmt`/`init`/`add`, and `test`, which always mocks regardless
of what's configured — see `CONTRIBUTING.md`).

**A real environment variable always wins.** `.env` only fills in a
variable that isn't already set (checked with `std::env::var_os`
before ever calling `set_var`) — never overrides one a caller already
exported, matching the precedence every other dotenv-style tool uses.

**The format is deliberately minimal**: `KEY=VALUE`, one per line,
blank lines and `#`-comment lines skipped, split at the first `=` (so
a value containing one — a URL query string, say — survives intact),
both sides trimmed. No quote-stripping, no multi-line values, no
`export` prefix — exactly the shape this project's own `.env.example`
files already use, not a general-purpose dotenv parser.

**Silent, harmless no-op when no `.env` file exists.** This is a
convenience layered on top of the existing `AINT_MODEL_URL`/etc.
environment-variable story, not a new requirement — a program that
never used `.env` before this milestone behaves identically after it.

## Design decisions

**`std::env::set_var` runs exactly once, on the main thread, before
the interpreter thread is ever spawned.** `set_var` documents itself
as unsound to call while another thread might be reading the
environment concurrently — `aint`'s only other thread is the one
`main` spawns immediately after (`STACK_SIZE`'s own doc comment
explains why that thread exists at all). Loading `.env` first, before
that thread starts, means nothing is ever reading the environment
concurrently with the one place it's written.

**Parsing (`parse_dotenv`) is a pure function, kept separate from
applying it (`load_dotenv`, the only thing that touches real process
state)** — the same split this project already draws elsewhere
between logic and its impure edges. `parse_dotenv` gets thorough,
fast unit tests; `load_dotenv`'s actual effect on a real process is
verified by two CLI integration tests that spawn the real `aint`
binary against a real (mock) HTTP server, from a temp directory
holding a real `.env` file, with `AINT_MODEL_URL`/etc. explicitly
removed from the child's own environment first — proving the value
can only have come from the file, not something inherited or
exported by the test itself.

## Explicitly out of scope

- **A general-purpose dotenv parser** (quoted values, multi-line
  values, variable interpolation, an `export` prefix). Real, separate,
  additive work if a future need shows up — this project's own `.env`
  files have never needed any of it.
- **Searching upward for `.env`** the way `find_package_root` does for
  `aint.toml` — current-directory-only, matching how `aint-website`'s
  own `.env` already lives at the project root a user would actually
  run `aint` from.
- **`.env` for anything beyond environment variables** (a config file
  format, CLI flag defaults, etc.) — out of scope; this is specifically
  about the credential-export friction the retrospective found.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
