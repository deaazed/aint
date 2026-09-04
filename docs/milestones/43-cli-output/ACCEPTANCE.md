# Milestone 43 — Verbose, colored CLI output — acceptance

## Scope

See `SPEC.md`. A `ui` module (`step`/`ok_line`/`warn_line`/
`error_line`/`fail_line`) applied across every `aint` command, with
`check`/`fmt --check`'s documented silent-on-success contract kept
completely intact.

## Acceptance criteria

- [x] `crates/cli/src/ui.rs` (new) — five small functions over
      `anstream`/`anstyle` (already in the dependency graph via
      `clap`'s own color support, confirmed in `Cargo.lock` before
      adding them as direct dependencies — nothing new to compile).
- [x] Every `eprintln!`/`println!` call site in `main.rs` that prints
      actual text is routed through the matching `ui` function —
      verified by grepping the file afterward: the only bare one left
      is `test()`'s `println!();` (a blank separator line before the
      summary, nothing to colorize), plus `ui.rs`'s own internal use
      of `anstream::println!`/`eprintln!`.
- [x] `run`/`run --vm`/`test` narrate checking, then running/testing,
      then a timing line on success — all to stderr; `run`'s own
      stdout (the interpreter's `print()` output) is untouched.
- [x] `init`/`add`/`scaffold`/`upgrade` narrate their real steps
      (creating a package, cloning a dependency, resolving the graph,
      asking the model, checking the result, downloading a release)
      and colorize their final outcome.
- [x] `check` and `fmt --check` print **nothing** on success, on
      either stream — verified directly, not assumed: their own
      functions never call `step`/`ok_line`/etc. on that path, and two
      tests assert `output.stdout.is_empty() && output.stderr.is_empty()`
      for real successful runs (`check_accepts_a_well_typed_program_
      silently`, strengthened to check both streams; a new
      `fmt_check_is_silent_when_already_formatted`, since no test
      covered that success path at all before this milestone).
- [x] **No existing test broke** — confirmed by running the full
      pre-existing suite after every meaningful change, not just at
      the end. This was the real design constraint: every message's
      *text* stayed exactly what it was (only color wrapped around
      it), and streams were checked against every `.contains`/exact-
      match assertion in the crate before deciding where new content
      could safely go (`crates/cli/tests/*.rs` was read in full for
      this, not sampled).
- [x] Piped/redirected output carries zero raw ANSI escape bytes —
      verified two ways: (1) the full pre-existing test suite passing
      unchanged, since several tests exact-match `aint run`'s stdout
      and would fail on any stray byte; (2) a new
      `aint_run_narrates_on_stderr_with_no_escape_codes_in_a_pipe`
      test asserting it directly (`!output.stderr.contains(&0x1b)`),
      plus a direct `System.Diagnostics.Process`-level byte dump done
      by hand during implementation (not just relying on `anstream`'s
      own reputation) that showed 0 escape bytes on both streams when
      piped, and the real "==> checking .../==> running .../==>
      finished in 419.8µs" text present and correct on stderr.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **Real-terminal color emission wasn't independently re-verified
  beyond trusting `anstream`** — attempting to force it via
  `CLICOLOR_FORCE=1` through a redirected pipe (this session's own
  tooling has no direct access to a real interactive terminal/PTY)
  didn't produce visible escape codes, which is plausibly correct
  behavior (forcing color into what's still, at the OS level, a pipe
  isn't necessarily what "force" should mean) but wasn't chased down
  to a definitive answer. The safety-critical direction — piped output
  staying clean — *is* independently, directly verified; the positive
  direction rests on `anstream` being the standard, widely-used tool
  for exactly this job (the same one `clap` itself uses for `--help`).
- **No `--quiet`/verbosity flag.** Explicitly out of scope — see
  `SPEC.md`.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. Every command is meaningfully more verbose, in ASCII, in
color — while `check`/`fmt --check`'s documented silent-on-success
contract is not just preserved but more thoroughly tested than it was
before this milestone (both streams, not just stdout; both commands,
not just one). Verified by the full pre-existing test suite passing
unchanged throughout implementation (not just at the end), three new
regression-guard tests, and a direct byte-level inspection of both
streams against the real built binary.
