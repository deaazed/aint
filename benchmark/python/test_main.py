"""pytest suite mirroring milestone 25's AINT test coverage:
../../crates/cli/tests/customer_support.rs (register/login/list, live
HTTP) and ../../examples/customer_support/priority_logic_test.an
(the infer+tool priority decision, via `aint test`/`mock`).

The comparison point for milestone 26: FastAPI's `TestClient` and
pytest's `monkeypatch` fixture give the same "run it without a live
model" testability AINT's `mock`/`MockModel`/`MockTool`
(milestones 08/11/15) provide as first-class language features - the
mechanism is a general-purpose testing idiom here, not something the
framework or language built specifically for AI-touching code.
"""

import uuid
from pathlib import Path

import pytest
from fastapi.testclient import TestClient

import ai
import db
import main
from models import Sentiment


@pytest.fixture(autouse=True)
def isolated_db(monkeypatch, tmp_path):
    """A fresh SQLite file per test, mirroring the scratch-directory-
    per-test isolation in aint-runtime's db.rs test suite."""
    db_path = tmp_path / f"test_{uuid.uuid4().hex}.db"
    monkeypatch.setattr(db, "DB_PATH", db_path)
    db.init_db()
    yield


@pytest.fixture
def client():
    return TestClient(main.app)


def test_register_then_duplicate_is_rejected(client):
    response = client.post("/register", json={"email": "ada@example.com", "password": "hunter2"})
    assert response.status_code == 200
    assert "user_id" in response.json()

    duplicate = client.post("/register", json={"email": "ada@example.com", "password": "hunter2"})
    assert "already registered" in duplicate.json()["error"]


def test_login_with_wrong_password_is_rejected(client):
    client.post("/register", json={"email": "ada@example.com", "password": "hunter2"})
    response = client.post("/login", json={"email": "ada@example.com", "password": "wrong"})
    assert "invalid email or password" in response.json()["error"]


def test_login_succeeds_with_the_right_password(client):
    client.post("/register", json={"email": "ada@example.com", "password": "hunter2"})
    response = client.post("/login", json={"email": "ada@example.com", "password": "hunter2"})
    assert response.status_code == 200
    assert "token" in response.json()


def test_list_tickets_requires_authentication(client):
    response = client.post("/tickets/list", json={"token": "not-a-real-token"})
    assert "not authenticated" in response.json()["error"]


def test_list_tickets_is_empty_for_a_new_session(client):
    client.post("/register", json={"email": "ada@example.com", "password": "hunter2"})
    login = client.post("/login", json={"email": "ada@example.com", "password": "hunter2"})
    token = login.json()["token"]

    response = client.post("/tickets/list", json={"token": token})
    assert response.json() == {"tickets": []}


# --- the AI-driven priority decision (mirrors priority_logic_test.an) -----


def test_negative_ticket_from_a_premium_customer_is_high_priority(monkeypatch):
    monkeypatch.setattr(ai, "classify_sentiment", lambda body: Sentiment.negative)
    monkeypatch.setattr(ai, "lookup_account_tier", lambda user_id: "premium")
    ai._graph = None
    assert ai.decide_priority("everything is on fire", "user-1") == "high"


def test_negative_ticket_from_a_standard_customer_stays_normal_priority(monkeypatch):
    monkeypatch.setattr(ai, "classify_sentiment", lambda body: Sentiment.negative)
    monkeypatch.setattr(ai, "lookup_account_tier", lambda user_id: "standard")
    ai._graph = None
    assert ai.decide_priority("small issue", "user-2") == "normal"


def test_positive_ticket_never_calls_the_tool_and_stays_normal_priority(monkeypatch):
    def fail_if_called(user_id):
        raise AssertionError("lookup_account_tier should not be called for positive sentiment")

    monkeypatch.setattr(ai, "classify_sentiment", lambda body: Sentiment.positive)
    monkeypatch.setattr(ai, "lookup_account_tier", fail_if_called)
    ai._graph = None
    assert ai.decide_priority("thanks, great job", "user-3") == "normal"


def test_neutral_ticket_never_calls_the_tool_and_stays_normal_priority(monkeypatch):
    def fail_if_called(user_id):
        raise AssertionError("lookup_account_tier should not be called for neutral sentiment")

    monkeypatch.setattr(ai, "classify_sentiment", lambda body: Sentiment.neutral)
    monkeypatch.setattr(ai, "lookup_account_tier", fail_if_called)
    ai._graph = None
    assert ai.decide_priority("just checking in", "user-4") == "normal"


# --- full flow, mocked AI --------------------------------------------------


def test_full_ticket_lifecycle_create_list_resolve(client, monkeypatch):
    monkeypatch.setattr(ai, "classify_sentiment", lambda body: Sentiment.negative)
    monkeypatch.setattr(ai, "lookup_account_tier", lambda user_id: "premium")
    ai._graph = None

    client.post("/register", json={"email": "ada@example.com", "password": "hunter2"})
    login = client.post("/login", json={"email": "ada@example.com", "password": "hunter2"})
    token = login.json()["token"]

    created = client.post(
        "/tickets",
        json={"token": token, "subject": "help", "body": "my app is broken"},
    )
    assert created.status_code == 200
    assert created.json()["priority"] == "high"
    ticket_id = created.json()["ticket_id"]

    listed = client.post("/tickets/list", json={"token": token})
    assert len(listed.json()["tickets"]) == 1
    assert listed.json()["tickets"][0]["id"] == ticket_id

    resolved = client.post("/tickets/resolve", json={"token": token, "ticket_id": ticket_id})
    assert resolved.json()["status"] == "resolved"
