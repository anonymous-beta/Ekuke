use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, Local};
use serde_json::json;
use tauri::{AppHandle, State, Window};
use tauri::Emitter;
use tempfile::TempDir;
use tokio::sync::Mutex;

use crate::case::{CaseManager, CaseStatus, CaseMetadata};
use crate::collect::Collector;
use crate::config::Config;
use crate::crypto::EncryptionManager;
use crate::db::GraphDb;
use crate::entity::EntityManager;
use crate::models::SearchResult;
use crate::plugin::PluginManager;
use crate::search::SearchIndex;
use crate::AppState;

// ─── State ─────────────────────────────────────────────

#[tauri::command]
async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let config = state.global_config.lock().await;
    Ok(config.clone())
}

#[tauri::command]
async fn set_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    let mut cfg = state.global_config.lock().await;
    *cfg = config;
    cfg.save().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
async fn set_db_path(state: State<'_, AppState>, path: String) -> Result<(), String> {
    let path = PathBuf::from(path);
    let mut db_path = state.db_path.lock().await;
    *db_path = Some(path.clone());
    
    let mut global = state.global_config.lock().await;
    global.db_path = Some(path);
    global.save().map_err(|e| e.to_string())?;
    
    Ok(())
}

// ─── Database ─────────────────────────────────────────

#[tauri::command]
async fn init_db(state: State<'_, AppState>, path: Option<String>) -> Result<String, String> {
    let path = match path {
        Some(p) => PathBuf::from(p),
        None => {
            let cfg = state.global_config.lock().await;
            cfg.db_path.clone().unwrap_or_else(|| {
                let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
                p.push(".ekuke");
                p.push("db");
                p
            })
        }
    };

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let db = GraphDb::new(&path).map_err(|e| e.to_string())?;
    let db_arc = Arc::new(db);
    
    let mut db_guard = state.db.lock().await;
    *db_guard = Some(db_arc.clone());
    
    let mut db_path_guard = state.db_path.lock().await;
    *db_path_guard = Some(path.clone());
    
    let mut global = state.global_config.lock().await;
    global.db_path = Some(path.clone());
    global.save().map_err(|e| e.to_string())?;

    Ok(path.display().to_string())
}

#[tauri::command]
async fn get_db_stats(state: State<'_, AppState>) -> Result<serde_json::Value, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.get_stats().await.map_err(|e| e.to_string())
}

// ─── Notes / Evidence ──────────────────────────────────

#[tauri::command]
async fn create_note(
    state: State<'_, AppState>,
    case_id: String,
    title: String,
    content: String,
    tags: Option<Vec<String>>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    let note_id = db.create_note(case_id, title, content, tags)
        .await
        .map_err(|e| e.to_string())?;
    
    // Also index the note for search
    let search = state.search.lock().await;
    if let Some(search_arc) = search.as_ref() {
        if let Ok(note) = db.get_note(&note_id).await {
            if let Ok(search_engine) = Arc::clone(search_arc).lock() {
                let _ = search_engine.add_note(&note);
            }
        }
    }
    
    Ok(note_id)
}

