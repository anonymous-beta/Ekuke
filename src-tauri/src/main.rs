#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::Arc;
use tokio::sync::Mutex;
use ekuke::{AppState, config::Config};

fn main() {
    let state = AppState {
        db_path: Arc::new(Mutex::new(None)),
        case_config: Arc::new(Mutex::new(None)),
        global_config: Arc::new(Mutex::new(Config::load_or_default())),
        db: Arc::new(Mutex::new(None)),
        search: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            ekuke::cmd::create_case,
            ekuke::cmd::open_case,
            ekuke::cmd::save_case,
            ekuke::cmd::close_case,
            ekuke::cmd::export_case,
            ekuke::cmd::import_case,
            ekuke::cmd::add_entity,
            ekuke::cmd::get_entities,
            ekuke::cmd::get_entity_by_id,
            ekuke::cmd::search_entities,
            ekuke::cmd::add_relationship,
            ekuke::cmd::get_relationships,
            ekuke::cmd::delete_entity,
            ekuke::cmd::update_entity,
            ekuke::cmd::run_transform,
            ekuke::cmd::get_plugins,
            ekuke::cmd::extract_entities_from_text,
            ekuke::cmd::get_case_info,
            ekuke::cmd::get_config,
            ekuke::cmd::set_config,
        ])
        .run(tauri::generate_context!())
        .expect("EKUKE failed to launch");
}
