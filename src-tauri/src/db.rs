use crate::entity::{Entity, Relationship};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct GraphDb {
    conn: Arc<Mutex<Connection>>,
}

impl GraphDb {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path).context("Failed to open SQLite database")?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA foreign_keys = ON;

             CREATE TABLE IF NOT EXISTS entities (
                 id TEXT PRIMARY KEY,
                 entity_type TEXT NOT NULL,
                 label TEXT NOT NULL,
                 properties TEXT NOT NULL DEFAULT '{}',
                 created_at TEXT NOT NULL,
                 updated_at TEXT NOT NULL
             );

             CREATE TABLE IF NOT EXISTS relationships (
                 id TEXT PRIMARY KEY,
                 rel_type TEXT NOT NULL,
                 source_id TEXT NOT NULL,
                 target_id TEXT NOT NULL,
                 properties TEXT NOT NULL DEFAULT '{}',
                 created_at TEXT NOT NULL,
                 FOREIGN KEY(source_id) REFERENCES entities(id) ON DELETE CASCADE,
                 FOREIGN KEY(target_id) REFERENCES entities(id) ON DELETE CASCADE
             );

             CREATE INDEX IF NOT EXISTS idx_entities_type ON entities(entity_type);
             CREATE INDEX IF NOT EXISTS idx_entities_label ON entities(label);
             CREATE INDEX IF NOT EXISTS idx_rel_src ON relationships(source_id);
             CREATE INDEX IF NOT EXISTS idx_rel_tgt ON relationships(target_id);",
        )
        .context("Failed to initialize SQLite schema")?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn insert_entity(&self, entity: &Entity) -> Result<()> {
        let conn = self.conn.blocking_lock();
        let props = serde_json::to_string(&entity.properties)?;
        conn.execute(
            "INSERT INTO entities (id, entity_type, label, properties, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![entity.id, entity.entity_type, entity.label, props,
                entity.created_at.to_rfc3339(), entity.updated_at.to_rfc3339()],
        ).context("Failed to insert entity")?;
        Ok(())
    }

    pub fn update_entity(&self, entity: &Entity) -> Result<()> {
        let conn = self.conn.blocking_lock();
        let props = serde_json::to_string(&entity.properties)?;
        conn.execute(
            "UPDATE entities SET label = ?1, properties = ?2, updated_at = ?3 WHERE id = ?4",
            params![entity.label, props, entity.updated_at.to_rfc3339(), entity.id],
        ).context("Failed to update entity")?;
        Ok(())
    }

    fn row_to_entity(row: &rusqlite::Row) -> rusqlite::Result<Entity> {
        let props_str: String = row.get(3)?;
        Ok(Entity {
            id: row.get(0)?,
            entity_type: row.get(1)?,
            label: row.get(2)?,
            properties: serde_json::from_str(&props_str).unwrap_or_default(),
            created_at: row.get::<_, String>(4)?.parse().unwrap_or_else(|_| chrono::Utc::now()),
            updated_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| chrono::Utc::now()),
        })
    }

    const ENTITY_COLS: &'static str = "id, entity_type, label, properties, created_at, updated_at";

    pub fn get_entity_by_id(&self, id: &str) -> Result<Option<Entity>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM entities WHERE id = ?1", Self::ENTITY_COLS))?;
        let row = stmt.query_row([id], Self::row_to_entity).optional()?;
        Ok(row)
    }

    pub fn get_all_entities(&self) -> Result<Vec<Entity>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM entities ORDER BY entity_type, label", Self::ENTITY_COLS))?;
        let rows = stmt.query_map([], Self::row_to_entity)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn search_entities_by_label(&self, query: &str, limit: usize) -> Result<Vec<Entity>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM entities WHERE label LIKE ?1 OR entity_type LIKE ?1 LIMIT ?2",
                Self::ENTITY_COLS))?;
        let pattern = format!("%{}%", query);
        let rows = stmt.query_map(params![pattern, limit as i64], Self::row_to_entity)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn search_entities_by_type(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM entities WHERE entity_type = ?1", Self::ENTITY_COLS))?;
        let rows = stmt.query_map([entity_type], Self::row_to_entity)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn delete_entity(&self, id: &str) -> Result<()> {
        let conn = self.conn.blocking_lock();
        conn.execute("DELETE FROM entities WHERE id = ?1", [id])
            .context("Failed to delete entity")?;
        Ok(())
    }

    pub fn add_relationship(&self, rel: &Relationship) -> Result<()> {
        let conn = self.conn.blocking_lock();
        let props = serde_json::to_string(&rel.properties)?;
        conn.execute(
            "INSERT INTO relationships (id, rel_type, source_id, target_id, properties, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![rel.id, rel.rel_type, rel.source_id, rel.target_id, props,
                rel.created_at.to_rfc3339()],
        ).context("Failed to create relationship")?;
        Ok(())
    }

    fn row_to_rel(row: &rusqlite::Row) -> rusqlite::Result<Relationship> {
        let props_str: String = row.get(4)?;
        Ok(Relationship {
            id: row.get(0)?,
            rel_type: row.get(1)?,
            source_id: row.get(2)?,
            target_id: row.get(3)?,
            properties: serde_json::from_str(&props_str).unwrap_or_default(),
            created_at: row.get::<_, String>(5)?.parse().unwrap_or_else(|_| chrono::Utc::now()),
        })
    }

    const REL_COLS: &'static str = "id, rel_type, source_id, target_id, properties, created_at";

    pub fn get_relationships(&self) -> Result<Vec<Relationship>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM relationships", Self::REL_COLS))?;
        let rows = stmt.query_map([], Self::row_to_rel)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn get_relationships_for_entity(&self, entity_id: &str) -> Result<Vec<Relationship>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            &format!("SELECT {} FROM relationships WHERE source_id = ?1 OR target_id = ?1",
                Self::REL_COLS))?;
        let rows = stmt.query_map([entity_id], Self::row_to_rel)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(|e| e.into())
    }

    pub fn delete_relationship(&self, id: &str) -> Result<()> {
        let conn = self.conn.blocking_lock();
        conn.execute("DELETE FROM relationships WHERE id = ?1", [id])
            .context("Failed to delete relationship")?;
        Ok(())
    }

    pub fn entity_exists_by_label(&self, entity_type: &str, label: &str) -> Result<Option<String>> {
        let conn = self.conn.blocking_lock();
        let mut stmt = conn.prepare(
            "SELECT id FROM entities WHERE entity_type = ?1 AND label = ?2 LIMIT 1")?;
        let id: Option<String> = stmt.query_row([entity_type, label], |row| row.get(0)).optional()?;
        Ok(id)
    }

    pub fn count_entities(&self) -> Result<i64> {
        let conn = self.conn.blocking_lock();
        Ok(conn.query_row("SELECT COUNT(*) FROM entities", [], |r| r.get(0))?)
    }

    pub fn count_relationships(&self) -> Result<i64> {
        let conn = self.conn.blocking_lock();
        Ok(conn.query_row("SELECT COUNT(*) FROM relationships", [], |r| r.get(0))?)
    }
}