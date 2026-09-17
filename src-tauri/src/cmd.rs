use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};
use tauri::State;
use tokio::sync::Mutex;

use crate::ai::{AiClient, ChatMessage};
use crate::case::{CaseMetadata, CasePaths};
use crate::collect::Collector;
use crate::config::Config;
use crate::db::GraphDb;
use crate::entity::{Entity, Relationship};
use crate::plugin::{PluginEngine, PluginOutput};

type Cmd<T> = Result<T, String>;

async fn require_db(state: &State<'_, crate::AppState>) -> Cmd<Arc<GraphDb>> {
    state.db.lock().await.clone()
        .ok_or_else(|| "No case is open. Create or open a case first.".to_string())
}

async fn require_config(state: &State<'_, crate::AppState>) -> Cmd<Config> {
    Ok(state.config.lock().await.clone())
}

fn save_case_meta(root: &PathBuf, meta: &CaseMetadata) -> Cmd<()> {
    let mut v = serde_json::to_value(meta).map_err(|e| e.to_string())?;
    v["status"] = json!("open");
    std::fs::write(root.join("case.json"), serde_json::to_string_pretty(&v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

// ─── Config / Settings ────────────────────────────────
#[tauri::command]
pub async fn get_config(state: State<'_, crate::AppState>) -> Cmd<Value> {
    let cfg = state.config.lock().await;
    serde_json::to_value(&*cfg).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn set_config(state: State<'_, crate::AppState>, config: Value) -> Cmd<()> {
    let new_cfg: Config = serde_json::from_value(config).map_err(|e| e.to_string())?;
    new_cfg.save().map_err(|e| e.to_string())?;
    *state.config.lock().await = new_cfg;
    Ok(())
}

// ─── Cases ────────────────────────────────────────────
#[tauri::command]
pub async fn create_case(state: State<'_, crate::AppState>, name: String) -> Cmd<Value> {
    let cfg = require_config(&state).await?;
    let mut meta = CaseMetadata::new(&name, &cfg.default_author);
    let paths = CasePaths::from_root(cfg.cases_dir.join(&meta.id));
    paths.ensure_dirs().map_err(|e| e.to_string())?;
    save_case_meta(&paths.root, &meta)?;

    let db = GraphDb::new(&paths.db).map_err(|e| e.to_string())?;
    *state.case_config.lock().await = Some(meta.clone());
    *state.db.lock().await = Some(Arc::new(db));
    *state.db_path.lock().await = Some(paths.db.clone());

    Ok(json!({"id": meta.id, "name": meta.name, "root": paths.root.display().to_string()}))
}

fn read_case(root: &PathBuf) -> Cmd<(CaseMetadata, String)> {
    let content = std::fs::read_to_string(root.join("case.json")).map_err(|e| e.to_string())?;
    let v: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
    let meta: CaseMetadata = serde_json::from_value(v.clone()).map_err(|e| e.to_string())?;
    let status = v.get("status").and_then(|s| s.as_str()).unwrap_or("open").to_string();
    Ok((meta, status))
}

#[tauri::command]
pub async fn list_cases(state: State<'_, crate::AppState>) -> Cmd<Vec<Value>> {
    let cfg = require_config(&state).await?;
    let mut out = Vec::new();
    if !cfg.cases_dir.exists() { return Ok(out); }
    for entry in std::fs::read_dir(&cfg.cases_dir).map_err(|e| e.to_string())? {
        let root = entry.map_err(|e| e.to_string())?.path();
        if root.is_dir() && root.join("case.json").exists() {
            if let Ok((meta, status)) = read_case(&root) {
                out.push(json!({
                    "id": meta.id, "name": meta.name, "description": meta.description,
                    "created_at": meta.created_at, "status": status,
                }));
            }
        }
    }
    Ok(out)
}

#[tauri::command]
pub async fn open_case(state: State<'_, crate::AppState>, case_id: String) -> Cmd<Value> {
    let cfg = require_config(&state).await?;
    let root = cfg.cases_dir.join(&case_id);
    if !root.join("case.json").exists() { return Err("Case not found".into()); }
    let (meta, _) = read_case(&root)?;
    let paths = CasePaths::from_root(root);
    let db = GraphDb::new(&paths.db).map_err(|e| e.to_string())?;
    *state.case_config.lock().await = Some(meta.clone());
    *state.db.lock().await = Some(Arc::new(db));
    *state.db_path.lock().await = Some(paths.db.clone());
    Ok(json!({"id": meta.id, "name": meta.name}))
}

#[tauri::command]
pub async fn delete_case(state: State<'_, crate::AppState>, case_id: String) -> Cmd<()> {
    let cfg = require_config(&state).await?;
    let root = cfg.cases_dir.join(&case_id);
    if !root.exists() { return Err("Case not found".into()); }
    std::fs::remove_dir_all(&root).map_err(|e| e.to_string())?;
    let open = state.case_config.lock().await.clone();
    if let Some(meta) = open {
        if meta.id == case_id {
            *state.case_config.lock().await = None;
            *state.db.lock().await = None;
            *state.db_path.lock().await = None;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn get_current_case(state: State<'_, crate::AppState>) -> Cmd<Value> {
    let open = state.case_config.lock().await.clone();
    match open {
        Some(meta) => Ok(json!({"id": meta.id, "name": meta.name})),
        None => Ok(Value::Null),
    }
}

// ─── Entities ─────────────────────────────────────────
#[tauri::command]
pub async fn get_entities(state: State<'_, crate::AppState>, entity_type: Option<String>) -> Cmd<Vec<Value>> {
    let db = require_db(&state).await?;
    let entities = match entity_type {
        Some(t) if !t.is_empty() => db.search_entities_by_type(&t).map_err(|e| e.to_string())?,
        _ => db.get_all_entities().map_err(|e| e.to_string())?,
    };
    Ok(entities.iter().map(|e| serde_json::to_value(e).unwrap_or(Value::Null)).collect())
}

#[tauri::command]
pub async fn get_entity_by_id(state: State<'_, crate::AppState>, id: String) -> Cmd<Value> {
    let db = require_db(&state).await?;
    let e = db.get_entity_by_id(&id).map_err(|e| e.to_string())?
        .ok_or_else(|| "Entity not found".to_string())?;
    serde_json::to_value(&e).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn create_entity(
    state: State<'_, crate::AppState>,
    entity_type: String,
    label: String,
    properties: Option<Value>,
) -> Cmd<Value> {
    let db = require_db(&state).await?;
    if let Some(id) = db.entity_exists_by_label(&entity_type, &label).map_err(|e| e.to_string())? {
        if let Some(e) = db.get_entity_by_id(&id).map_err(|e| e.to_string())? {
            return serde_json::to_value(&e).map_err(|e| e.to_string());
        }
    }
    let mut entity = Entity::new(&entity_type, &label);
    if let Some(Value::Object(map)) = properties {
        for (k, v) in map { entity.properties.insert(k, v); }
    }
    db.insert_entity(&entity).map_err(|e| e.to_string())?;
    serde_json::to_value(&entity).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn update_entity(
    state: State<'_, crate::AppState>,
    id: String,
    label: String,
    properties: Option<Value>,
) -> Cmd<Value> {
    let db = require_db(&state).await?;
    let mut e = db.get_entity_by_id(&id).map_err(|e| e.to_string())?
        .ok_or_else(|| "Entity not found".to_string())?;
    e.label = label;
    if let Some(Value::Object(map)) = properties {
        for (k, v) in map { e.properties.insert(k, v); }
    }
    e.updated_at = chrono::Utc::now();
    db.update_entity(&e).map_err(|e| e.to_string())?;
    serde_json::to_value(&e).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity(state: State<'_, crate::AppState>, id: String) -> Cmd<()> {
    let db = require_db(&state).await?;
    db.delete_entity(&id).map_err(|e| e.to_string())
}

// ─── Relationships ────────────────────────────────────
#[tauri::command]
pub async fn create_relationship(
    state: State<'_, crate::AppState>,
    source_id: String,
    target_id: String,
    rel_type: String,
    properties: Option<Value>,
) -> Cmd<Value> {
    let db = require_db(&state).await?;
    if db.get_entity_by_id(&source_id).map_err(|e| e.to_string())?.is_none() {
        return Err("Source entity not found".into());
    }
    if db.get_entity_by_id(&target_id).map_err(|e| e.to_string())?.is_none() {
        return Err("Target entity not found".into());
    }
    let mut rel = Relationship::new(&rel_type, &source_id, &target_id);
    if let Some(Value::Object(map)) = properties {
        for (k, v) in map { rel.properties.insert(k, v); }
    }
    db.add_relationship(&rel).map_err(|e| e.to_string())?;
    serde_json::to_value(&rel).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_relationships(state: State<'_, crate::AppState>) -> Cmd<Vec<Value>> {
    let db = require_db(&state).await?;
    let rels = db.get_relationships().map_err(|e| e.to_string())?;
    Ok(rels.iter().map(|r| serde_json::to_value(r).unwrap_or(Value::Null)).collect())
}

#[tauri::command]
pub async fn search_entities(state: State<'_, crate::AppState>, query: String, limit: Option<usize>) -> Cmd<Vec<Value>> {
    let db = require_db(&state).await?;
    let entities = db.search_entities_by_label(&query, limit.unwrap_or(20)).map_err(|e| e.to_string())?;
    Ok(entities.iter().map(|e| serde_json::to_value(e).unwrap_or(Value::Null)).collect())
}

// ─── Plugins ──────────────────────────────────────────
#[tauri::command]
pub async fn get_plugins(state: State<'_, crate::AppState>) -> Cmd<Vec<Value>> {
    let cfg = require_config(&state).await?;
    let engine = PluginEngine::new(&cfg.plugins_dir);
    let manifests = engine.discover_plugins().map_err(|e| e.to_string())?;
    Ok(manifests.iter().map(|m| serde_json::to_value(m).unwrap_or(Value::Null)).collect())
}

#[tauri::command]
pub async fn run_transform(
    state: State<'_, crate::AppState>,
    plugin_id: String,
    entity_id: String,
    config: Option<Value>,
) -> Cmd<Value> {
    let db = require_db(&state).await?;
    let cfg = require_config(&state).await?;
    let engine = PluginEngine::new(&cfg.plugins_dir);
    let manifests = engine.discover_plugins().map_err(|e| e.to_string())?;
    let manifest = manifests.iter().find(|m| m.id == plugin_id)
        .ok_or_else(|| format!("Plugin '{}' not found in {:?}", plugin_id, cfg.plugins_dir))?;
    let entity = db.get_entity_by_id(&entity_id).map_err(|e| e.to_string())?
        .ok_or_else(|| "Entity not found".to_string())?;

    let mut plugin_config: HashMap<String, String> = cfg.api_keys.clone();
    if let Some(Value::Object(map)) = config {
        for (k, v) in map {
            if let Some(s) = v.as_str() { plugin_config.insert(k, s.to_string()); }
        }
    }

    let output: PluginOutput = engine.execute(manifest, &entity, &plugin_config).await.map_err(|e| e.to_string())?;

    let mut ids: HashMap<(String, String), String> = HashMap::new();
    ids.insert((entity.entity_type.clone(), entity.label.clone()), entity.id.clone());
    for pe in &output.entities {
        let id = if let Some(existing) = db.entity_exists_by_label(&pe.entity_type, &pe.label).map_err(|e| e.to_string())? {
            existing
        } else {
            let mut e = Entity::new(&pe.entity_type, &pe.label);
            e.properties = pe.properties.clone();
            db.insert_entity(&e).map_err(|err| err.to_string())?;
            e.id
        };
        ids.insert((pe.entity_type.clone(), pe.label.clone()), id);
    }
    let mut rel_count = 0;
    for pr in &output.relationships {
        if let (Some(s), Some(t)) = (
            ids.get(&(pr.source_type.clone(), pr.source_label.clone())),
            ids.get(&(pr.target_type.clone(), pr.target_label.clone())),
        ) {
            db.add_relationship(&Relationship::new(&pr.rel_type, s, t)).map_err(|e| e.to_string())?;
            rel_count += 1;
        }
    }
    Ok(json!({"entities": output.entities.len(), "relationships": rel_count}))
}

// ─── Text extraction ──────────────────────────────────
#[tauri::command]
pub async fn extract_entities_from_text(state: State<'_, crate::AppState>, text: String) -> Cmd<usize> {
    let db = require_db(&state).await?;
    let collector = Collector::new();
    let entities = collector.extract_entities_from_text(&text).map_err(|e| e.to_string())?;
    let mut added = 0;
    for e in entities {
        if db.entity_exists_by_label(&e.entity_type, &e.label).map_err(|er| er.to_string())?.is_none() {
            db.insert_entity(&e).map_err(|er| er.to_string())?;
            added += 1;
        }
    }
    Ok(added)
}

// ─── Export / Import (.ekuke zip) ─────────────────────
#[tauri::command]
pub async fn export_case(state: State<'_, crate::AppState>, case_id: String) -> Cmd<String> {
    let cfg = require_config(&state).await?;
    let root = cfg.cases_dir.join(&case_id);
    if !root.join("case.json").exists() { return Err("Case not found".into()); }

    let out_path = cfg.cases_dir.join(format!("{}.ekuke", case_id));
    let file = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    zip.add_directory("data", options).map_err(|e| e.to_string())?;
    let case_json = std::fs::read(root.join("case.json")).map_err(|e| e.to_string())?;
    zip.start_file("case.json", options).map_err(|e| e.to_string())?;
    std::io::Write::write_all(&mut zip, &case_json).map_err(|e| e.to_string())?;

    let db_path = CasePaths::from_root(&root).db;
    if db_path.exists() {
        zip.start_file("data/ekuke.db", options).map_err(|e| e.to_string())?;
        let db_bytes = std::fs::read(&db_path).map_err(|e| e.to_string())?;
        std::io::Write::write_all(&mut zip, &db_bytes).map_err(|e| e.to_string())?;
    }
    zip.finish().map_err(|e| e.to_string())?;
    Ok(out_path.display().to_string())
}

#[tauri::command]
pub async fn import_case(state: State<'_, crate::AppState>, path: String) -> Cmd<Value> {
    let cfg = require_config(&state).await?;
    let file = std::fs::File::open(&path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;

    // read case.json first to get id
    let mut case_json = String::new();
    {
        let mut f = archive.by_name("case.json").map_err(|e| e.to_string())?;
        std::io::Read::read_to_string(&mut f, &mut case_json).map_err(|e| e.to_string())?;
    }
    let meta: CaseMetadata = serde_json::from_str(&case_json).map_err(|e| e.to_string())?;
    let root = cfg.cases_dir.join(&meta.id);
    std::fs::create_dir_all(root.join("data")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(root.join("attachments")).map_err(|e| e.to_string())?;
    std::fs::write(root.join("case.json"), &case_json).map_err(|e| e.to_string())?;

    if let Ok(mut f) = archive.by_name("data/ekuke.db") {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut f, &mut bytes).map_err(|e| e.to_string())?;
        std::fs::write(root.join("data").join("ekuke.db"), bytes).map_err(|e| e.to_string())?;
    }

    let paths = CasePaths::from_root(&root);
    let db = GraphDb::new(&paths.db).map_err(|e| e.to_string())?;
    *state.case_config.lock().await = Some(meta.clone());
    *state.db.lock().await = Some(Arc::new(db));
    *state.db_path.lock().await = Some(paths.db.clone());
    Ok(json!({"id": meta.id, "name": meta.name}))
}

// ─── AI Assistant ─────────────────────────────────────
#[tauri::command]
pub async fn ai_chat(
    state: State<'_, crate::AppState>,
    messages: Vec<ChatMessage>,
) -> Cmd<Value> {
    let cfg = require_config(&state).await?;
    if !cfg.ai_enabled { return Err("AI assistant is disabled. Enable it in Settings.".into()); }
    if cfg.ai_api_key.is_empty() && !cfg.ai_base_url.contains("localhost") && !cfg.ai_base_url.contains("127.0.0.1") {
        return Err("No AI API key configured. Set one in Settings.".into());
    }
    let client = AiClient::new(cfg.ai_base_url.clone(), cfg.ai_api_key.clone(),
        cfg.ai_model.clone(), cfg.ai_temperature);

    let db = state.db.lock().await.clone();
    let plugins_dir = cfg.plugins_dir.clone();
    let reply = client.chat(&messages, db, Some(plugins_dir), cfg.api_keys.clone()).await
        .map_err(|e| e.to_string())?;
    Ok(json!({"reply": reply.0, "actions": reply.1}))
}

#[tauri::command]
pub async fn ai_test_connection(state: State<'_, crate::AppState>) -> Cmd<String> {
    let cfg = require_config(&state).await?;
    let client = AiClient::new(cfg.ai_base_url.clone(), cfg.ai_api_key.clone(),
        cfg.ai_model.clone(), cfg.ai_temperature);
    client.test_connection().await.map_err(|e| e.to_string())
}

// ─── System ───────────────────────────────────────────
#[tauri::command]
pub async fn health_check() -> String { "ok".to_string() }

#[tauri::command]
pub async fn get_app_version() -> String { env!("CARGO_PKG_VERSION").to_string() }