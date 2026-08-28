"""SQLite-backed storage for the customer-support API.

Deliberately using Python's batteries-included `sqlite3` rather than
hand-rolling a file format the way AINT's `db` stdlib module had to
(see ../../docs/milestones/25-real-application/SPEC.md) - that's
exactly the kind of "what does the ecosystem give you for free"
contrast milestone 26 is measuring.
"""

import sqlite3
from pathlib import Path

DB_PATH = Path(__file__).parent / "customer_support.db"


def get_connection() -> sqlite3.Connection:
    conn = sqlite3.connect(DB_PATH)
    conn.row_factory = sqlite3.Row
    return conn


def init_db() -> None:
    conn = get_connection()
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS users (
            id TEXT PRIMARY KEY,
            email TEXT UNIQUE NOT NULL,
            password_hash TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sessions (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS tickets (
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            subject TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            priority TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS jobs (
            id TEXT PRIMARY KEY,
            ticket_id TEXT NOT NULL,
            status TEXT NOT NULL
        );
        """
    )
    conn.commit()
    conn.close()
