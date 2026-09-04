# Milestone 42 — `aint upgrade` — acceptance

## Scope

See `SPEC.md`. A self-update subcommand, pull-based and manual only —
no automatic/background upgrading, no fleet-wide upgrade mechanism.

## Acceptance criteria

- [x] `Command::Upgrade { check: bool }` in `crates/cli/src/main.rs`,
      dispatched from `main`'s existing `match`.
- [x] `fetch_latest_tag` queries
      `https://api.github.com/repos/deaazed/aint/releases/latest`.
      `parse_semver` does a real `major.minor.patch` comparison — only
      an actually-newer release counts as an upgrade, not just an
      unequal version string. 3 new unit tests.
- [x] `platform_asset`/`platform_asset_for` mirror `install.sh`/
      `install.ps1`'s own OS/arch detection and asset naming exactly
      (`aint-windows-x86_64`, `aint-macos-x86_64`,
      `aint-macos-aarch64`, `aint-linux-x86_64`; `None` — reported as a
      clear "build from source" error, not a confusing 404 — for
      anything else, Linux/aarch64 included, matching `install.sh`'s
      own explicit rejection of it). 2 new unit tests guard against the
      two ever drifting apart.
- [x] `extract_binary` shells out to `tar` (Unix) / PowerShell's
      `Expand-Archive` (Windows) — no new archive-handling dependency.
- [x] `replace_running_binary`: a direct rename on Unix; rename-aside-
      then-rename-into-place on Windows, with best-effort cleanup of
      any leftover `.old` file (both the current one and one left by an
      interrupted previous upgrade).
- [x] `reqwest` (blocking client) added to `crates/cli`'s own
      dependencies — the same version/features `aint-runtime`'s
      `HttpModel` already pulls in async, so no new dependency joins
      the workspace's graph.
- [x] **Verified against the real, live GitHub release infrastructure,
      twice over** — not simulated:
      - `aint upgrade --check` against the actual just-built `0.2.0`
        binary correctly reported "already the latest version" once
        `v0.2.0` was actually published.
      - A genuine self-upgrade: built a binary from this exact source
        with `Cargo.toml`'s version temporarily set to `0.1.9` (there
        being no way to fetch a real pre-milestone-42 binary and have
        it use a command milestone 42 itself adds), copied it out, ran
        `aint upgrade` from it — it downloaded the real published
        `v0.2.0` Windows asset, extracted it, and replaced itself in
        place. `--version` on the resulting binary correctly reported
        `0.2.0`. The expected leftover `aint.exe.old` (Windows keeps a
        running process's own backing file locked regardless of its
        current name) was observed exactly as `SPEC.md` predicted.
      - Also caught a real bug this way, before it shipped: an
        equality-only version check offered to "upgrade" a `0.2.0` dev
        build down to `v0.1.1`, since `releases/latest` hadn't updated
        yet mid-release. Fixed by `parse_semver`'s real comparison,
        re-verified after the fix.
- [x] `README.md`'s install section gets a short "already have `aint`?"
      pointer to `aint upgrade`/`aint upgrade --check`.
- [x] `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
      and `cargo fmt --check` all clean.

## Known, honestly-stated gaps

- **No automatic/background/fleet-wide upgrading.** A deliberate
  choice — see `SPEC.md`'s "Scope."
- **No migration tooling for a future breaking change**, and no
  `aint.toml` toolchain-version field. Neither is needed yet — nothing
  through v0.2.0 has ever broken an existing program. Real, separate,
  additive work if that ever changes.
- **The very first upgrade for anyone on v0.2.0 or earlier (this
  milestone's own command didn't exist in that release — it was built
  and tagged separately, after v0.2.0 had already shipped) has to be a
  manual reinstall** (`install.sh`/`install.ps1`) — `aint upgrade` can
  only upgrade a binary that already has it. `v0.3.0` is the first
  release that actually contains it; every release from there onward
  can use `aint upgrade` itself.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope."

## Outcome

Satisfied. `aint upgrade` and `aint upgrade --check` both work,
verified against the real, live GitHub release infrastructure rather
than a mock — including one real self-replacement that took an actual
old binary to an actual new one. The process of verifying it for real
caught and fixed a genuine version-comparison bug before it ever
shipped.
