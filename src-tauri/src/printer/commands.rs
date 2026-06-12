// Tauri command entry points for the printer prototype.
//
// These are the only functions the React frontend calls (via invoke()).
// They orchestrate discovery + status + SQLite + ESC/POS + spooler, and
// return UI-friendly types.

use tauri::State;

use crate::db::AppState;
use crate::printer::{discovery, error::PrinterError, escpos, model::*, spooler, status};

/// Local timestamp "YYYY-MM-DD HH:MM:SS" via SQLite (avoids pulling a time crate
/// and matches how the backend stamps rows).
async fn now(state: &AppState) -> Result<String, PrinterError> {
    let ts: String = sqlx::query_scalar("SELECT datetime('now','localtime')")
        .fetch_one(&state.db)
        .await?;
    Ok(ts)
}

/// List printers: OS-discovered names merged with the saved default flag.
#[tauri::command]
pub async fn list_printers(state: State<'_, AppState>) -> Result<Vec<PrinterInfo>, String> {
    list_inner(&state, false).await.map_err(|e| e.to_string())
}

/// Re-scan the OS and upsert discovered printers into the DB, then return them.
#[tauri::command]
pub async fn refresh_printers(state: State<'_, AppState>) -> Result<Vec<PrinterInfo>, String> {
    list_inner(&state, true).await.map_err(|e| e.to_string())
}

async fn list_inner(state: &AppState, persist: bool) -> Result<Vec<PrinterInfo>, PrinterError> {
    let names = discovery::system_printers().await.unwrap_or_default();

    // Which name is the saved default?
    let default_name: Option<String> =
        sqlx::query_scalar("SELECT name FROM printers WHERE is_default = 1 LIMIT 1")
            .fetch_optional(&state.db)
            .await?;

    if persist {
        for name in &names {
            sqlx::query("INSERT OR IGNORE INTO printers (name, type) VALUES (?, 'thermal')")
                .bind(name)
                .execute(&state.db)
                .await?;
        }
    }

    let mut out = Vec::with_capacity(names.len());
    for name in names {
        let st = status::status_of(&name).await;
        out.push(PrinterInfo {
            is_default: default_name.as_deref() == Some(name.as_str()),
            kind: "thermal".into(),
            is_system: true,
            status: st,
            name,
        });
    }
    Ok(out)
}

/// Live status of one printer (drives the 🟢/🔴 badge).
#[tauri::command]
pub async fn get_printer_status(name: String) -> Result<PrinterStatus, String> {
    Ok(status::status_of(&name).await)
}

/// Persist the default printer (exactly one row keeps is_default = 1).
#[tauri::command]
pub async fn set_default_printer(state: State<'_, AppState>, name: String) -> Result<(), String> {
    set_default_inner(&state, &name)
        .await
        .map_err(|e| e.to_string())
}

async fn set_default_inner(state: &AppState, name: &str) -> Result<(), PrinterError> {
    // Make sure the row exists (it may not be discovered yet on a fresh DB).
    sqlx::query("INSERT OR IGNORE INTO printers (name, type) VALUES (?, 'thermal')")
        .bind(name)
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE printers SET is_default = 0")
        .execute(&state.db)
        .await?;
    sqlx::query("UPDATE printers SET is_default = 1 WHERE name = ?")
        .bind(name)
        .execute(&state.db)
        .await?;
    Ok(())
}

/// Read the saved default printer name, if any.
#[tauri::command]
pub async fn get_default_printer(state: State<'_, AppState>) -> Result<Option<String>, String> {
    sqlx::query_scalar("SELECT name FROM printers WHERE is_default = 1 LIMIT 1")
        .fetch_optional(&state.db)
        .await
        .map_err(|e| e.to_string())
}

/// Print a small "PRINTER OK" page to validate the path end-to-end.
#[tauri::command]
pub async fn test_print(state: State<'_, AppState>, name: String) -> Result<PrintResult, String> {
    let result = (|| async {
        gate(&name).await?;
        let ts = now(&state).await?;
        let bytes = escpos::build_test_page(&name, &ts);
        spooler::send_raw(&name, &bytes).await?;
        Ok::<_, PrinterError>(())
    })()
    .await;

    Ok(match result {
        Ok(()) => PrintResult::ok(format!("Test page sent to '{name}'."), None),
        Err(e) => PrintResult::err(friendly(&e)),
    })
}

