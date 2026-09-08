use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use anyhow::Result;
use chrono::Local;
use serde_json::{json, Value};
use tauri::{AppHandle, State};
use tokio::sync::Mutex;

// --- AppState Definition (Move to lib.rs or state.rs if preferred) ---
pub struct AppState {
    pub global_config: Mutex<crate::config::Config>, // Ensure crate::config::Config exists or replace with a dummy struct
    pub db: Mutex<Option<Arc<crate::db::GraphDb>>>,
    pub db_path: Mutex<Option<PathBuf>>,
    pub search: Mutex<Option<Arc<Mutex<crate::search::SearchIndex>>>>,
}

// ─── Config ─────────────────────────────────────────────
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Value, String> {
    let config = state.global_config.lock().await;
    Ok(json!(config))
}

#[tauri::command]
pub async fn set_config(state: State<'_, AppState>, config: Value) -> Result<(), String> {
    let mut cfg = state.global_config.lock().await;
    // *cfg = config; // Implement proper config parsing here
    Ok(())
}

#[tauri::command]
pub async fn set_db_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let mut db_path_guard = state.db_path.lock().await;
    *db_path_guard = Some(PathBuf::from(path));
    Ok(())
}

// ─── Database ─────────────────────────────────────────
#[tauri::command]
pub async fn init_db(state: State<'_, AppState>, path: Option<String>) -> Result<String, String> {
    let path_buf = match path {
        Some(p) => PathBuf::from(p),
        None => {
            let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            p.push(".ekuke");
            p.push("db");
            p
        }
    };

    if let Some(parent) = path_buf.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Stub: Replace with actual GraphDb::new(&path_buf)
    let mut db_guard = state.db.lock().await;
    *db_guard = None; 
    
    let mut db_path_guard = state.db_path.lock().await;
    *db_path_guard = Some(path_buf.clone());

    Ok(path_buf.display().to_string())
}

#[tauri::command]
pub async fn get_db_stats(state: State<'_, AppState>) -> Result<Value, String> {
    Ok(json!({"status": "ok", "notes_count": 0}))
}

// ─── Notes / Evidence ──────────────────────────────────
#[tauri::command]
pub async fn create_note(state: State<'_, AppState>, case_id: String, title: String, content: String, tags: Option<Vec<String>>) -> Result<String, String> {
    Ok(format!("note_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_note(state: State<'_, AppState>, note_id: String) -> Result<Value, String> {
    Ok(json!({"id": note_id, "title": "Stub Note", "content": "Stub content"}))
}

#[tauri::command]
pub async fn update_note(state: State<'_, AppState>, note_id: String, title: String, content: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn delete_note(state: State<'_, AppState>, note_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn list_notes(state: State<'_, AppState>, case_id: Option<String>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

// ─── Search ────────────────────────────────────────────
#[tauri::command]
pub async fn initialize_search(state: State<'_, AppState>) -> Result<(), String> {
    let mut search_guard = state.search.lock().await;
    if search_guard.is_some() {
        return Ok(());
    }
    
    let index_path = {
        let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push(".ekuke");
        p.push("search_index");
        p
    };
    
    if let Some(parent) = index_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    
    // Stub: Replace with actual SearchIndex::new(&index_path)
    *search_guard = None;
    Ok(())
}

#[tauri::command]
pub async fn search_notes(state: State<'_, AppState>, query: String) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn search_exact(state: State<'_, AppState>, query: String) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

// ─── Case Management ──────────────────────────────────
#[tauri::command]
pub async fn create_case(state: State<'_, AppState>, name: String, description: Option<String>) -> Result<String, String> {
    Ok(format!("case_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_case(state: State<'_, AppState>, case_id: String) -> Result<Value, String> {
    Ok(json!({"id": case_id, "name": "Stub Case"}))
}

#[tauri::command]
pub async fn list_cases(state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn update_case_status(state: State<'_, AppState>, case_id: String, status: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn delete_case(state: State<'_, AppState>, case_id: String) -> Result<(), String> {
    Ok(())
}

// ─── Entities & Relations ─────────────────────────────
#[tauri::command]
pub async fn create_entity(state: State<'_, AppState>, name: String, entity_type: String) -> Result<String, String> {
    Ok(format!("entity_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_entity(state: State<'_, AppState>, entity_id: String) -> Result<Value, String> {
    Ok(json!({"id": entity_id}))
}

#[tauri::command]
pub async fn list_entities(state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn update_entity(state: State<'_, AppState>, entity_id: String, name: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn delete_entity(state: State<'_, AppState>, entity_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn create_relation(state: State<'_, AppState>, source_id: String, target_id: String, relation_type: String) -> Result<String, String> {
    Ok(format!("rel_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_relations(state: State<'_, AppState>, entity_id: String) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn delete_relation(state: State<'_, AppState>, relation_id: String) -> Result<(), String> {
    Ok(())
}

// ─── Tags ─────────────────────────────────────────────
#[tauri::command]
pub async fn add_tag(state: State<'_, AppState>, item_id: String, tag: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn remove_tag(state: State<'_, AppState>, item_id: String, tag: String) -> Result<(), String> {
    Ok(())
}

// ─── Encryption ───────────────────────────────────────
#[tauri::command]
pub async fn encrypt_text(state: State<'_, AppState>, text: String) -> Result<String, String> {
    Ok(format!("encrypted_{}", text))
}

#[tauri::command]
pub async fn decrypt_text(state: State<'_, AppState>, encrypted_text: String) -> Result<String, String> {
    Ok(encrypted_text.replace("encrypted_", ""))
}

// ─── Collection ───────────────────────────────────────
#[tauri::command]
pub async fn collect_files(state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<String>, String> {
    Ok(paths)
}

#[tauri::command]
pub async fn get_collected_files(state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

// ─── Plugins ──────────────────────────────────────────
#[tauri::command]
pub async fn load_plugin(state: State<'_, AppState>, plugin_path: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn run_plugin(state: State<'_, AppState>, plugin_name: String, args: Value) -> Result<Value, String> {
    Ok(json!({"status": "success"}))
}

// ─── Export ───────────────────────────────────────────
#[tauri::command]
pub async fn export_case(state: State<'_, AppState>, case_id: String, format: String) -> Result<String, String> {
    Ok("/path/to/exported/file".to_string())
}

// ─── System ───────────────────────────────────────────
#[tauri::command]
pub async fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub async fn get_system_info() -> Result<Value, String> {
    Ok(json!({"os": std::env::consts::OS, "arch": std::env::consts::ARCH}))
}

#[tauri::command]
pub async fn health_check() -> String {
    "ok".to_string()
}

// ─── Init Plugin ──────────────────────────────────────
pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("commands")
        .invoke_handler(tauri::generate_handler![
            get_config, set_config, set_db_path,
            init_db, get_db_stats,
            create_note, get_note, update_note, delete_note, list_notes,
            initialize_search, search_notes, search_exact,
            create_case, get_case, list_cases, update_case_status, delete_case,
            create_entity, get_entity, list_entities, update_entity, delete_entity,
            create_relation, get_relations, delete_relation,
            add_tag, remove_tag,
            encrypt_text, decrypt_text,
            collect_files, get_collected_files,
            load_plugin, list_plugins, run_plugin,
            export_case,
            get_app_version, get_system_info, health_check,
        ])
        .build()
}