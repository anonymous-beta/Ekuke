pub mod ai;
pub mod case;
pub mod cmd;
pub mod collect;
pub mod config;
pub mod crypto;
pub mod db;
pub mod entity;
pub mod models;
pub mod plugin;
pub mod search;
pub mod utils;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use case::CaseMetadata;
use config::Config;
use db::GraphDb;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Mutex<Config>>,
    pub case_config: Arc<Mutex<Option<CaseMetadata>>>,
    pub db_path: Arc<Mutex<Option<PathBuf>>>,
    pub db: Arc<Mutex<Option<Arc<GraphDb>>>>,
}

pub fn run() {
    let config = Config::load_or_default();
    std::fs::create_dir_all(&config.cases_dir).ok();
    std::fs::create_dir_all(&config.plugins_dir).ok();

    let state = AppState {
        config: Arc::new(Mutex::new(config.clone())),
        case_config: Arc::new(Mutex::new(None)),
        db_path: Arc::new(Mutex::new(None)),
        db: Arc::new(Mutex::new(None)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            cmd::get_config,
            cmd::set_config,
            cmd::create_case,
            cmd::list_cases,
            cmd::open_case,
            cmd::delete_case,
            cmd::get_current_case,
            cmd::get_entities,
            cmd::get_entity_by_id,
            cmd::create_entity,
            cmd::update_entity,
            cmd::delete_entity,
            cmd::create_relationship,
            cmd::get_relationships,
            cmd::search_entities,
            cmd::get_plugins,
            cmd::run_transform,
            cmd::extract_entities_from_text,
            cmd::export_case,
            cmd::import_case,
            cmd::ai_chat,
            cmd::ai_test_connection,
            cmd::health_check,
            cmd::get_app_version,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}