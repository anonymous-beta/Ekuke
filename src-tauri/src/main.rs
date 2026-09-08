#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

pub mod cmd;
pub mod search;
pub mod models;
pub mod utils;

fn main() {
    // Initialize the custom commands plugin
    let commands_plugin = cmd::init();

    tauri::Builder::default()
        // Manage the AppState so it can be injected into commands
        .manage(cmd::AppState {
            global_config: tokio::sync::Mutex::new(serde_json::json!({})),
            db: tokio::sync::Mutex::new(None),
            db_path: tokio::sync::Mutex::new(None),
            search: tokio::sync::Mutex::new(None),
        })
        .plugin(commands_plugin)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}