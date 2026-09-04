# AINT

AINT is a programming language where deterministic computation and AI
inference are both first-class — typed, checked, and testable the
same way, not one bolted onto the other through an SDK.

```an
import string

enum Intent { Refund Support Sales Other }

tool find_customer(id: String) -> String

infer classify(message: String) -> Intent
    permissions [find_customer]

fn greeting(name: String) -> String effects [pure] {
    return string_concat("Hi ", name)
}

fn respond(message: String, customer_id: String) -> String {
    let intent = await classify(message)

    if intent == Intent_Refund {
        let customer = await find_customer(customer_id)
        return string_concat(greeting(customer), ", I'll help you process your refund.")
    } else {
        return "Could you provide more details?"
    }
}

test "a refund message gets routed to a human-sounding reply" {
    mock classify -> Intent_Refund
    mock find_customer -> "Ada"
    assert respond("I want my money back", "42") == "Hi Ada, I'll help you process your refund."
}
```

`classify` isn't a function that happens to call an LLM behind the
scenes — `infer` is its own statement, and `permissions` states,
checked at compile time and enforced again at the point of execution,
exactly which `tool` its model conversation may reach. A `pure`
function can't call either. `test`/`mock`/`assert` are real language
statements: the test above runs offline, deterministically, with no
live model — every AI-touching program in this repository is tested
this way, including a full HTTP customer-support API
(`examples/customer_support/`).

That governance — what a function can and can't touch, checked by the
compiler and enforced by the runtime, not left to convention — turned
out to be the language's actual distinguishing bet, found by
benchmarking a real application against the Python + Pydantic +
LangGraph stack that would otherwise build the same thing. See
[`docs/milestones/27-killer-abstraction/FINDINGS.md`](docs/milestones/27-killer-abstraction/FINDINGS.md)
for the reasoning and [`docs/milestones/26-benchmark/RESULTS.md`](docs/milestones/26-benchmark/RESULTS.md)
for the numbers behind it.

## Documentation

- [`docs/LANGUAGE_DESIGN.md`](docs/LANGUAGE_DESIGN.md) — the thesis,
  what AINT deliberately isn't, and design principles.
- [`docs/SPECIFICATION.md`](docs/SPECIFICATION.md) — the full language
  and runtime reference: grammar, type system, both execution engines,
  every stdlib module, the error model, and a consolidated list of
  known gaps.
- [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) — what's
  guaranteed to keep working across versions, and what isn't yet.
- [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) — compiler pipeline
  and crate layout.
- [`docs/RUNTIME.md`](docs/RUNTIME.md) — historical design notes;
  `SPECIFICATION.md` is the accurate reference.
- [`ROADMAP.md`](ROADMAP.md) — all 29 milestones, `00` through `28`,
  each with a `docs/milestones/NN-name/SPEC.md` and `ACCEPTANCE.md`
  explaining what was built and why.
- [`CONTRIBUTING.md`](CONTRIBUTING.md) — the milestone-gated workflow
  and Rust conventions this project follows.

## What's here

A tree-walking interpreter and a stack-based bytecode VM (`aint run`
vs. `aint run --vm` — the VM covers AINT's deterministic core only,
about 13x faster on compute-heavy code); a type checker with a static
effect system (`pure`/`inference`/`tool`/`network`/`filesystem`); a
package manifest and local dependency resolver (`aint init`/
`aint add`); a source formatter and `aint check` (`aint fmt`); a
stdlib covering math/string/time/collections, `Distribution<T>`/
`Option<T>` operations, and — since milestone 25 — real JSON, a
file-backed database, password hashing, logging, and a hand-rolled
HTTP/1.1 server, enough to build and run a real HTTP API entirely in
AINT (`examples/customer_support/`); since milestone 29, `import
"path" as alias` — a real AINT program can span more than one file;
since milestone 30, closures (`fn(...) -> T { ... }` as a value);
since milestone 32, `aint scaffold "description" <path>` — generates a
starter project from a plain-English description using the same model
backend `aint run` does, always checked before it's reported as done;
and since milestone 34, `tool name(params) -> Type { body }` — a real
implementation a model's tool calls actually run, not just `MockTool`.

What's deliberately not here yet: native compilation, a hosted
package registry, diamond-shared imports, and an LSP beyond syntax
highlighting — each named directly, with the reason, in
`docs/SPECIFICATION.md`'s "known gaps" and `docs/ARCHITECTURE.md`'s
"what's still deliberately not built."

## Install

No Rust toolchain needed — prebuilt binaries, fetched by a small
script (Linux and macOS):

```
curl -fsSL https://raw.githubusercontent.com/deaazed/aint/main/install.sh | sh
```

Windows:

```
irm https://raw.githubusercontent.com/deaazed/aint/main/install.ps1 | iex
```

Both install to `~/.aint/bin` (`%USERPROFILE%\.aint\bin` on Windows)
and tell you if that isn't already on `PATH`. Verify with
`aint --version`.

Already have `aint`? `aint upgrade` replaces it in place with the
latest release — `aint upgrade --check` just reports whether one's
available. Never automatic; only ever runs when you ask.

## Building from source

For contributing, or a platform the installer doesn't cover yet.
Requires a recent stable Rust toolchain.

```
cargo build
cargo test --workspace
cargo run -- run examples/hello.an
```

Or, for a release build (recommended for anything performance-
sensitive — the debug build is meaningfully slower):

```
cargo build --release
./target/release/aint run examples/showcase.an
./target/release/aint test examples/testing.an
./target/release/aint run --vm examples/fibonacci.an
```

`examples/` has more — `enums.an`, `stdlib.an`, `async.an`,
`security.an`, `closures.an`, `modularity/` (a program split across
two files), `customer_support/` (a full HTTP API: `aint run
examples/customer_support/server.an`, then `curl` it), and `router/`
(a real, importable route table built entirely out of `import` and
closures — no framework-shaped stdlib additions required: `aint run
examples/router/demo.an`, then `curl`).

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.
