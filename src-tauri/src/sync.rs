// Sync layer: local <-> cloud (transactional outbox + background drain).
//
// WRITE PATH (in tickets.rs): every cloud-relevant mutation writes its business
// row AND a `sync_queue` row in the SAME transaction via `enqueue()`. If the
// transaction commits, the change is durably queued; if it rolls back, neither
// the row nor the queue entry exists. No lost or orphaned changes.
//
// DRAIN PATH (here): a background tokio task pops the oldest pending row, marks
// it in_flight, POSTs the payload to the cloud Axum API, and on success marks it
// done. On failure it goes back to pending with attempts+1 and exponential
// backoff. The cloud upserts by `idempotency_key`, so a retried POST never
// duplicates. On startup, any in_flight rows (from a crash mid-send) are reset
// to pending.

use serde::Serialize;
use sqlx::{Sqlite, SqlitePool, Transaction};
use std::time::Duration;
use uuid::Uuid;

/// Where the cloud Axum API lives. Overridable so the same build can point at
/// dev (localhost) or production without recompiling.
pub fn cloud_base_url() -> String {
    std::env::var("CLOUD_API_URL").unwrap_or_else(|_| "http://localhost:8080".to_string())
}

/// One queued change waiting to reach the cloud.
#[derive(Debug, sqlx::FromRow)]
struct QueueRow {
    id: i64,
    idempotency_key: String,
    entity_type: String,
    operation: String,
    payload: String,
    attempts: i64,
}

/// Insert an outbox row INSIDE the caller's transaction (transactional outbox).
///
/// `payload` is the JSON body the cloud endpoint expects. `idempotency_key` is a
/// fresh UUID the cloud will upsert by, so retries are safe.
pub async fn enqueue<T: Serialize>(
    tx: &mut Transaction<'_, Sqlite>,
    entity_type: &str,
    entity_id: i64,
    operation: &str,
    payload: &T,
) -> Result<(), sqlx::Error> {
    let body = serde_json::to_string(payload).map_err(|e| {
        // Surface JSON errors as a sqlx-compatible error so callers keep one error type.
        sqlx::Error::Protocol(format!("sync payload serialize failed: {e}"))
    })?;
    let key = Uuid::new_v4().to_string();

    sqlx::query(
        "INSERT INTO sync_queue (idempotency_key, entity_type, entity_id, operation, payload, status)
         VALUES (?, ?, ?, ?, ?, 'pending')",
    )
    .bind(&key)
    .bind(entity_type)
    .bind(entity_id)
    .bind(operation)
    .bind(&body)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Map a queued entity+operation to its cloud endpoint and HTTP method.
/// Everything funnels through upsert-style endpoints keyed by idempotency_key so
/// the cloud is safe to call repeatedly.
fn endpoint_for(entity_type: &str, _operation: &str) -> Option<(reqwest::Method, String)> {
    let base = cloud_base_url();
    match entity_type {
        // The cloud exposes a single idempotent upsert for tickets (create/update/
        // validate all carry the full intended state in the payload).
        "ticket" => Some((reqwest::Method::POST, format!("{base}/sync/tickets"))),
        _ => None,
    }
}

/// Spawn the background drain loop. Returns immediately; the task runs for the
/// lifetime of the app.
pub fn spawn_worker(pool: SqlitePool) {
    tauri::async_runtime::spawn(async move {
        // Recover from a crash that left rows mid-flight.
        if let Err(e) = reset_in_flight(&pool).await {
            eprintln!("[sync] failed to reset in_flight rows: {e}");
        }

        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(15))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[sync] could not build HTTP client: {e}");
                return;
            }
        };

        loop {
            match drain_once(&pool, &client).await {
                Ok(true) => {
                    // Did work — immediately look for the next row.
                    continue;
                }
                Ok(false) => {
                    // Queue empty — idle before polling again.
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
                Err(e) => {
                    eprintln!("[sync] drain error: {e}");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                }
            }
        }
    });
}

/// On startup, return any in_flight rows to pending so a crash mid-send retries.
async fn reset_in_flight(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE sync_queue SET status = 'pending' WHERE status = 'in_flight'")
        .execute(pool)
        .await?;
    Ok(())
}

/// Process at most one queued row. Returns Ok(true) if a row was handled,
/// Ok(false) if the queue was empty.
async fn drain_once(pool: &SqlitePool, client: &reqwest::Client) -> Result<bool, sqlx::Error> {
    // Claim the oldest pending row (FIFO) and mark it in_flight atomically.
    // Using a transaction so two workers (or a restart) can't grab the same row.
    let mut tx = pool.begin().await?;

    let row = sqlx::query_as::<_, QueueRow>(
        "SELECT id, idempotency_key, entity_type, operation, payload, attempts
         FROM sync_queue
         WHERE status = 'pending'
         ORDER BY id ASC
         LIMIT 1",
    )
    .fetch_optional(&mut *tx)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            tx.commit().await?;
            return Ok(false);
        }
    };

    sqlx::query("UPDATE sync_queue SET status = 'in_flight', last_attempt_at = datetime('now') WHERE id = ?")
        .bind(row.id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // Attempt the network call OUTSIDE any DB transaction (it can be slow).
    let result = send_to_cloud(client, &row).await;

    match result {
        Ok(()) => {
            sqlx::query("UPDATE sync_queue SET status = 'done', last_error = NULL WHERE id = ?")
                .bind(row.id)
                .execute(pool)
                .await?;
        }
        Err(err_msg) => {
            // Back to pending with attempts+1; the loop's backoff governs pacing.
            let attempts = row.attempts + 1;
            // Exponential backoff cap so a long-dead cloud doesn't hot-loop:
            // sleep grows with attempts, capped at 5 minutes.
            let backoff = backoff_secs(attempts);
            sqlx::query(
                "UPDATE sync_queue
                 SET status = 'pending', attempts = ?, last_error = ?
                 WHERE id = ?",
            )
            .bind(attempts)
            .bind(&err_msg)
            .bind(row.id)
            .execute(pool)
            .await?;
            eprintln!(
                "[sync] row {} failed (attempt {attempts}), backing off {backoff}s: {err_msg}",
                row.id
            );
            tokio::time::sleep(Duration::from_secs(backoff)).await;
        }
    }

    Ok(true)
}

/// Exponential backoff in seconds: 2,4,8,16,... capped at 300s (5 min).
fn backoff_secs(attempts: i64) -> u64 {
    let exp = attempts.clamp(1, 8) as u32; // cap the shift so it can't overflow
    (2u64.saturating_pow(exp)).min(300)
}

/// POST the payload to the matching cloud endpoint, attaching the idempotency
/// key as a header so the cloud upserts safely.
async fn send_to_cloud(client: &reqwest::Client, row: &QueueRow) -> Result<(), String> {
    let (method, url) = match endpoint_for(&row.entity_type, &row.operation) {
        Some(e) => e,
        None => return Err(format!("no cloud endpoint for entity '{}'", row.entity_type)),
    };

    // payload is already JSON; forward it verbatim with the right content type.
    let resp = client
        .request(method, &url)
        .header("Content-Type", "application/json")
        .header("Idempotency-Key", &row.idempotency_key)
        .body(row.payload.clone())
        .send()
        .await
        .map_err(|e| format!("request failed: {e}"))?;

    if resp.status().is_success() {
        Ok(())
    } else {
        let code = resp.status();
        let text = resp.text().await.unwrap_or_default();
        Err(format!("cloud returned {code}: {text}"))
    }
}
