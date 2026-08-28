use crate::entity::{Entity, Relationship};
use anyhow::{Context, Result};
use kuzu::{Database, Connection, SystemConfig};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GraphDb {
    db: Arc<Mutex<Database>>,
}

impl GraphDb {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let db = Database::new(path, SystemConfig::default())
            .context("Failed to initialize Kùzu database")?;
        
        let conn = Connection::new(&db)?;
        
        // Initialize schema if empty
        conn.query(
            "CREATE NODE TABLE IF NOT EXISTS Entity(
                id STRING, 
                entity_type STRING, 
                label STRING, 
                properties STRING, 
                created_at STRING, 
                updated_at STRING, 
                PRIMARY KEY(id)
            )"
        ).ok();
        
        conn.query(
            "CREATE REL TABLE IF NOT EXISTS RELATES(
                FROM Entity TO Entity, 
                rel_type STRING, 
                properties STRING, 
                created_at STRING
            )"
        ).ok();

        Ok(Self {
            db: Arc::new(Mutex::new(db)),
        })
    }

    pub fn insert_entity(&self, entity: &Entity) -> Result<()> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let props = serde_json::to_string(&entity.properties)?;
        
        let query = format!(
            "CREATE (e:Entity {{
                id: '{}', 
                entity_type: '{}', 
                label: '{}', 
                properties: '{}', 
                created_at: '{}', 
                updated_at: '{}'
            }})",
            escape_cypher(&entity.id),
            escape_cypher(&entity.entity_type),
            escape_cypher(&entity.label),
            escape_cypher(&props),
            entity.created_at.to_rfc3339(),
            entity.updated_at.to_rfc3339()
        );
        
        conn.query(&query).context("Failed to insert entity")?;
        Ok(())
    }

    pub fn update_entity(&self, entity: &Entity) -> Result<()> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let props = serde_json::to_string(&entity.properties)?;
        
        let query = format!(
            "MATCH (e:Entity {{id: '{}'}}) 
             SET e.label = '{}', e.properties = '{}', e.updated_at = '{}'",
            escape_cypher(&entity.id),
            escape_cypher(&entity.label),
            escape_cypher(&props),
            entity.updated_at.to_rfc3339()
        );
        
        conn.query(&query).context("Failed to update entity")?;
        Ok(())
    }

    pub fn get_entity_by_id(&self, id: &str) -> Result<Option<Entity>> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let query = format!("MATCH (e:Entity {{id: '{}'}}) RETURN e", escape_cypher(id));
        
        let result = conn.query(&query).context("Failed to query entity")?;
        
        if let Some(row) = result.iter().next() {
            let node = row.get_node(0)?;
            let entity = node_to_entity(&node)?;
            return Ok(Some(entity));
        }
        
        Ok(None)
    }

    pub fn get_all_entities(&self) -> Result<Vec<Entity>> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let result = conn.query("MATCH (e:Entity) RETURN e")?;
        
        let mut entities = Vec::new();
        for row in result.iter() {
            let node = row.get_node(0)?;
            entities.push(node_to_entity(&node)?);
        }
        
        Ok(entities)
    }

    pub fn search_entities_by_type(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let query = format!(
            "MATCH (e:Entity) WHERE e.entity_type = '{}' RETURN e",
            escape_cypher(entity_type)
        );
        
        let result = conn.query(&query)?;
        let mut entities = Vec::new();
        for row in result.iter() {
            let node = row.get_node(0)?;
            entities.push(node_to_entity(&node)?);
        }
        
        Ok(entities)
    }

    pub fn delete_entity(&self, id: &str) -> Result<()> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let query = format!("MATCH (e:Entity {{id: '{}'}}) DETACH DELETE e", escape_cypher(id));
        conn.query(&query).context("Failed to delete entity")?;
        Ok(())
    }

    pub fn add_relationship(&self, rel: &Relationship) -> Result<()> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let props = serde_json::to_string(&rel.properties)?;
        
        let query = format!(
            "MATCH (a:Entity {{id: '{}'}}), (b:Entity {{id: '{}'}})
             CREATE (a)-[:RELATES {{
                rel_type: '{}', 
                properties: '{}', 
                created_at: '{}'
             }}]->(b)",
            escape_cypher(&rel.source_id),
            escape_cypher(&rel.target_id),
            escape_cypher(&rel.rel_type),
            escape_cypher(&props),
            rel.created_at.to_rfc3339()
        );
        
        conn.query(&query).context("Failed to create relationship")?;
        Ok(())
    }

    pub fn get_relationships(&self) -> Result<Vec<Relationship>> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let result = conn.query("MATCH (a:Entity)-[r:RELATES]->(b:Entity) RETURN a.id, b.id, r")?;
        
        let mut relationships = Vec::new();
        for row in result.iter() {
            let source_id: String = row.get(0)?;
            let target_id: String = row.get(1)?;
            let rel = row.get_rel(2)?;
            
            let rel_type = rel.get_property("rel_type").and_then(|v| v.get_str().ok()).unwrap_or("related").to_string();
            let props_str = rel.get_property("properties").and_then(|v| v.get_str().ok()).unwrap_or("{}");
            let properties = serde_json::from_str(props_str).unwrap_or_default();
            let created_at = rel.get_property("created_at")
                .and_then(|v| v.get_str().ok())
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(chrono::Utc::now);
            
            relationships.push(Relationship {
                id: format!("{}-{}-{}", source_id, rel_type, target_id),
                rel_type,
                source_id,
                target_id,
                properties,
                created_at,
            });
        }
        
        Ok(relationships)
    }

    pub fn entity_exists_by_label(&self, entity_type: &str, label: &str) -> Result<Option<String>> {
        let db = self.db.blocking_lock();
        let conn = Connection::new(&*db)?;
        let query = format!(
            "MATCH (e:Entity) WHERE e.entity_type = '{}' AND e.label = '{}' RETURN e.id LIMIT 1",
            escape_cypher(entity_type),
            escape_cypher(label)
        );
        
        let result = conn.query(&query)?;
        if let Some(row) = result.iter().next() {
            let id: String = row.get(0)?;
            return Ok(Some(id));
        }
        
        Ok(None)
    }
}

fn escape_cypher(input: &str) -> String {
    input.replace('\\', "\\\\").replace('\'', "\\'").replace('\n', "\\n")
}

fn node_to_entity(node: &kuzu::NodeVal) -> Result<Entity> {
    let id = node.get_property("id").and_then(|v| v.get_str().ok()).unwrap_or("").to_string();
    let entity_type = node.get_property("entity_type").and_then(|v| v.get_str().ok()).unwrap_or("unknown").to_string();
    let label = node.get_property("label").and_then(|v| v.get_str().ok()).unwrap_or("").to_string();
    let props_str = node.get_property("properties").and_then(|v| v.get_str().ok()).unwrap_or("{}");
    let properties = serde_json::from_str(&props_str).unwrap_or_default();
    
    let created_at = node.get_property("created_at")
        .and_then(|v| v.get_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(chrono::Utc::now);
    
    let updated_at = node.get_property("updated_at")
        .and_then(|v| v.get_str().ok())
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(chrono::Utc::now);
    
    Ok(Entity {
        id,
        entity_type,
        label,
        properties,
        created_at,
        updated_at,
    })
}
