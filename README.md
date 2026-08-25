# AINT

AINT is a general-purpose programming language where deterministic
computation and probabilistic AI inference are both first-class
citizens — not one bolted onto the other through an SDK.

```an
enum Intent {
    refund
    support
    sales
    other
}

tool customer.find(id: String) -> Customer

infer classify(message: Text) -> Distribution<Intent>

fn respond(message: Text, customer_id: String) -> Text {
    let intent = classify(message)

    if intent.probability(refund) >= 0.90 {
        let customer = customer.find(customer_id)
        return "I'll help you process your refund."
    }

    return "Could you provide more details?"
}
```

`classify(message)` isn't a function that happens to call an LLM behind
the scenes — its result is typed as `Distribution<Intent>`, and the
compiler won't let you treat it as a plain `Intent` without deciding how
you're resolving the uncertainty. Tool calls are typed and validated the
same way. Tests run offline against deterministic mocks, no live model
required.

Read [`docs/LANGUAGE_DESIGN.md`](docs/LANGUAGE_DESIGN.md) for the full
thesis and what AINT deliberately isn't. `docs/ARCHITECTURE.md` and
`docs/RUNTIME.md` cover how the compiler and runtime are put together.

## Status

Early. There is a Cargo workspace and a CLI skeleton; the lexer, parser,
and interpreter don't exist yet. See [`ROADMAP.md`](ROADMAP.md) for the
full milestone plan and current progress.

## Building

Requires a recent stable Rust toolchain.

```
cargo build
cargo test
cargo run -- run examples/hello.an
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE)
or [MIT license](LICENSE-MIT) at your option.
