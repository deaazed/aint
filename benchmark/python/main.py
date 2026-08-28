"""FastAPI equivalent of ../../examples/customer_support/server.an
(milestone 25) - same routes, same behavior, Python's idiomatic stack
instead of AINT's from-scratch stdlib. See
../../docs/milestones/26-benchmark/SPEC.md for the comparison this
exists to support.
"""

import logging
import secrets
from contextlib import asynccontextmanager

import bcrypt
from fastapi import FastAPI
from fastapi.responses import JSONResponse

import ai
from db import get_connection, init_db
from models import (
    CreateTicketRequest,
    CreateTicketResponse,
    ErrorResponse,
    LoginRequest,
    LoginResponse,
    RegisterRequest,
    RegisterResponse,
    ResolveTicketRequest,
    ResolveTicketResponse,
    Ticket,
    TicketListResponse,
)

logging.basicConfig(level=logging.INFO, format="[%(asctime)s %(levelname)s] %(message)s")
logger = logging.getLogger("customer_support")


@asynccontextmanager
async def lifespan(_app: FastAPI):
    init_db()
    logger.info("customer support server starting")
    yield


app = FastAPI(title="customer-support", lifespan=lifespan)


def error(message: str, status_code: int = 400) -> JSONResponse:
    return JSONResponse(status_code=status_code, content=ErrorResponse(error=message).model_dump())


def authenticate(token: str) -> str | None:
    conn = get_connection()
    row = conn.execute("SELECT user_id FROM sessions WHERE id = ?", (token,)).fetchone()
    conn.close()
    return row["user_id"] if row else None


@app.post("/register")
def register(body: RegisterRequest):
    conn = get_connection()
    existing = conn.execute("SELECT id FROM users WHERE email = ?", (body.email,)).fetchone()
    if existing:
        conn.close()
        return error("that email is already registered")

    user_id = secrets.token_hex(24)
    password_hash = bcrypt.hashpw(body.password.encode(), bcrypt.gensalt()).decode()
    conn.execute(
        "INSERT INTO users (id, email, password_hash) VALUES (?, ?, ?)",
        (user_id, body.email, password_hash),
    )
    conn.commit()
    conn.close()
    logger.info("registered user %s", user_id)
    return RegisterResponse(user_id=user_id)


@app.post("/login")
def login(body: LoginRequest):
    conn = get_connection()
    row = conn.execute("SELECT id, password_hash FROM users WHERE email = ?", (body.email,)).fetchone()
    if row is None or not bcrypt.checkpw(body.password.encode(), row["password_hash"].encode()):
        conn.close()
        return error("invalid email or password")

    token = secrets.token_hex(24)
    conn.execute("INSERT INTO sessions (id, user_id) VALUES (?, ?)", (token, row["id"]))
    conn.commit()
    conn.close()
    logger.info("logged in user %s", row["id"])
    return LoginResponse(token=token)


@app.post("/tickets")
def create_ticket(body: CreateTicketRequest):
    user_id = authenticate(body.token)
    if user_id is None:
        return error("not authenticated")

    priority = ai.decide_priority(body.body, user_id)

    ticket_id = secrets.token_hex(24)
    conn = get_connection()
    conn.execute(
        "INSERT INTO tickets (id, user_id, subject, body, status, priority) VALUES (?, ?, ?, ?, 'open', ?)",
        (ticket_id, user_id, body.subject, body.body, priority),
    )
    if priority == "high":
        job_id = secrets.token_hex(24)
        conn.execute(
            "INSERT INTO jobs (id, ticket_id, status) VALUES (?, ?, 'pending')",
            (job_id, ticket_id),
        )
    conn.commit()
    conn.close()
    logger.info("created ticket %s", ticket_id)
    return CreateTicketResponse(ticket_id=ticket_id, priority=priority)


@app.post("/tickets/list")
def list_tickets(body: dict):
    user_id = authenticate(body.get("token", ""))
    if user_id is None:
        return error("not authenticated")

    conn = get_connection()
    rows = conn.execute("SELECT * FROM tickets WHERE user_id = ?", (user_id,)).fetchall()
    conn.close()
    return TicketListResponse(tickets=[Ticket(**dict(row)) for row in rows])


@app.post("/tickets/resolve")
def resolve_ticket(body: ResolveTicketRequest):
    user_id = authenticate(body.token)
    if user_id is None:
        return error("not authenticated")

    conn = get_connection()
    row = conn.execute("SELECT user_id FROM tickets WHERE id = ?", (body.ticket_id,)).fetchone()
    if row is None:
        conn.close()
        return error("no such ticket")
    if row["user_id"] != user_id:
        conn.close()
        return error("not your ticket")

    conn.execute("UPDATE tickets SET status = 'resolved' WHERE id = ?", (body.ticket_id,))
    conn.commit()
    conn.close()
    logger.info("resolved ticket %s", body.ticket_id)
    return ResolveTicketResponse(status="resolved")
