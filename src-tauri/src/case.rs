use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub version: String,
}

impl CaseMetadata {
    pub fn new(name: &str, author: &str) -> Self {
        let now = Utc::now();
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            name: name.to_string(),
            description: String::new(),
            author: author.to_string(),
            created_at: now,
            updated_at: now,
            version: "0.1.0".to_string(),
        }
    }
}

pub struct CasePaths {
    pub root: PathBuf,
    pub db: PathBuf,
    pub search: PathBuf,
    pub attachments: PathBuf,
}

impl CasePaths {
    pub fn from_root<P: AsRef<std::path::Path>>(root: P) -> Self {
        let root = root.as_ref().to_path_buf();
        Self {
            db: root.join("data").join("ekuke.db"),
            search: root.join("search.idx"),
            attachments: root.join("attachments"),
            root,
        }
    }

    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(&self.search)?;
        std::fs::create_dir_all(&self.attachments)?;
        if let Some(parent) = self.db.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(())
    }
}
