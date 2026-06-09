-- Phase 1 (printer prototype) local schema.
-- This DB lives inside the desktop app and is independent of the Axum backend.

-- Known printers and which one is the saved default.
CREATE TABLE IF NOT EXISTS printers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL UNIQUE,          -- system printer name (CUPS queue / Windows printer name)
    type        TEXT NOT NULL DEFAULT 'thermal',
    is_default  INTEGER NOT NULL DEFAULT 0     -- 0/1; at most one row should be 1
);

-- Locally printed tickets, kept so we can reprint the last one
-- WITHOUT generating a new ticket number.
CREATE TABLE IF NOT EXISTS tickets (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    ticket_number TEXT NOT NULL UNIQUE,
    qr_data       TEXT NOT NULL,               -- payload encoded in the QR (defaults to ticket_number)
    company       TEXT NOT NULL DEFAULT '',    -- company/event name shown on the ticket
    footer        TEXT NOT NULL DEFAULT '',    -- footer line shown on the ticket
    printed_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
