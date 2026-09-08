use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use serde_json::{json, Value};
use tauri::State;
use tokio::sync::Mutex;

// --- AppState Definition ---
pub struct AppState {
    pub global_config: Mutex<crate::config::Config>, 
    pub db: Mutex<Option<Arc<crate::db::GraphDb>>>,
    pub db_path: Mutex<Option<PathBuf>>,
    pub search: Mutex<Option<Arc<Mutex<crate::search::SearchIndex>>>>,
}

// ─── Config ─────────────────────────────────────────────
#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Value, String> {
    let config = state.global_config.lock().await;
    // FIX: Dereference the MutexGuard before serializing
    Ok(json!(&*config))
}

#[tauri::command]
pub async fn set_config(_state: State<'_, AppState>, _config: Value) -> Result<(), String> {
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

    let mut db_guard = state.db.lock().await;
    *db_guard = None; 
    
    let mut db_path_guard = state.db_path.lock().await;
    *db_path_guard = Some(path_buf.clone());

    Ok(path_buf.display().to_string())
}

#[tauri::command]
pub async fn get_db_stats(_state: State<'_, AppState>) -> Result<Value, String> {
    Ok(json!({"status": "ok", "notes_count": 0}))
}

// ─── Notes / Evidence ──────────────────────────────────
#[tauri::command]
pub async fn create_note(_state: State<'_, AppState>, _case_id: String, _title: String, _content: String, _tags: Option<Vec<String>>) -> Result<String, String> {
    Ok(format!("note_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_note(_state: State<'_, AppState>, _note_id: String) -> Result<Value, String> {
    Ok(json!({"id": _note_id, "title": "Stub Note", "content": "Stub content"}))
}

#[tauri::command]
pub async fn update_note(_state: State<'_, AppState>, _note_id: String, _title: String, _content: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn delete_note(_state: State<'_, AppState>, _note_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn list_notes(_state: State<'_, AppState>, _case_id: Option<String>) -> Result<Vec<Value>, String> {
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
    
    *search_guard = None;
    Ok(())
}

#[tauri::command]
pub async fn search_notes(_state: State<'_, AppState>, _query: String) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn search_exact(_state: State<'_, AppState>, _query: String) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

// ─── Case Management ──────────────────────────────────
#[tauri::command]
pub async fn create_case(_state: State<'_, AppState>, _name: String, _description: Option<String>) -> Result<String, String> {
    Ok(format!("case_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_case(_state: State<'_, AppState>, _case_id: String) -> Result<Value, String> {
    Ok(json!({"id": _case_id, "name": "Stub Case"}))
}

#[tauri::command]
pub async fn list_cases(_state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn update_case_status(_state: State<'_, AppState>, _case_id: String, _status: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn delete_case(_state: State<'_, AppState>, _case_id: String) -> Result<(), String> {
    Ok(())
}

// ─── Entities & Relations ─────────────────────────────
#[tauri::command]
pub async fn create_entity(_state: State<'_, AppState>, _name: String, _entity_type: String) -> Result<String, String> {
    Ok(format!("entity_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_entity(_state: State<'_, AppState>, _entity_id: String) -> Result<Value, String> {
    Ok(json!({"id": _entity_id}))
}

#[tauri::command]
pub async fn list_entities(_state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn update_entity(_state: State<'_, AppState>, _entity_id: String, _name: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn delete_entity(_state: State<'_, AppState>, _entity_id: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn create_relation(_state: State<'_, AppState>, _source_id: String, _target_id: String, _relation_type: String) -> Result<String, String> {
    Ok(format!("rel_{}", uuid::Uuid::new_v4()))
}

#[tauri::command]
pub async fn get_relations(_state: State<'_, AppState>, _entity_id: String) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn delete_relation(_state: State<'_, AppState>, _relation_id: String) -> Result<(), String> {
    Ok(())
}

// ─── Tags ─────────────────────────────────────────────
#[tauri::command]
pub async fn add_tag(_state: State<'_, AppState>, _item_id: String, _tag: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn remove_tag(_state: State<'_, AppState>, _item_id: String, _tag: String) -> Result<(), String> {
    Ok(())
}

// ─── Encryption ───────────────────────────────────────
#[tauri::command]
pub async fn encrypt_text(_state: State<'_, AppState>, text: String) -> Result<String, String> {
    Ok(format!("encrypted_{}", text))
}

#[tauri::command]
pub async fn decrypt_text(_state: State<'_, AppState>, encrypted_text: String) -> Result<String, String> {
    Ok(encrypted_text.replace("encrypted_", ""))
}

// ─── Collection ───────────────────────────────────────
#[tauri::command]
pub async fn collect_files(_state: State<'_, AppState>, paths: Vec<String>) -> Result<Vec<String>, String> {
    Ok(paths)
}

#[tauri::command]
pub async fn get_collected_files(_state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

// ─── Plugins ──────────────────────────────────────────
#[tauri::command]
pub async fn load_plugin(_state: State<'_, AppState>, _plugin_path: String) -> Result<(), String> {
    Ok(())
}

#[tauri::command]
pub async fn list_plugins(_state: State<'_, AppState>) -> Result<Vec<Value>, String> {
    Ok(vec![])
}

#[tauri::command]
pub async fn run_plugin(_state: State<'_, AppState>, _plugin_name: String, _args: Value) -> Result<Value, String> {
    Ok(json!({"status": "success"}))
}

// ─── Export ───────────────────────────────────────────
#[tauri::command]
pub async fn export_case(_state: State<'_, AppState>, _case_id: String, _format: String) -> Result<String, String> {
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