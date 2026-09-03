# Milestone 41 — `aint` loads `.env` automatically — acceptance

## Scope

See `SPEC.md`. `load_dotenv`/`parse_dotenv` in `crates/cli/src/main.rs`
— every `aint` subcommand gets `.env` loaded before it runs, with a
real environment variable always taking precedence.

## Acceptance criteria

- [x] `load_dotenv()` called first thing in `main()`, before
      `Cli::parse()` and before the interpreter thread is spawned.
      Reads `.env` from the current directory; does nothing if it
      doesn't exist.
- [x] `parse_dotenv(text: &str) -> Vec<(String, String)>` — a pure
      function, `KEY=VALUE` per line, blank/`#`-comment lines skipped,
      split at the first `=`, both sides trimmed. 6 new unit tests
      (basic pairs, comments/blank lines, whitespace trimming, a value
      containing `=`, a line with no `=` at all, empty input).
- [x] `load_dotenv` only sets a variable via `std::env::set_var` when
      `std::env::var_os` shows it isn't already set — a real exported
      variable always wins. The one `unsafe` block (`set_var` requires
      it) is justified with a `SAFETY` comment: called once, from
      `main`, strictly before the only other thread `aint` ever spawns
      exists, so nothing can be reading the environment concurrently.
- [x] Two new CLI integration tests
      (`crates/cli/tests/dotenv.rs`) — both spawn the real built
      `aint` binary against a real local mock HTTP server (the same
      pattern `aint-runtime`'s own `http_model.rs` test module uses
      internally, duplicated rather than exposed across the crate
      boundary):
      - `aint_run_loads_dotenv_and_uses_it_for_a_real_model_call`: a
        `.env` in a fresh temp directory, `AINT_MODEL_URL`/`NAME`/
        `API_KEY` explicitly removed from the child process's own
        environment, `aint run` from that directory still reaches the
        mock server and prints its response — provable only if
        `load_dotenv` actually read the file, since nothing else could
        have supplied the URL.
      - `a_real_env_var_overrides_the_dotenv_value`: `.env` points at
        an unreachable address; a real environment variable pointing
        at the mock server is also set on the child. The real value
        wins, proving `.env` only fills in what's unset.
- [x] `docs/SPECIFICATION.md` §7 documents the new behavior (loaded
      once per invocation, real env vars win, minimal format, silent
      no-op when absent); the milestone-41 "not started" known-gap
      entry removed now that it's done.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **No general-purpose dotenv parser, no upward search for `.env`.**
  Explicitly out of scope — see `SPEC.md`.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied — the last of Phase 3's five milestones. Every `aint`
subcommand picks up real model credentials from a `.env` file with no
export step, closing the exact friction that made `aint-website` need
its own wrapper script. Verified by the full pre-existing test suite
passing unchanged, 6 new unit tests for the parser, and two real
integration tests proving both the loading behavior and the
real-environment-variable-wins precedence against the actual built
binary and a real local server — not asserted by inspection, but by a
program that could only produce its actual output if the file was
genuinely read.
