// Local ticket lifecycle — ported from ticketing-backend/src/tickets.rs.
//
// Same SQL and same validation rules; only the transport changed: Axum
// extractors (State/Json/Path) become Tauri command params, and every handler
// returns Result<T, String> so the React frontend can catch and display errors
// exactly as it did over HTTP.
//
// Section C will extend create/update/validate to also enqueue a sync_queue row
// in the SAME transaction (transactional outbox). For now these are pure-local.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tauri::State;
use uuid::Uuid;

use crate::db::AppState;
use crate::sync;

#[derive(Serialize, Deserialize, sqlx::FromRow)]
pub struct Ticket {
    pub id: i64,
    pub ticket_code: String,
    pub invoice_id: Option<i64>,
    pub valid_date: String,
    pub status: String,
    pub used_at: Option<String>,
    pub created_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub reason: String,
    pub ticket_code: Option<String>,
    pub valid_date: Option<String>,
}

/// GET /tickets  ->  list_tickets
#[tauri::command]
pub async fn list_tickets(state: State<'_, AppState>) -> Result<Vec<Ticket>, String> {
    sqlx::query_as::<_, Ticket>(
        "SELECT id, ticket_code, invoice_id, valid_date, status, used_at, created_at
         FROM tickets
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| e.to_string())
}

/// GET /tickets/{id}  ->  get_ticket
#[tauri::command]
pub async fn get_ticket(state: State<'_, AppState>, id: i64) -> Result<Ticket, String> {
    sqlx::query_as::<_, Ticket>(
        "SELECT id, ticket_code, invoice_id, valid_date, status, used_at, created_at
         FROM tickets WHERE id = ?",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Ticket not found".to_string())
}

/// POST /tickets  ->  create_ticket
/// Generates a unique TKT-<uuid> code, same as the cloud handler.
#[tauri::command]
pub async fn create_ticket(
    state: State<'_, AppState>,
    valid_date: String,
    invoice_id: Option<i64>,
) -> Result<Ticket, String> {
    let code = format!("TKT-{}", Uuid::new_v4().simple());

    // Business row + outbox row in ONE transaction (transactional outbox).
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;

    let ticket = sqlx::query_as::<_, Ticket>(
        "INSERT INTO tickets (ticket_code, invoice_id, valid_date, status)
         VALUES (?, ?, ?, 'active')
         RETURNING id, ticket_code, invoice_id, valid_date, status, used_at, created_at",
    )
    .bind(&code)
    .bind(invoice_id)
    .bind(&valid_date)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| e.to_string())?;

    sync::enqueue(&mut tx, "ticket", ticket.id, "create", &ticket)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(ticket)
}

/// PUT /tickets/{id}  ->  update_ticket
/// COALESCE keeps existing values when a field is not supplied; used_at is set
/// automatically when status flips to 'used'. Identical to the cloud handler.
#[tauri::command]
pub async fn update_ticket(
    state: State<'_, AppState>,
    id: i64,
    status: Option<String>,
    valid_date: Option<String>,
) -> Result<Ticket, String> {
    let mut tx = state.db.begin().await.map_err(|e| e.to_string())?;

    let ticket = sqlx::query_as::<_, Ticket>(
        "UPDATE tickets
         SET status     = COALESCE(?, status),
             valid_date = COALESCE(?, valid_date),
             used_at    = CASE WHEN ? = 'used' THEN datetime('now') ELSE NULL END
         WHERE id = ?
         RETURNING id, ticket_code, invoice_id, valid_date, status, used_at, created_at",
    )
    .bind(&status)
    .bind(&valid_date)
    .bind(&status)
    .bind(id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Ticket not found".to_string())?;

    sync::enqueue(&mut tx, "ticket", ticket.id, "update", &ticket)
        .await
        .map_err(|e| e.to_string())?;

    tx.commit().await.map_err(|e| e.to_string())?;
    Ok(ticket)
}

/// DELETE /tickets/{id}  ->  delete_ticket
#[tauri::command]
pub async fn delete_ticket(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let res = sqlx::query("DELETE FROM tickets WHERE id = ?")
        .bind(id)
        .execute(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    if res.rows_affected() == 0 {
        return Err("Ticket not found".to_string());
    }
    Ok(())
}

/// POST /validate  ->  validate_ticket
/// The "entry" check used by the scanner. Same branch logic as the cloud:
/// not found / cancelled / already used / not valid today -> {valid:false};
/// otherwise mark used and grant entry.
#[tauri::command]
pub async fn validate_ticket(
    state: State<'_, AppState>,
    ticket_code: String,
) -> Result<ValidationResult, String> {
    validate_inner(&state.db, &ticket_code)
        .await
        .map_err(|e| e.to_string())
}

async fn validate_inner(db: &SqlitePool, ticket_code: &str) -> Result<ValidationResult, sqlx::Error> {
    let ticket = sqlx::query_as::<_, Ticket>(
        "SELECT id, ticket_code, invoice_id, valid_date, status, used_at, created_at
         FROM tickets WHERE ticket_code = ?",
    )
    .bind(ticket_code)
    .fetch_optional(db)
    .await?;

    let t = match ticket {
        Some(t) => t,
        None => {
            return Ok(ValidationResult {
                valid: false,
                reason: "Ticket not found".into(),
                ticket_code: None,
                valid_date: None,
            })
        }
    };

    let today: String = sqlx::query_scalar("SELECT date('now','localtime')")
        .fetch_one(db)
        .await?;

    let reason: Option<String> = if t.status == "cancelled" {
        Some("Ticket has been cancelled".into())
    } else if t.status == "used" {
        Some(format!(
            "Already used at {}",
            t.used_at.clone().unwrap_or_default()
        ))
    } else if t.valid_date != today {
        Some(format!("Not valid today (valid on {})", t.valid_date))
    } else {
        None
    };

    if let Some(reason) = reason {
        return Ok(ValidationResult {
            valid: false,
            reason,
            ticket_code: Some(t.ticket_code),
            valid_date: Some(t.valid_date),
        });
    }

    // Valid — mark used so it can't enter twice. The mutation + its outbox row
    // go in ONE transaction, and we re-read the row so the synced payload
    // carries the final 'used' state (status + used_at).
    let mut tx = db.begin().await?;

    let used = sqlx::query_as::<_, Ticket>(
        "UPDATE tickets SET status = 'used', used_at = datetime('now')
         WHERE id = ?
         RETURNING id, ticket_code, invoice_id, valid_date, status, used_at, created_at",
    )
    .bind(t.id)
    .fetch_one(&mut *tx)
    .await?;

    sync::enqueue(&mut tx, "ticket", used.id, "validate", &used).await?;

    tx.commit().await?;

    Ok(ValidationResult {
        valid: true,
        reason: "Entry granted".into(),
        ticket_code: Some(used.ticket_code),
        valid_date: Some(used.valid_date),
    })
}
