use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entity {
    pub id: String,
    pub entity_type: String,
    pub label: String,
    pub properties: HashMap<String, serde_json::Value>,
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

    pub fn with_properties(mut self, props: HashMap<String, serde_json::Value>) -> Self {
        self.properties = props;
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub source_id: String,
    pub target_id: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

impl Relationship {
    pub fn new(rel_type: &str, source_id: &str, target_id: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
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
