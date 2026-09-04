//! Small, ASCII-only colored output helpers for every `aint` command
//! (milestone 43) — see `main.rs`'s own module doc comment for the
//! overall convention. `anstream` decides on its own whether color is
//! actually appropriate (a real terminal, `NO_COLOR` unset) and
//! strips the ANSI codes otherwise — every function here is safe to
//! call unconditionally.

use anstyle::{AnsiColor, Color, Style};

const ERROR: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Red)))
    .bold();
const SUCCESS: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Green)))
    .bold();
const STEP: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Cyan)))
    .bold();
const WARN: Style = Style::new()
    .fg_color(Some(Color::Ansi(AnsiColor::Yellow)))
    .bold();

/// A step about to run — `==> <text>`, cyan, to stderr. Narration
/// only, never the sole source of truth for whether something
/// succeeded — and never called from `aint check`/`aint fmt --check`'s
/// success path. Both are documented as silent on success, matching
/// `gofmt -l`/`tsc --noEmit`'s convention that scripts and CI can rely
/// on empty output meaning "fine" — printing a step before knowing
/// the outcome would break that on its own, even with nothing else
/// added. Every call site earns this by inspection, not by a flag:
/// `check`/`fmt --check`'s own functions simply never call it.
pub fn step(text: impl std::fmt::Display) {
    anstream::eprintln!("{STEP}==>{STEP:#} {text}");
}

/// A successful outcome — green, to stdout, the same stream these
/// confirmations already printed to before this milestone.
/// Deliberately untouched by this whole module: `run`'s interpreter
/// output and `test`'s per-test/summary lines carry meaning of their
/// own (exact-matched by several tests, for `run`) — colored directly
/// at their own call sites instead of routed through here, so this
/// function is only ever a command's own final "done" line.
pub fn ok_line(text: impl std::fmt::Display) {
    anstream::println!("{SUCCESS}{text}{SUCCESS:#}");
}

/// Worth flagging, not necessarily a hard failure — yellow, to
/// stderr, the same diagnostic stream `error_line` uses (a warning is
/// a diagnostic about the run, not part of a command's own output —
/// `aint scaffold`'s "does not type-check" warning is checked there
/// specifically by a real test).
pub fn warn_line(text: impl std::fmt::Display) {
    anstream::eprintln!("{WARN}{text}{WARN:#}");
}

/// An error — red, to stderr. A drop-in colorized replacement for
/// every `eprintln!("error: ...")` (or similarly-shaped) line this
/// crate used before this milestone; the message text itself is never
/// changed, only wrapped in color, so nothing that matches on it
/// (several tests do, via `.contains(...)`) breaks.
pub fn error_line(text: impl std::fmt::Display) {
    anstream::eprintln!("{ERROR}{text}{ERROR:#}");
}

/// A failure — red, to *stdout*. `aint test`'s per-test `FAILED` line
/// is the one place a red message belongs on stdout rather than
/// stderr: it's part of the same result stream as the "ok" lines next
/// to it (several tests check for it there specifically, via
/// `stdout.contains("FAILED")`), not a separate error channel.
pub fn fail_line(text: impl std::fmt::Display) {
    anstream::println!("{ERROR}{text}{ERROR:#}");
}
