use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub default_author: String,
    pub plugins_dir: PathBuf,
    pub cases_dir: PathBuf,
    #[serde(default)]
    pub api_keys: HashMap<String, String>,
    #[serde(default)]
    pub ui_theme: String,
    #[serde(default)]
    pub proxy_enabled: bool,
    #[serde(default)]
    pub proxy_url: String,
    #[serde(default)]
    pub tor_enabled: bool,
    // ── AI Assistant settings ──
    #[serde(default = "default_ai_base_url")]
    pub ai_base_url: String,
    #[serde(default)]
    pub ai_api_key: String,
    #[serde(default = "default_ai_model")]
    pub ai_model: String,
    #[serde(default = "default_ai_temperature")]
    pub ai_temperature: f32,
    #[serde(default)]
    pub ai_enabled: bool,
}

fn default_ai_base_url() -> String { "https://api.openai.com/v1".to_string() }
fn default_ai_model() -> String { "gpt-4o-mini".to_string() }
fn default_ai_temperature() -> f32 { 0.2 }

impl Default for Config {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let ekuke_dir = home.join(".ekuke");
        Self {
            default_author: "Anonymous-beta".to_string(),
            plugins_dir: ekuke_dir.join("plugins"),
            cases_dir: ekuke_dir.join("cases"),
            api_keys: HashMap::new(),
            ui_theme: "dark".to_string(),
            proxy_enabled: false,
            proxy_url: String::new(),
            tor_enabled: false,
            ai_base_url: default_ai_base_url(),
            ai_api_key: String::new(),
            ai_model: default_ai_model(),
            ai_temperature: default_ai_temperature(),
            ai_enabled: false,
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