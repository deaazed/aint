# Python comparison stack (milestone 26)

The Python + FastAPI + Pydantic + LangGraph equivalent of
`../../examples/customer_support/`, built to measure against — see
`../../docs/milestones/26-benchmark/SPEC.md` for methodology and
`RESULTS.md` in the same directory for the numbers.

Same routes, same behavior as `server.an`/`worker.an`:
`/register`, `/login`, `/tickets`, `/tickets/list`,
`/tickets/resolve`, plus a background job drainer.

## Running it

```
pip install -r requirements.txt
python -m uvicorn main:app --port 8080
```

In another terminal: `python worker.py` for the background job
processor (a genuine perpetual poll loop — see `worker.py`'s own doc
comment for why this doesn't need the workaround
`examples/customer_support/worker.an` does).

Sentiment classification needs a real `OPENAI_API_KEY` in the
environment to call a real model, the same way `server.an` needs
`AINT_MODEL_URL` set — see
`../../docs/milestones/25-real-application/SPEC.md`.

## Testing it

```
pip install -r requirements.txt
python -m pytest -v
```

10 cases in `test_main.py`, covering the same scenarios as
`examples/customer_support/priority_logic_test.an` (via
`aint test`) plus the full HTTP lifecycle (via FastAPI's
`TestClient`), in one suite.
