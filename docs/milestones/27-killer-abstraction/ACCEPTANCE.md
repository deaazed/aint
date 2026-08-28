# Milestone 27 — Find the killer abstraction — acceptance

## Scope

See `SPEC.md`. A synthesis milestone: review every prior milestone's
evidence, answer what AINT actually made dramatically easier, and
update `LANGUAGE_DESIGN.md`'s thesis if the evidence points somewhere
different from the original working hypothesis. No code changes.

## Acceptance criteria

- [x] Every milestone from 08 through 26 was reviewed against one
      question — does Python's current ecosystem already match this,
      or not — and sorted into two groups in `SPEC.md`: matched
      (typed/structured inference, typed tool schemas, vendor-neutral
      model selection, deterministic offline testing) and not matched
      (typed uncertainty, static effect checking, per-inference tool
      authorization, language-level resource budgets, automatic
      tracing).
- [x] The finding is stated as a specific, falsifiable thesis in
      `FINDINGS.md`, not a vague "AINT is good at AI stuff": AINT's
      distinguishing capability is that a program's AI surface area is
      statically checkable and runtime-enforceable
      (`effects`/`permissions`/`budget`), not that individual AI
      operations are easier to write (where Python's ecosystem has
      substantially caught up, per milestone 26's own numbers).
- [x] The finding is honest about its own biggest weakness: nothing in
      `examples/customer_support/` actually exercises the governance
      claim it's built on (no meaningful `effects [pure]`, no
      `permissions` restriction, no `budget`) — stated directly in
      `FINDINGS.md`, not glossed over.
- [x] `docs/LANGUAGE_DESIGN.md`'s thesis section was revised, not
      silently replaced: the original working hypothesis is preserved
      verbatim under "Historical context," with the revision and its
      reasoning stated above it, per `ROADMAP.md`'s own instruction to
      replace the working hypothesis "if it turns out to be
      different" — traceable, not rewritten as if it had always said
      this.
- [x] `docs/LANGUAGE_DESIGN.md`'s closing "How to know if this is
      working" section was updated from an open question ("if AINT
      isn't clearly better... milestone 27 exists to go find it") to
      a reported result, citing the actual milestone 26 numbers and
      the milestone 27 conclusion they led to.
- [x] The document's design principles, effects/budget/testability
      sections, and "what AINT is not" list were left unchanged —
      confirmed consistent with the revised thesis, not contradicted
      by it, so nothing else needed to move.

## Explicitly out of scope

See `SPEC.md`'s "Explicitly out of scope" — no new code, no
re-running milestone 26's benchmark, no decision on `agent` becoming a
keyword.

## Outcome

Satisfied by `docs/milestones/27-killer-abstraction/SPEC.md`
(methodology and evidence review), `FINDINGS.md` (the thesis and its
reasoning), and the revision to `docs/LANGUAGE_DESIGN.md` (the thesis
section and the closing section), both keeping the original working
hypothesis visible as history rather than erasing it.
