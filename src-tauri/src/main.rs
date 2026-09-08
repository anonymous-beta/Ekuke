#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Declare your modules
pub mod cmd;
pub mod search;
pub mod models;
pub mod utils;
// Add other modules here as you create them: pub mod db; pub mod crypto; etc.

fn main() {
    // Initialize your custom commands plugin
    let commands_plugin = cmd::init();

    tauri::Builder::default()
        .plugin(commands_plugin)
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}