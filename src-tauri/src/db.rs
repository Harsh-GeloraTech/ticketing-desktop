// Local SQLite store for the printer prototype.
//
// This DB is independent of the Axum backend — Phase 1 runs standalone.
// It lives in the OS app-data directory so it survives reinstalls of the
// frontend assets and isn't tied to the process working directory.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;
use tauri::{AppHandle, Manager};

/// Shared application state held by Tauri and injected into commands.
#[derive(Clone)]
pub struct AppState {
    pub db: SqlitePool,
}

/// Resolve the on-disk path for the local SQLite file (…/app_data_dir/ticketing-prototype.db),
/// creating the parent directory if needed.
fn db_path(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("cannot resolve app data dir: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create app data dir: {e}"))?;
    Ok(dir.join("ticketing-prototype.db"))
}

/// Open (creating if absent) the local DB, enable WAL, and run migrations.
pub async fn init(app: &AppHandle) -> Result<SqlitePool, String> {
    let path = db_path(app)?;

    let options = SqliteConnectOptions::from_str(&format!("sqlite://{}", path.display()))
        .map_err(|e| format!("bad sqlite url: {e}"))?
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(4)
        .connect_with(options)
        .await
        .map_err(|e| format!("cannot open local db: {e}"))?;

    // WAL: keeps reads (status polling) from blocking writes (print logging).
    sqlx::query("PRAGMA journal_mode=WAL;")
        .execute(&pool)
        .await
        .map_err(|e| format!("pragma failed: {e}"))?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(|e| format!("migration failed: {e}"))?;

    Ok(pool)
}
