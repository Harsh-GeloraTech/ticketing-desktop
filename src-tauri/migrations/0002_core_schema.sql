-- Section A: bring the local DB up to the full app schema.
--
-- The printer prototype (migration 0001) created a `tickets` table for storing
-- printed slips. That name now belongs to the ported backend ticket lifecycle,
-- so the printer table is renamed to `printed_tickets` first. The printer Rust
-- commands are updated to match.
ALTER TABLE tickets RENAME TO printed_tickets;

-- ---------------------------------------------------------------------------
-- Local mirror of the cloud schema (kept byte-for-byte equivalent to
-- ticketing-backend/migrations/20260607102004_init_schema.sql).
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS users (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    username      TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL DEFAULT 'operator',  -- admin | manager | operator
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS invoices (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    invoice_no    TEXT NOT NULL UNIQUE,
    customer_name TEXT,
    total         REAL NOT NULL DEFAULT 0,
    created_by    INTEGER REFERENCES users(id),
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS tickets (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    ticket_code TEXT NOT NULL UNIQUE,
    invoice_id  INTEGER REFERENCES invoices(id),
    valid_date  TEXT NOT NULL,                    -- the date this ticket is valid for
    status      TEXT NOT NULL DEFAULT 'active',   -- active | used | cancelled
    used_at     TEXT,
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- ---------------------------------------------------------------------------
-- Section C: transactional outbox. Every local mutation that must reach the
-- cloud writes a row here in the SAME transaction as the business change.
-- A background task drains it FIFO and POSTs to the cloud, retrying on failure.
-- ---------------------------------------------------------------------------
CREATE TABLE IF NOT EXISTS sync_queue (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    idempotency_key TEXT NOT NULL UNIQUE,          -- UUID; cloud upserts by this so retries don't duplicate
    entity_type     TEXT NOT NULL,                 -- 'ticket' | 'invoice' | 'user' | ...
    entity_id       INTEGER,                       -- local row id this change refers to
    operation       TEXT NOT NULL,                 -- 'create' | 'update' | 'validate' | 'delete'
    payload         TEXT NOT NULL,                 -- JSON body to POST to the cloud
    status          TEXT NOT NULL DEFAULT 'pending', -- pending | in_flight | done | failed
    attempts        INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    created_at      TEXT NOT NULL DEFAULT (datetime('now')),
    last_attempt_at TEXT
);

-- FIFO drain by oldest-pending-first.
CREATE INDEX IF NOT EXISTS idx_sync_queue_status_id ON sync_queue (status, id);
