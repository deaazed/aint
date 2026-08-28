"""Background job processor - the Python equivalent of
../../examples/customer_support/worker.an.

Unlike the AINT version, this is a genuinely perpetual poll loop -
Python has real loops and no call-stack-depth concern from running
one, so there's no need for the "drain once, let the OS re-invoke you"
workaround AINT's version needs (see
../../docs/milestones/25-real-application/SPEC.md's "What building
this actually found"). That contrast is itself one of the things
milestone 26 is measuring.
"""

import logging
import time

from db import get_connection

logging.basicConfig(level=logging.INFO, format="[%(asctime)s %(levelname)s] %(message)s")
logger = logging.getLogger("worker")

POLL_INTERVAL_SECONDS = 5


def process_pending() -> int:
    conn = get_connection()
    pending = conn.execute("SELECT id, ticket_id FROM jobs WHERE status = 'pending'").fetchall()
    for job in pending:
        logger.info("sending follow-up email for ticket %s", job["ticket_id"])
        conn.execute("UPDATE jobs SET status = 'processed' WHERE id = ?", (job["id"],))
    conn.commit()
    conn.close()
    return len(pending)


def run_forever() -> None:
    while True:
        processed = process_pending()
        if processed:
            logger.info("processed %d jobs", processed)
        time.sleep(POLL_INTERVAL_SECONDS)


if __name__ == "__main__":
    run_forever()