#[tauri::command]
async fn get_note(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.get_note(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_note(
    state: State<'_, AppState>,
    id: String,
    title: Option<String>,
    content: Option<String>,
    tags: Option<Vec<String>>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.update_note(&id, title, content, tags)
        .await
        .map_err(|e| e.to_string())?;
    
    // Re-index the note
    let search = state.search.lock().await;
    if let Some(search_arc) = search.as_ref() {
        if let Ok(note) = db.get_note(&id).await {
            if let Ok(mut search_engine) = Arc::clone(search_arc).lock() {
                let _ = search_engine.update_note(&note.path, &note);
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn delete_note(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    // Get the note path before deleting
    let note = db.get_note(&id).await.ok();
    db.delete_note(&id).await.map_err(|e| e.to_string())?;
    
    // Remove from search index
    if let Some(note) = note {
        let search = state.search.lock().await;
        if let Some(search_arc) = search.as_ref() {
            if let Ok(mut search_engine) = Arc::clone(search_arc).lock() {
                let _ = search_engine.remove_note(&note.path);
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn list_notes(
    state: State<'_, AppState>,
    case_id: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.list_notes(case_id, limit.unwrap_or(100), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

// ─── Search ────────────────────────────────────────────

#[tauri::command]
async fn initialize_search(
    state: State<'_, AppState>,
) -> Result<(), String> {
    let mut search_guard = state.search.lock().await;
    
    if search_guard.is_some() {
        return Ok(());
    }
    
    let index_path = {
        let cfg = state.global_config.lock().await;
        cfg.search_index_path.clone().unwrap_or_else(|| {
            let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
            p.push(".ekuke");
            p.push("search_index");
            p
        })
    };
    
    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    
    let search_index = SearchIndex::new(&index_path).map_err(|e| e.to_string())?;
    *search_guard = Some(Arc::new(search_index));
    
    // Index existing notes
    if let Some(search_arc) = search_guard.as_ref() {
        let db = state.db.lock().await;
        if let Some(db_arc) = db.as_ref() {
            if let Ok(notes) = db_arc.list_notes(None, 10000, 0).await {
                if let Ok(mut search_engine) = Arc::clone(search_arc).lock() {
                    for note_val in notes {
                        if let Ok(title) = note_val["title"].as_str().unwrap_or("").to_string() {
                            // Attempt to re-index
                            // This is a simplified re-index; in production you'd use a proper Note struct
                        }
                    }
                }
            }
        }
    }
    
    Ok(())
}

#[tauri::command]
async fn search_notes(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
    fuzzy: Option<bool>,
) -> Result<Vec<serde_json::Value>, String> {
    let search_guard = state.search.lock().await;
    let search_arc = search_guard.as_ref().ok_or("Search not initialized")?;
    
    let search_engine = search_arc.lock();
    let results = search_engine.search(
        &query,
        &crate::models::SearchOptions {
            limit: limit.unwrap_or(50),
            fuzzy_distance: if fuzzy.unwrap_or(false) { Some(2) } else { None },
        },
    ).map_err(|e| e.to_string())?;
    
    Ok(results
        .into_iter()
        .map(|item| {
            json!({
                "title": item.title,
                "path": item.path.display().to_string(),
                "content": item.content,
                "timestamp": item.timestamp,
                "score": item.score,
            })
        })
        .collect())
}

#[tauri::command]
async fn search_exact(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let search_guard = state.search.lock().await;
    let search_arc = search_guard.as_ref().ok_or("Search not initialized")?;
    
    let search_engine = search_arc.lock();
    let results = search_engine.search(
        &query,
        &crate::models::SearchOptions {
            limit: limit.unwrap_or(50),
            fuzzy_distance: None,
        },
    ).map_err(|e| e.to_string())?;
    
    Ok(results
        .into_iter()
        .map(|item| {
            json!({
                "title": item.title,
                "path": item.path.display().to_string(),
                "content": item.content,
                "timestamp": item.timestamp,
                "score": item.score,
            })
        })
        .collect())
}

// ─── Case Management ──────────────────────────────────

#[tauri::command]
async fn create_case(
    state: State<'_, AppState>,
    name: String,
    description: Option<String>,
    case_type: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    let metadata = CaseMetadata {
        id: uuid::Uuid::new_v4().to_string(),
        name,
        description: description.unwrap_or_default(),
        case_type: case_type.unwrap_or_else(|| "General".to_string()),
        status: CaseStatus::Active,
        created_at: Local::now().to_rfc3339(),
        updated_at: Local::now().to_rfc3339(),
        tags: Vec::new(),
    };
    
    let case_id = metadata.id.clone();
    db.create_case(metadata).await.map_err(|e| e.to_string())?;
    Ok(case_id)
}

#[tauri::command]
async fn get_case(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.get_case(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_cases(
    state: State<'_, AppState>,
    limit: Option<usize>,
    offset: Option<usize>,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.list_cases(limit.unwrap_or(100), offset.unwrap_or(0))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_case_status(
    state: State<'_, AppState>,
    id: String,
    status: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    let status = match status.to_lowercase().as_str() {
        "active" => CaseStatus::Active,
        "closed" => CaseStatus::Closed,
        "archived" => CaseStatus::Archived,
        _ => return Err("Invalid status".to_string()),
    };
    
    db.update_case_status(&id, status).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_case(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.delete_case(&id).await.map_err(|e| e.to_string())
}

// ─── Entity Management ─────────────────────────────────

#[tauri::command]
async fn create_entity(
    state: State<'_, AppState>,
    case_id: String,
    name: String,
    entity_type: String,
    metadata: Option<serde_json::Value>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    let entity = crate::entity::Entity {
        id: uuid::Uuid::new_v4().to_string(),
        case_id,
        name,
        entity_type,
        metadata: metadata.unwrap_or_else(|| json!({})),
        created_at: Local::now().to_rfc3339(),
        updated_at: Local::now().to_rfc3339(),
    };
    
    let id = entity.id.clone();
    db.create_entity(entity).await.map_err(|e| e.to_string())?;
    Ok(id)
}

#[tauri::command]
async fn get_entity(
    state: State<'_, AppState>,
    id: String,
) -> Result<serde_json::Value, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.get_entity(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_entities(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.list_entities(&case_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn update_entity(
    state: State<'_, AppState>,
    id: String,
    name: Option<String>,
    metadata: Option<serde_json::Value>,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.update_entity(&id, name, metadata).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_entity(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.delete_entity(&id).await.map_err(|e| e.to_string())
}

// ─── Relations ─────────────────────────────────────────

#[tauri::command]
async fn create_relation(
    state: State<'_, AppState>,
    case_id: String,
    source_id: String,
    target_id: String,
    relation_type: String,
    metadata: Option<serde_json::Value>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.create_relation(case_id, source_id, target_id, relation_type, metadata)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_relations(
    state: State<'_, AppState>,
    entity_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.get_relations(&entity_id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn delete_relation(
    state: State<'_, AppState>,
    id: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.delete_relation(&id).await.map_err(|e| e.to_string())
}

// ─── Tags ──────────────────────────────────────────────

#[tauri::command]
async fn add_tag(
    state: State<'_, AppState>,
    case_id: String,
    tag: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.add_tag(&case_id, &tag).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_tag(
    state: State<'_, AppState>,
    case_id: String,
    tag: String,
) -> Result<(), String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.remove_tag(&case_id, &tag).await.map_err(|e| e.to_string())
}

// ─── Encryption ────────────────────────────────────────

#[tauri::command]
async fn encrypt_text(
    text: String,
    password: String,
) -> Result<String, String> {
    let manager = EncryptionManager::new();
    manager.encrypt_text(&text, &password).map_err(|e| e.to_string())
}

#[tauri::command]
async fn decrypt_text(
    encrypted: String,
    password: String,
) -> Result<String, String> {
    let manager = EncryptionManager::new();
    manager.decrypt_text(&encrypted, &password).map_err(|e| e.to_string())
}

// ─── File Collection ──────────────────────────────────

#[tauri::command]
async fn collect_files(
    state: State<'_, AppState>,
    case_id: String,
    source_paths: Vec<String>,
    target_dir: Option<String>,
    recursive: Option<bool>,
    preserve_structure: Option<bool>,
) -> Result<Vec<String>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    let collector = Collector::new(db.clone());
    let target = target_dir.map(PathBuf::from);
    
    let sources: Vec<PathBuf> = source_paths.into_iter().map(PathBuf::from).collect();
    
    collector.collect(
        &case_id,
        &sources,
        target.as_ref(),
        recursive.unwrap_or(true),
        preserve_structure.unwrap_or(true),
    ).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_collected_files(
    state: State<'_, AppState>,
    case_id: String,
) -> Result<Vec<serde_json::Value>, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    db.get_collected_files(&case_id).await.map_err(|e| e.to_string())
}

// ─── Plugin System ─────────────────────────────────────

#[tauri::command]
async fn load_plugin(
    state: State<'_, AppState>,
    path: String,
) -> Result<serde_json::Value, String> {
    let mut plugin_manager = state.plugin_manager.lock().await;
    let manager = plugin_manager.as_mut().ok_or("Plugin manager not initialized")?;
    manager.load_plugin(&PathBuf::from(path)).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_plugins(
    state: State<'_, AppState>,
) -> Result<Vec<serde_json::Value>, String> {
    let plugin_manager = state.plugin_manager.lock().await;
    let manager = plugin_manager.as_ref().ok_or("Plugin manager not initialized")?;
    Ok(manager.list_plugins())
}

#[tauri::command]
async fn run_plugin(
    state: State<'_, AppState>,
    name: String,
    input: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let plugin_manager = state.plugin_manager.lock().await;
    let manager = plugin_manager.as_ref().ok_or("Plugin manager not initialized")?;
    manager.run_plugin(&name, input).await.map_err(|e| e.to_string())
}

// ─── Export ────────────────────────────────────────────

#[tauri::command]
async fn export_case(
    state: State<'_, AppState>,
    case_id: String,
    format: String,
    path: Option<String>,
) -> Result<String, String> {
    let db = state.db.lock().await;
    let db = db.as_ref().ok_or("Database not initialized")?;
    
    let export_path = path.map(PathBuf::from).unwrap_or_else(|| {
        let mut p = std::env::temp_dir();
        p.push(format!("case_export_{}.{}", case_id, format));
        p
    });
    
    db.export_case(&case_id, &format, &export_path)
        .await
        .map_err(|e| e.to_string())?;
    
    Ok(export_path.display().to_string())
}

// ─── System ────────────────────────────────────────────

#[tauri::command]
async fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
async fn get_system_info() -> Result<serde_json::Value, String> {
    Ok(json!({
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "version": env!("CARGO_PKG_VERSION"),
        "hostname": gethostname::gethostname().to_string_lossy().to_string(),
    }))
}

#[tauri::command]
async fn health_check() -> String {
    "ok".to_string()
}

// ─── Init ──────────────────────────────────────────────

pub fn init() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    let plugin = tauri::plugin::Builder::new("commands")
        .invoke_handler(tauri::generate_handler![
            // Config
            get_config,
            set_config,
            set_db_path,
            
            // Database
            init_db,
            get_db_stats,
            
            // Notes
            create_note,
            get_note,
            update_note,
            delete_note,
            list_notes,
            
            // Search
            initialize_search,
            search_notes,
            search_exact,
            
            // Cases
            create_case,
            get_case,
            list_cases,
            update_case_status,
            delete_case,
            
            // Entities
            create_entity,
            get_entity,
            list_entities,
            update_entity,
            delete_entity,
            
            // Relations
            create_relation,
            get_relations,
            delete_relation,
            
            // Tags
            add_tag,
            remove_tag,
            
            // Encryption
            encrypt_text,
            decrypt_text,
            
            // Collection
            collect_files,
            get_collected_files,
            
            // Plugins
            load_plugin,
            list_plugins,
            run_plugin,
            
            // Export
            export_case,
            
            // System
            get_app_version,
            get_system_info,
            health_check,
        ])
        .build();

    plugin
}