use crate::entity::Entity;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub author: String,
    pub description: String,
    pub input_types: Vec<String>,
    pub output_types: Vec<String>,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub config_fields: Vec<ConfigField>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigField {
    pub name: String,
    pub field_type: String,
    #[serde(default)]
    pub default: Option<String>,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginInput {
    pub entity: Entity,
    pub config: HashMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginOutput {
    #[serde(default)]
    pub entities: Vec<PartialEntity>,
    #[serde(default)]
    pub relationships: Vec<PartialRelationship>,
    #[serde(default)]
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialEntity {
    pub entity_type: String,
    pub label: String,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PartialRelationship {
    pub rel_type: String,
    pub source_label: String,
    pub source_type: String,
    pub target_label: String,
    pub target_type: String,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

pub struct PluginEngine {
    plugins_dir: PathBuf,
}

impl PluginEngine {
    pub fn new<P: AsRef<Path>>(plugins_dir: P) -> Self {
        Self {
            plugins_dir: plugins_dir.as_ref().to_path_buf(),
        }
    }

    pub fn discover_plugins(&self) -> Result<Vec<PluginManifest>> {
        let mut manifests = Vec::new();
        
        if !self.plugins_dir.exists() {
            return Ok(manifests);
        }
        
        for entry in std::fs::read_dir(&self.plugins_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("manifest.json");
                if manifest_path.exists() {
                    let content = std::fs::read_to_string(&manifest_path)?;
                    let manifest: PluginManifest = serde_json::from_str(&content)
                        .with_context(|| format!("Failed to parse manifest: {:?}", manifest_path))?;
                    manifests.push(manifest);
                }
            }
        }
        
        Ok(manifests)
    }

    pub async fn execute(
        &self,
        manifest: &PluginManifest,
        entity: &Entity,
        config: &HashMap<String, String>,
    ) -> Result<PluginOutput> {
        let plugin_dir = self.plugins_dir.join(&manifest.id);
        
        let mut cmd = Command::new(&manifest.command);
        for arg in &manifest.args {
            let resolved = arg.replace("{input}", &entity.label)
                .replace("{type}", &entity.entity_type)
                .replace("{id}", &entity.id);
            cmd.arg(resolved);
        }
        
        cmd.current_dir(&plugin_dir);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        
        let mut child = cmd.spawn()
            .with_context(|| format!("Failed to spawn plugin: {}", manifest.command))?;
        
        let input = PluginInput {
            entity: entity.clone(),
            config: config.clone(),
        };
        
        let input_json = serde_json::to_string(&input)?;
        
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(input_json.as_bytes()).await?;
            stdin.shutdown().await?;
        }
        
        let output = child.wait_with_output().await
            .context("Failed to read plugin output")?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("Plugin failed: {}", stderr);
        }
        
        let plugin_output: PluginOutput = serde_json::from_slice(&output.stdout)
            .with_context(|| {
                let stdout = String::from_utf8_lossy(&output.stdout);
                format!("Plugin returned invalid JSON. stdout: {}", stdout)
            })?;
        
        if let Some(err) = plugin_output.error {
            anyhow::bail!("Plugin error: {}", err);
        }
        
        Ok(plugin_output)
    }
}
