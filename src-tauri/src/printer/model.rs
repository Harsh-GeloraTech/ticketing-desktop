// Shared printer types crossing the Rust <-> React boundary.
// All are serde-serializable so they map cleanly to TypeScript.

use serde::{Deserialize, Serialize};

/// A printer the app knows about: discovered from the OS and/or saved in SQLite.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrinterInfo {
    pub name: String,
    /// "thermal" for the prototype; left as a field so future code can classify.
    pub kind: String,
    /// True when this is the saved default in our DB.
    pub is_default: bool,
    /// True when the OS reports the device present (installed/known).
    pub is_system: bool,
    /// Live connection state at discovery time.
    pub status: PrinterStatus,
}

/// Connection/online state of a single printer.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PrinterStatus {
    /// 🟢 true = reachable/online, 🔴 false = disconnected/offline/unknown.
    pub connected: bool,
    /// Human-readable detail ("Idle", "Powered off / offline", "Not found", …).
    pub detail: String,
}

impl PrinterStatus {
    pub fn connected(detail: impl Into<String>) -> Self {
        Self {
            connected: true,
            detail: detail.into(),
        }
    }
    pub fn disconnected(detail: impl Into<String>) -> Self {
        Self {
            connected: false,
            detail: detail.into(),
        }
    }
}

/// A locally-stored printed ticket (for "view last" / "reprint").
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Ticket {
    pub id: i64,
    pub ticket_number: String,
    pub qr_data: String,
    pub company: String,
    pub footer: String,
    pub printed_at: String,
}

/// Result of a print attempt — the UI turns this into a success/error message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintResult {
    pub ok: bool,
    pub message: String,
    /// Present when a ticket row was created/used for this print.
    pub ticket: Option<Ticket>,
}

impl PrintResult {
    pub fn ok(message: impl Into<String>, ticket: Option<Ticket>) -> Self {
        Self {
            ok: true,
            message: message.into(),
            ticket,
        }
    }
    pub fn err(message: impl Into<String>) -> Self {
        Self {
            ok: false,
            message: message.into(),
            ticket: None,
        }
    }
}