/// Issue + print a new ticket, storing it locally for reprint.
/// A fresh ticket number is generated here.
#[tauri::command]
pub async fn print_ticket(
    state: State<'_, AppState>,
    name: String,
    company: String,
    footer: String,
) -> Result<PrintResult, String> {
    let result = (|| async {
        gate(&name).await?;

        // Generate a unique, human-friendly ticket number.
        let suffix = uuid::Uuid::new_v4().simple().to_string();
        let ticket_number = format!("TKT-{}", &suffix[..8].to_uppercase());
        let qr_data = ticket_number.clone();

        // Store BEFORE printing so a printer failure still leaves a record we
        // can retry/reprint, and so the number is stable.
        let ticket = sqlx::query_as::<_, Ticket>(
            "INSERT INTO printed_tickets (ticket_number, qr_data, company, footer)
             VALUES (?, ?, ?, ?)
             RETURNING id, ticket_number, qr_data, company, footer, printed_at",
        )
        .bind(&ticket_number)
        .bind(&qr_data)
        .bind(&company)
        .bind(&footer)
        .fetch_one(&state.db)
        .await?;

        let bytes = escpos::build_ticket(
            &company,
            &ticket.ticket_number,
            &ticket.printed_at,
            &ticket.qr_data,
            &footer,
        );
        spooler::send_raw(&name, &bytes).await?;
        Ok::<_, PrinterError>(ticket)
    })()
    .await;

    Ok(match result {
        Ok(t) => PrintResult::ok(format!("Ticket {} printed.", t.ticket_number), Some(t)),
        Err(e) => PrintResult::err(friendly(&e)),
    })
}

/// The most recently printed ticket (for "View Last Printed Ticket").
#[tauri::command]
pub async fn get_last_ticket(state: State<'_, AppState>) -> Result<Option<Ticket>, String> {
    sqlx::query_as::<_, Ticket>(
        "SELECT id, ticket_number, qr_data, company, footer, printed_at
         FROM printed_tickets ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())
}

/// Reprint the last ticket WITHOUT generating a new number.
#[tauri::command]
pub async fn reprint_last(state: State<'_, AppState>, name: String) -> Result<PrintResult, String> {
    let result = (|| async {
        let last = sqlx::query_as::<_, Ticket>(
            "SELECT id, ticket_number, qr_data, company, footer, printed_at
             FROM printed_tickets ORDER BY id DESC LIMIT 1",
        )
        .fetch_optional(&state.db)
        .await?;

        let ticket = last.ok_or_else(|| PrinterError::Print("No ticket to reprint yet.".into()))?;

        gate(&name).await?;
        let bytes = escpos::build_ticket(
            &ticket.company,
            &ticket.ticket_number,
            &ticket.printed_at, // keep the ORIGINAL print time on a reprint
            &ticket.qr_data,
            &ticket.footer,
        );
        spooler::send_raw(&name, &bytes).await?;
        Ok::<_, PrinterError>(ticket)
    })()
    .await;

    Ok(match result {
        Ok(t) => PrintResult::ok(format!("Reprinted ticket {}.", t.ticket_number), Some(t)),
        Err(e) => PrintResult::err(friendly(&e)),
    })
}

/// Pre-print gate: refuse to send a job to a printer that isn't online.
async fn gate(name: &str) -> Result<(), PrinterError> {
    let st = status::status_of(name).await;
    if st.connected {
        Ok(())
    } else if st.detail.to_lowercase().contains("not found") {
        Err(PrinterError::NotFound(name.to_string()))
    } else {
        Err(PrinterError::Offline(name.to_string()))
    }
}

/// Map internal errors to short, user-facing messages.
fn friendly(e: &PrinterError) -> String {
    match e {
        PrinterError::NotFound(n) => {
            format!("Printer '{n}' was not found. Check the USB cable or select another printer.")
        }
        PrinterError::Offline(n) => {
            format!("Printer '{n}' is offline. Make sure it is powered on and connected.")
        }
        PrinterError::Timeout => {
            "The print job timed out. The printer may be busy or unresponsive.".into()
        }
        PrinterError::Print(m) => format!("Printing failed: {m}"),
        PrinterError::Discovery(m) => format!("Could not list printers: {m}"),
        PrinterError::Db(m) => format!("Local database error: {m}"),
    }
}
