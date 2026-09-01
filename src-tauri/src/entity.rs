use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub label: String,
    pub properties: HashMap<String, Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Entity {
    pub fn new(entity_type: &str, label: &str) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            entity_type: entity_type.to_string(),
            label: label.to_string(),
            properties: HashMap::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub source_id: String,
    pub target_id: String,
    pub properties: HashMap<String, Value>,
    pub created_at: DateTime<Utc>,
}

impl Relationship {
    pub fn new(rel_type: &str, source_id: &str, target_id: &str) -> Self {
        Self {
            id: format!("{}-{}-{}", source_id, rel_type, target_id),
            rel_type: rel_type.to_string(),
            source_id: source_id.to_string(),
            target_id: target_id.to_string(),
            properties: HashMap::new(),
            created_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransformResult {
    pub entities: Vec<Entity>,
    pub relationships: Vec<Relationship>,
}