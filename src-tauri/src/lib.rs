// Tauri app entry. Sets up the local SQLite store and registers the
// printer-prototype commands the React frontend calls via invoke().

mod db;
mod printer;
mod sync;
mod tickets;

use db::AppState;
use tauri::Manager;

// Kept from the template; harmless and handy as a connectivity check.
#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // Open the local DB + run migrations before the UI can call commands.
            let handle = app.handle().clone();
            let pool = tauri::async_runtime::block_on(db::init(&handle))
                .expect("failed to initialise local database");

            // Start the background sync drain: pushes queued local changes to the
            // cloud Axum API and retries with backoff while offline.
            sync::spawn_worker(pool.clone());

            app.manage(AppState { db: pool });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            printer::commands::list_printers,
            printer::commands::refresh_printers,
            printer::commands::get_printer_status,
            printer::commands::set_default_printer,
            printer::commands::get_default_printer,
            printer::commands::test_print,
            printer::commands::print_ticket,
            printer::commands::get_last_ticket,
            printer::commands::reprint_last,
            // Local ticket lifecycle (ported from the Axum backend).
            tickets::list_tickets,
            tickets::get_ticket,
            tickets::create_ticket,
            tickets::update_ticket,
            tickets::delete_ticket,
            tickets::validate_ticket,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
