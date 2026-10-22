
use crate::entity::Entity;
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::time::timeout;

const PLUGIN_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_INPUT_BYTES: usize = 1_000_000;
const MAX_OUTPUT_BYTES: usize = 2_000_000;
const MAX_CONFIG_BYTES: usize = 64_000;

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

        let root = self
            .plugins_dir
            .canonicalize()
            .context("Could not resolve plugins directory")?;

        for entry in std::fs::read_dir(&root)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let manifest_path = path.join("manifest.json");

            if !manifest_path.is_file() {
                continue;
            }

            // Do not follow a plugin directory outside the configured root.
            let canonical_plugin = path
                .canonicalize()
                .context("Could not resolve plugin directory")?;

            if !canonical_plugin.starts_with(&root) {
                continue;
            }

            let content = std::fs::read_to_string(&manifest_path)
                .with_context(|| format!("Could not read {:?}", manifest_path))?;

            let manifest: PluginManifest = serde_json::from_str(&content)
                .with_context(|| format!("Invalid manifest: {:?}", manifest_path))?;

            validate_manifest(&manifest)?;

            manifests.push(manifest);
        }

        Ok(manifests)
    }

    pub async fn execute(
        &self,
        manifest: &PluginManifest,
        entity: &Entity,
        config: &HashMap<String, String>,
    ) -> Result<PluginOutput> {
        validate_manifest(manifest)?;

        let config_bytes = serde_json::to_vec(config)?.len();

        if config_bytes > MAX_CONFIG_BYTES {
            bail!("Plugin configuration exceeds the size limit");
        }

        let root = self
            .plugins_dir
            .canonicalize()
            .context("Could not resolve plugins directory")?;

        let plugin_dir = root.join(&manifest.id);

        let canonical_plugin = plugin_dir
            .canonicalize()
            .context("Plugin directory does not exist")?;

        if !canonical_plugin.starts_with(&root) || !canonical_plugin.is_dir() {
            bail!("Plugin directory is outside the configured plugins directory");
        }

        // Commands are intentionally not shell-interpreted. Arguments are
        // passed as individual OS arguments, not concatenated into a shell.
        let mut cmd = Command::new(&manifest.command);

        for arg in &manifest.args {
            let resolved = arg
                .replace("{input}", &entity.label)
                .replace("{type}", &entity.entity_type)
                .replace("{id}", &entity.id);

            cmd.arg(resolved);
        }

        cmd.current_dir(&canonical_plugin);
        cmd.stdin(std::process::Stdio::piped());
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());

        // If timeout drops the child future, terminate the child process.
        cmd.kill_on_drop(true);

        let mut child = cmd
            .spawn()
            .with_context(|| format!("Could not start plugin: {}", manifest.command))?;

        let input = PluginInput {
            entity: entity.clone(),
            config: config.clone(),
        };

        let input_json = serde_json::to_vec(&input)?;

        if input_json.len() > MAX_INPUT_BYTES {
            bail!("Plugin input exceeds the size limit");
        }

        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(&input_json)
                .await
                .context("Could not send input to plugin")?;

            stdin
                .shutdown()
                .await
                .context("Could not close plugin input")?;
        }

        let output = match timeout(PLUGIN_TIMEOUT, child.wait_with_output()).await {
            Ok(result) => result.context("Could not read plugin process output")?,
            Err(_) => bail!(
                "Plugin exceeded the {}-second execution limit",
                PLUGIN_TIMEOUT.as_secs()
            ),
        };

        if output.stdout.len() > MAX_OUTPUT_BYTES {
            bail!("Plugin output exceeds the size limit");
        }

        if output.stderr.len() > MAX_OUTPUT_BYTES {
            bail!("Plugin error output exceeds the size limit");
        }

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            let diagnostic: String = stderr.chars().take(4_000).collect();

            bail!("Plugin exited with an error: {}", diagnostic);
        }

        let plugin_output: PluginOutput = serde_json::from_slice(&output.stdout)
            .context("Plugin did not return valid JSON in the expected format")?;

        if let Some(err) = plugin_output.error.as_deref() {
            bail!("Plugin reported an error: {}", err);
        }

        Ok(plugin_output)
    }
}

fn validate_manifest(manifest: &PluginManifest) -> Result<()> {
    if manifest.id.is_empty()
        || manifest.id.len() > 100
        || !manifest
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("Invalid plugin ID: use letters, numbers, '-' or '_' only");
    }

    if manifest.command.trim().is_empty() {
        bail!("Plugin command cannot be empty");
    }

    if manifest.command.len() > 1_024 {
        bail!("Plugin command is too long");
    }

    if manifest.args.len() > 100 {
        bail!("Plugin has too many command arguments");
    }

    if manifest.args.iter().any(|arg| arg.len() > 8_192) {
        bail!("A plugin command argument exceeds the size limit");
    }

    if manifest.config_fields.len() > 100 {
        bail!("Plugin declares too many configuration fields");
    }

    Ok(())
}
