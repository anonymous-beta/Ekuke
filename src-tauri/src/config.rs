use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_author: String,
    pub plugins_dir: PathBuf,
    pub cases_dir: PathBuf,
    #[serde(default)]
    pub api_keys: std::collections::HashMap<String, String>,
    #[serde(default)]
    pub ui_theme: String,
    #[serde(default)]
    pub proxy_enabled: bool,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default)]
    pub tor_enabled: bool,
}

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let ekuke_dir = home.join(".ekuke");
        
        Self {
            default_author: "Anonymous-beta".to_string(),
            plugins_dir: ekuke_dir.join("plugins"),
            cases_dir: ekuke_dir.join("cases"),
            api_keys: std::collections::HashMap::new(),
            ui_theme: "dark".to_string(),
            proxy_enabled: false,
            proxy_url: String::new(),
            tor_enabled: false,
        }
    }
}

impl Config {
    pub fn load_or_default() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str(&content) {
                    return config;
                }
            }
        }
        let config = Self::default();
        let _ = config.save();
        config
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    fn config_path() -> PathBuf {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        home.join(".ekuke").join("config.json")
    }
}
