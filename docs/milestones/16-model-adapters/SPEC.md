# Milestone 16 — Model adapters

## Scope

`ROADMAP.md`:

> `Model` implementations beyond `Mock`: vLLM, OpenAI-compatible APIs,
> Ollama. Source code never names a vendor; deployment config does.

## Design decisions

**One adapter, not three.** vLLM's OpenAI-compatible server, Ollama's
OpenAI-compatible endpoint, and OpenAI itself all speak the same wire
format — `POST {base_url}/chat/completions` with a JSON body shaped
the same way. A single `HttpModel { base_url, model, api_key }`
serves all three; which one a given program talks to is entirely a
runtime configuration value (the base URL), never a type or a branch
in source code. This is what "source code never names a vendor"
concretely means here — there is no `VllmModel`/`OllamaModel` type to
even choose between.

**`HttpModel` answers questions; it does not yet request tool calls.**
This is a real, deliberate scope cut, not an oversight. Supporting
tool-calling properly means building a JSON-schema request from
`ToolSignature` and parsing `tool_calls` back out of the response —
real work with a real gap behind it: `ToolSignature.params: Vec<Type>`
carries no parameter *names* (nothing has ever needed them before —
`tool`/`infer` bodies don't exist to bind them into), so any schema
`HttpModel` sent a real model would have to invent synthetic
parameter names (`arg0`, `arg1`, ...), which is a worse tool-calling
experience than not offering it yet. `HttpModel.infer` returns a
clear `ModelError` if a tool-calling-capable request would be needed
in principle — in practice, since `available_tools` is provided but
never *sent* to the model, a real backend has no way to request a
tool call through this adapter at all; agentic testing continues to
use `MockModel`, which fully supports it.

**Structured output is prompting plus type-directed text parsing, not
each vendor's JSON-mode/structured-output feature.** Those features
exist under different names and different guarantees per vendor
(OpenAI's `response_format`, vLLM's guided decoding, Ollama's own
JSON mode) — using them would mean the "one adapter" story breaks down
into per-vendor branches, exactly what's being avoided. Instead,
`HttpModel` builds a plain-language instruction naming the expected
shape ("respond with exactly one of: Positive, Neutral, Negative" for
an `Enum` return type, "respond with exactly `true` or `false`" for
`Bool`, and so on) and parses the response text against the *declared*
return type. This is honest about being less reliable than a real
structured-output feature — and it doesn't need to be perfectly
reliable, because milestone 09's schema validation already runs on
whatever comes back and rejects anything that doesn't conform, exactly
as it does for a misconfigured `MockModel`.

**`Distribution<T>`-returning `infer` calls are rejected with a clear
error**, not attempted. Getting a real probability distribution out of
a chat completions endpoint needs either raw logprobs (not exposed
uniformly across vLLM/OpenAI-compatible/Ollama) or multiple sampled
completions aggregated into frequencies (a real, more involved
feature). Neither is "beyond Mock" scope as ROADMAP.md states it;
`MockModel` remains the only way to test `Distribution<T>`-returning
`infer` functions, same as before this milestone.

**Tested against a hand-rolled local HTTP responder, not a mocking
crate or a real vendor endpoint.** No API keys or live services are
available to build against, and none should be required to prove this
code works — the same testability principle every AI-touching
milestone since 08 has held to. A minimal `TcpListener`-based
responder (read one HTTP request, write back a canned response) is
enough to exercise the real request-building and response-parsing
code paths without adding a new test-only dependency for something
this contained.

**`HttpModel` is not wired into `aint run` as a selectable backend.**
There is no deployment-config file format in AINT yet — that's
adjacent to milestone 23's manifest work, and inventing one now, just
for this, risks getting the format wrong before the milestone that
actually needs to design it does. `aint run`/`aint test` keep
defaulting to `MockModel` exactly as before; `HttpModel` is real,
tested, and usable by anything embedding `aint-runtime` as a library
starting now, with CLI/config wiring an explicit follow-up.

## Explicitly out of scope

- Tool-calling through `HttpModel` (see above).
- `Distribution<T>` through `HttpModel` (see above).
- A deployment-config file format or `aint run` CLI wiring to select
  `HttpModel`.
- Streaming responses, retries, timeouts (milestone 17's "AI resource
  management" territory).
- Any vendor-specific feature (OpenAI function calling's parallel
  tool calls, vLLM guided decoding, Ollama's non-OpenAI-compatible
  native API).

## Outcome

To be filled in `ACCEPTANCE.md` once implemented.
