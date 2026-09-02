use crate::{
    AppState,
    case::{CaseMetadata, CasePaths},
    crypto,
    db::GraphDb,
    search::SearchIndex,
    entity::{Entity, Relationship},
    plugin::PluginEngine,
    collect::Collector,
    config::Config,
};
use std::path::PathBuf;
use std::sync::Arc;
use tauri::State;
use walkdir::WalkDir;
use zip::{write::FileOptions, CompressionMethod};
use std::io::{Read, Write};

#[tauri::command]
pub async fn create_case(
    state: State<'_, AppState>,
    name: String,
    description: String,
    author: String,
    password: String,
) -> Result<CaseMetadata, String> {
    let config = {
        let guard = state.global_config.lock().await;
        guard.clone()
    };
    
    let case_id = uuid::Uuid::new_v4().to_string();
    let case_dir = config.cases_dir.join(&case_id);
    let paths = CasePaths::from_root(&case_dir);
    
    paths.ensure_dirs().map_err(|e| e.to_string())?;
    
    let meta = CaseMetadata {
        id: case_id,
        name,
        description,
        author,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        version: "0.1.0".to_string(),
    };
    
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    let encrypted = crypto::encrypt(meta_json.as_bytes(), &password).map_err(|e| e.to_string())?;
    std::fs::write(paths.root.join("case.meta"), encrypted).map_err(|e| e.to_string())?;
    
    let db = Arc::new(GraphDb::new(&paths.db).map_err(|e| e.to_string())?);
    let search = Arc::new(SearchIndex::new(&paths.search).map_err(|e| e.to_string())?);
    
    {
        let mut db_path_guard = state.db_path.lock().await;
        *db_path_guard = Some(paths.root.clone());
    }
    {
        let mut case_config_guard = state.case_config.lock().await;
        *case_config_guard = Some(meta.clone());
    }
    {
        let mut db_guard = state.db.lock().await;
        *db_guard = Some(db);
    }
    {
        let mut search_guard = state.search.lock().await;
        *search_guard = Some(search);
    }
    
    Ok(meta)
}

#[tauri::command]
pub async fn open_case(
    state: State<'_, AppState>,
    case_path: String,
    password: String,
) -> Result<CaseMetadata, String> {
    let path = PathBuf::from(case_path);
    let meta_path = path.join("case.meta");
    
    if !meta_path.exists() {
        return Err("Case metadata not found".to_string());
    }
    
    let encrypted = std::fs::read(&meta_path).map_err(|e| e.to_string())?;
    let decrypted = crypto::decrypt(&encrypted, &password).map_err(|e| e.to_string())?;
    let meta: CaseMetadata = serde_json::from_slice(&decrypted).map_err(|e| e.to_string())?;
    
    let paths = CasePaths::from_root(&path);
    
    let db = Arc::new(GraphDb::new(&paths.db).map_err(|e| e.to_string())?);
    let search = Arc::new(SearchIndex::new(&paths.search).map_err(|e| e.to_string())?);
    
    {
        let mut db_path_guard = state.db_path.lock().await;
        *db_path_guard = Some(path.clone());
    }
    {
        let mut case_config_guard = state.case_config.lock().await;
        *case_config_guard = Some(meta.clone());
    }
    {
        let mut db_guard = state.db.lock().await;
        *db_guard = Some(db);
    }
    {
        let mut search_guard = state.search.lock().await;
        *search_guard = Some(search);
    }
    
    Ok(meta)
}

