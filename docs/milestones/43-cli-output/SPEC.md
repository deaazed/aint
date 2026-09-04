# Milestone 43 — Verbose, colored CLI output

## Scope

Every `aint` command has been close to silent since the CLI existed —
a final one-line confirmation at most, nothing narrating what actually
happened in between. Requested directly: make the CLI more verbose on
every run, with color, using plain ASCII rather than Unicode/emoji
decoration.

**The one real constraint, confirmed before starting**: `aint check`
and `aint fmt --check` are documented as silent on success on
purpose, matching `gofmt -l`/`tsc --noEmit`'s convention that scripts
and CI can rely on empty output meaning "fine." Given the choice
between preserving that and making every command uniformly verbose,
the decision was to keep those two exactly as documented and make
every other command (and every failure path, including theirs)
noticeably more verbose and colored.

## What this milestone actually builds

**A small `ui` module** (`crates/cli/src/ui.rs`) with five functions,
each auto-detecting whether color is actually appropriate:

- `step(text)` — `==> text`, cyan, to stderr. Narration: "about to do
  this." Never called from `check`'s or `fmt --check`'s success path.
- `ok_line(text)` — green, to stdout. A command's own final "this
  succeeded" line.
- `warn_line(text)` — yellow, to stderr. Worth flagging, not
  necessarily fatal (`fmt --check` naming an unformatted file,
  `scaffold`'s "doesn't type-check" notice).
- `error_line(text)` — red, to stderr. A colorized drop-in for every
  `eprintln!("error: ...")`-shaped line the CLI already had — the
  message text is never changed, only wrapped in color.
- `fail_line(text)` — red, to *stdout*. The one place red belongs on
  stdout rather than stderr: `aint test`'s per-test `FAILED` line is
  part of the same result stream as the `ok` lines next to it, not a
  separate diagnostic channel.

Built on `anstream`/`anstyle` — already present in the dependency
graph via `clap`'s own `--help` coloring, so this adds nothing new to
compile. `anstream` decides on its own whether the destination is a
real, color-capable terminal (`NO_COLOR` unset, not a pipe/file) and
strips the ANSI codes itself otherwise; every function above is safe
to call unconditionally, with no caller-side terminal detection.

**Every command narrates its real steps**, not just a final line:
`run`/`run --vm`/`test` announce checking, then running/testing, then
a timing line on success; `init` announces creating the package before
confirming; `add` announces cloning a git dependency and re-resolving
the graph; `scaffold` announces what it's asking the model for and
when it's checking the result; `upgrade` announces checking the
latest release and downloading. `check` and `fmt --check` announce
nothing on their success path — not even to stderr — since printing
before knowing the outcome would break the "silent means fine"
contract on its own, independent of anything else.

## Design decisions

**ASCII only, no Unicode/emoji.** `==>` (not `⇒`/`→`), plain
`ok`/`FAILED`/`error:`/`warning:` words carrying the meaning color
reinforces rather than replaces — legible even with color stripped
(a pipe, `NO_COLOR`, a terminal with a limited code page), which
matters more for a language toolchain than it would for something
never run non-interactively.

**Color wraps existing text; it never replaces it.** Every
`error_line`/`fail_line`/`warn_line` call site kept the exact message
text it had before this milestone, just wrapped in ANSI codes — proven
safe by the fact that every existing test asserting on that text via
`.contains(...)` still passes unchanged. The one exception is `fmt
--check`'s unformatted-file report, reworded from a bare path to "is
not formatted" — checked only by exit code and file-untouched
assertions, never by message content, so free to improve.

**`run`'s stdout is never touched.** The interpreter's own `print()`
output is the actual, meaningful output of `aint run` — several tests
assert it byte-for-byte. Every new narration line for `run`/`run --vm`
goes to stderr exclusively; nothing new is ever written to stdout by
this milestone for those two commands.

**`check`/`fmt --check`'s silence is enforced by their functions
simply never calling `step`/`ok_line`/etc. on the success path — not
by a flag threaded through the `ui` module.** There's no
`--quiet`/verbosity level to get wrong; the guarantee is structural.

## Explicitly out of scope

- **A `--quiet`/`-v`/verbosity-level flag.** Not requested, and
  `check`/`fmt --check` already cover the "I need script-safe silent
  output" case that would otherwise motivate one.
- **Progress bars, spinners, or any output that overwrites itself.**
  Genuinely more complexity (terminal-width detection, redraw timing)
  than this request called for — narration lines, not a live display.
- **Colorizing `aint run`'s own interpreter output** (a user's
  `print()` calls). That output belongs entirely to the program being
  run, not to `aint` itself.

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
