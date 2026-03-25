mod data;
mod drawings;
mod ib_ws;

use drawings::DbPool;
use sqlx::postgres::PgPoolOptions;
use tauri::Manager;
use tauri::async_runtime;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            // PostgreSQL pool for drawings persistence
            let pool = async_runtime::block_on(async {
                let p = PgPoolOptions::new()
                    .max_connections(5)
                    .connect("postgresql://postgres:monkeyxx@192.168.1.143:5432/ococo")
                    .await
                    .expect("Failed to connect to PostgreSQL");

                // Idempotent schema migration — creates tables if missing
                drawings::ensure_schema(&p).await
                    .expect("Failed to ensure DB schema");

                p
            });
            app.manage(DbPool(pool));

            // IB WebSocket hot path — Rust-native, msgpack binary
            let ib_handle = ib_ws::spawn(app.handle().clone());
            app.manage(ib_handle);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            data::get_bars,
            data::get_options_chain,
            drawings::drawings_load_all,
            drawings::drawings_load_symbol,
            drawings::drawings_save,
            drawings::drawings_update_points,
            drawings::drawings_update_style,
            drawings::drawings_remove,
            drawings::drawings_clear,
            drawings::groups_load_all,
            drawings::groups_save,
            drawings::groups_remove,
            drawings::groups_update_style,
            ib_ws::ib_ws_send,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