#[tauri::command]
pub async fn save_case(
    state: State<'_, AppState>,
    password: String,
) -> Result<(), String> {
    let db_path = {
        let guard = state.db_path.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    let mut meta = {
        let guard = state.case_config.lock().await;
        guard.clone().ok_or("No case metadata")?
    };
    
    meta.updated_at = chrono::Utc::now();
    
    let paths = CasePaths::from_root(&db_path);
    let meta_json = serde_json::to_string_pretty(&meta).map_err(|e| e.to_string())?;
    let encrypted = crypto::encrypt(meta_json.as_bytes(), &password).map_err(|e| e.to_string())?;
    std::fs::write(paths.root.join("case.meta"), encrypted).map_err(|e| e.to_string())?;
    
    {
        let mut case_config_guard = state.case_config.lock().await;
        *case_config_guard = Some(meta);
    }
    
    Ok(())
}

#[tauri::command]
pub async fn close_case(state: State<'_, AppState>) -> Result<(), String> {
    {
        let mut guard = state.db_path.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.case_config.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.db.lock().await;
        *guard = None;
    }
    {
        let mut guard = state.search.lock().await;
        *guard = None;
    }
    Ok(())
}

#[tauri::command]
pub async fn get_case_info(state: State<'_, AppState>) -> Result<Option<CaseMetadata>, String> {
    let guard = state.case_config.lock().await;
    Ok(guard.clone())
}

#[tauri::command]
pub async fn add_entity(
    state: State<'_, AppState>,
    entity_type: String,
    label: String,
    properties: serde_json::Value,
) -> Result<Entity, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    let search = {
        let guard = state.search.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    let mut entity = Entity::new(&entity_type, &label);
    if let serde_json::Value::Object(map) = properties {
        for (k, v) in map {
            entity.properties.insert(k, v);
        }
    }
    
    db.insert_entity(&entity).map_err(|e| e.to_string())?;
    search.index_entity(&entity).map_err(|e| e.to_string())?;
    
    Ok(entity)
}

#[tauri::command]
pub async fn get_entities(state: State<'_, AppState>) -> Result<Vec<Entity>, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    db.get_all_entities().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_entity_by_id(state: State<'_, AppState>, id: String) -> Result<Option<Entity>, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    db.get_entity_by_id(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn search_entities(
    state: State<'_, AppState>,
    query: String,
    limit: usize,
) -> Result<Vec<crate::search::SearchResult>, String> {
    let search = {
        let guard = state.search.lock().await;
        guard.clone().ok_or("No case open")?
    };
    search.search(&query, limit).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn add_relationship(
    state: State<'_, AppState>,
    rel_type: String,
    source_id: String,
    target_id: String,
    properties: serde_json::Value,
) -> Result<Relationship, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    let mut rel = Relationship::new(&rel_type, &source_id, &target_id);
    if let serde_json::Value::Object(map) = properties {
        for (k, v) in map {
            rel.properties.insert(k, v);
        }
    }
    
    db.add_relationship(&rel).map_err(|e| e.to_string())?;
    Ok(rel)
}

#[tauri::command]
pub async fn get_relationships(state: State<'_, AppState>) -> Result<Vec<Relationship>, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    db.get_relationships().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn delete_entity(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    let search = {
        let guard = state.search.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    db.delete_entity(&id).map_err(|e| e.to_string())?;
    search.remove_entity(&id).map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub async fn update_entity(
    state: State<'_, AppState>,
    id: String,
    label: String,
    properties: serde_json::Value,
) -> Result<Entity, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    let search = {
        let guard = state.search.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    let mut entity = db.get_entity_by_id(&id).map_err(|e| e.to_string())?
        .ok_or("Entity not found")?;
    
    entity.label = label;
    entity.updated_at = chrono::Utc::now();
    if let serde_json::Value::Object(map) = properties {
        entity.properties = map.into_iter().collect();
    }
    
    db.update_entity(&entity).map_err(|e| e.to_string())?;
    search.remove_entity(&id).map_err(|e| e.to_string())?;
    search.index_entity(&entity).map_err(|e| e.to_string())?;
    
    Ok(entity)
}

#[tauri::command]
pub async fn run_transform(
    state: State<'_, AppState>,
    plugin_id: String,
    entity_id: String,
    config: std::collections::HashMap<String, String>,
) -> Result<crate::entity::TransformResult, String> {
    let db = {
        let guard = state.db.lock().await;
        guard.clone().ok_or("No case open")?
    };
    let search = {
        let guard = state.search.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    let global_config = {
        let guard = state.global_config.lock().await;
        guard.clone()
    };
    
    let engine = PluginEngine::new(&global_config.plugins_dir);
    let plugins = engine.discover_plugins().map_err(|e| e.to_string())?;
    
    let manifest = plugins.into_iter()
        .find(|p| p.id == plugin_id)
        .ok_or("Plugin not found")?;
    
    let source_entity = db.get_entity_by_id(&entity_id).map_err(|e| e.to_string())?
        .ok_or("Source entity not found")?;
    
    let output = engine.execute(&manifest, &source_entity, &config).await
        .map_err(|e| e.to_string())?;
    
    let mut result = crate::entity::TransformResult {
        entities: Vec::new(),
        relationships: Vec::new(),
    };
    
    let mut label_to_id: std::collections::HashMap<(String, String), String> = std::collections::HashMap::new();
    
    for partial in &output.entities {
        let key = (partial.entity_type.clone(), partial.label.clone());
        let existing_id = db.entity_exists_by_label(&partial.entity_type, &partial.label)
            .map_err(|e| e.to_string())?;
        
        let entity_id = if let Some(id) = existing_id {
            id
        } else {
            let mut entity = Entity::new(&partial.entity_type, &partial.label);
            entity.properties = partial.properties.clone();
            db.insert_entity(&entity).map_err(|e| e.to_string())?;
            search.index_entity(&entity).map_err(|e| e.to_string())?;
            entity.id.clone()
        };
        
        label_to_id.insert(key, entity_id.clone());
        result.entities.push(db.get_entity_by_id(&entity_id).map_err(|e| e.to_string())?.unwrap());
    }
    
    for partial_rel in &output.relationships {
        let source_key = (partial_rel.source_type.clone(), partial_rel.source_label.clone());
        let target_key = (partial_rel.target_type.clone(), partial_rel.target_label.clone());
        
        let source_id = if source_key.1 == source_entity.label && source_key.0 == source_entity.entity_type {
            source_entity.id.clone()
        } else {
            label_to_id.get(&source_key).cloned()
                .or_else(|| db.entity_exists_by_label(&source_key.0, &source_key.1).ok().flatten())
                .ok_or("Relationship source not found")?
        };
        
        let target_id = label_to_id.get(&target_key).cloned()
            .or_else(|| db.entity_exists_by_label(&target_key.0, &target_key.1).ok().flatten())
            .ok_or("Relationship target not found")?;
        
        let mut rel = Relationship::new(&partial_rel.rel_type, &source_id, &target_id);
        rel.properties = partial_rel.properties.clone();
        db.add_relationship(&rel).map_err(|e| e.to_string())?;
        result.relationships.push(rel);
    }
    
    Ok(result)
}

#[tauri::command]
pub async fn get_plugins(state: State<'_, AppState>) -> Result<Vec<crate::plugin::PluginManifest>, String> {
    let global_config = {
        let guard = state.global_config.lock().await;
        guard.clone()
    };
    let engine = PluginEngine::new(&global_config.plugins_dir);
    engine.discover_plugins().map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn extract_entities_from_text(
    _state: State<'_, AppState>,
    text: String,
) -> Result<Vec<Entity>, String> {
    let collector = Collector::new();
    collector.extract_entities_from_text(&text).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    let guard = state.global_config.lock().await;
    Ok(guard.clone())
}

#[tauri::command]
pub async fn set_config(
    state: State<'_, AppState>,
    config: Config,
) -> Result<(), String> {
    config.save().map_err(|e| e.to_string())?;
    let mut guard = state.global_config.lock().await;
    *guard = config;
    Ok(())
}

#[tauri::command]
pub async fn export_case(
    state: State<'_, AppState>,
    password: String,
) -> Result<String, String> {
    let db_path = {
        let guard = state.db_path.lock().await;
        guard.clone().ok_or("No case open")?
    };
    
    let export_path = db_path.with_extension("ekuke");
    let file = std::fs::File::create(&export_path).map_err(|e| e.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
    
    for entry in WalkDir::new(&db_path) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path.is_file() {
            let name = path.strip_prefix(&db_path).map_err(|e| e.to_string())?
                .to_string_lossy();
            zip.start_file(name, options).map_err(|e| e.to_string())?;
            let mut f = std::fs::File::open(path).map_err(|e| e.to_string())?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            zip.write_all(&buffer).map_err(|e| e.to_string())?;
        }
    }
    
    zip.finish().map_err(|e| e.to_string())?;
    
    let zip_bytes = std::fs::read(&export_path).map_err(|e| e.to_string())?;
    let encrypted = crypto::encrypt(&zip_bytes, &password).map_err(|e| e.to_string())?;
    std::fs::write(&export_path, encrypted).map_err(|e| e.to_string())?;
    
    Ok(export_path.to_string_lossy().to_string())
}

#[tauri::command]
pub async fn import_case(
    state: State<'_, AppState>,
    ekuke_path: String,
    password: String,
) -> Result<CaseMetadata, String> {
    let encrypted = std::fs::read(&ekuke_path).map_err(|e| e.to_string())?;
    let zip_bytes = crypto::decrypt(&encrypted, &password).map_err(|e| e.to_string())?;
    
    let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let temp_path = temp_dir.path();
    
    let zip_path = temp_path.join("case.zip");
    std::fs::write(&zip_path, &zip_bytes).map_err(|e| e.to_string())?;
    
    let file = std::fs::File::open(&zip_path).map_err(|e| e.to_string())?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    archive.extract(temp_path).map_err(|e| e.to_string())?;
    
    let meta_path = temp_path.join("case.meta");
    if !meta_path.exists() {
        return Err("Invalid .ekuke file: missing case.meta".to_string());
    }
    
    let meta_encrypted = std::fs::read(&meta_path).map_err(|e| e.to_string())?;
    let meta_json = crypto::decrypt(&meta_encrypted, &password).map_err(|e| e.to_string())?;
    let meta: CaseMetadata = serde_json::from_slice(&meta_json).map_err(|e| e.to_string())?;
    
    let config = {
        let guard = state.global_config.lock().await;
        guard.clone()
    };
    let case_dir = config.cases_dir.join(&meta.id);
    if case_dir.exists() {
        std::fs::remove_dir_all(&case_dir).map_err(|e| e.to_string())?;
    }
    std::fs::create_dir_all(&case_dir).map_err(|e| e.to_string())?;
    
    for entry in WalkDir::new(temp_path) {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        if path == temp_path || path == zip_path { continue; }
        if path.is_file() {
            let rel = path.strip_prefix(temp_path).map_err(|e| e.to_string())?;
            let dest = case_dir.join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(path, dest).map_err(|e| e.to_string())?;
        }
    }
    
    let paths = CasePaths::from_root(&case_dir);
    let db = Arc::new(GraphDb::new(&paths.db).map_err(|e| e.to_string())?);
    let search = Arc::new(SearchIndex::new(&paths.search).map_err(|e| e.to_string())?);
    
    {
        let mut db_path_guard = state.db_path.lock().await;
        *db_path_guard = Some(case_dir.clone());
    }
    {
        let mut case_config_guard = state.case_config.lock().await;
        *case_config_guard = Some(meta.clone());
    }
    {
        let mut db_guard = state.db.lock().await;
        *db_guard = Some(db);
    }
    {
        let mut search_guard = state.search.lock().await;
        *search_guard = Some(search);
    }
    
    Ok(meta)
}
