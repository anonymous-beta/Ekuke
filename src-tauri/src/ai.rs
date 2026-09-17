use crate::db::GraphDb;
use crate::entity::{Entity, Relationship};
use crate::plugin::{PluginEngine, PluginOutput};
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: Value, // string or array of content parts
}

pub struct AiClient {
    base_url: String,
    api_key: String,
    model: String,
    temperature: f32,
}

pub const SYSTEM_PROMPT: &str = r#"You are EKUKE AI, the assistant inside the EKUKE local-first OSINT investigation platform.
You help investigators run OSINT workflows on their current case graph.
You have tools to: list/search/create entities, link entities with relationships, run enrichment plugins, and summarize the case.
Rules:
- Use tools whenever the user asks you to find, add, link, or enrich anything.
- After tool calls, briefly report what you did (entity labels, relationship types, plugin results).
- Never invent data: if a plugin or search returns nothing, say so.
- This is for authorized investigative work. Be concise and technical."#;

pub fn tools_schema() -> Vec<Value> {
    vec![
        json!({
            "type": "function",
            "function": {
                "name": "list_entities",
                "description": "List entities in the current case, optionally filtered by type",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "entity_type": {"type": "string", "description": "Optional type filter, e.g. Domain, Email, IPv4, Person"}
                    }
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "search_entities",
                "description": "Search entities by label substring",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "query": {"type": "string"}
                    },
                    "required": ["query"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "create_entity",
                "description": "Create (or get existing) entity in the case graph",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "entity_type": {"type": "string"},
                        "label": {"type": "string"},
                        "properties": {"type": "object"}
                    },
                    "required": ["entity_type", "label"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "link_entities",
                "description": "Create a relationship between two entities by their labels",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "source_label": {"type": "string"},
                        "target_label": {"type": "string"},
                        "rel_type": {"type": "string", "description": "e.g. resolves_to, owned_by, registered_to, mentions"}
                    },
                    "required": ["source_label", "target_label", "rel_type"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "run_plugin",
                "description": "Run an installed enrichment plugin against an entity label",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "plugin_id": {"type": "string"},
                        "entity_label": {"type": "string"}
                    },
                    "required": ["plugin_id", "entity_label"]
                }
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "list_plugins",
                "description": "List installed plugins with their ids",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
        json!({
            "type": "function",
            "function": {
                "name": "case_summary",
                "description": "Get counts and a sample of the current case graph",
                "parameters": {"type": "object", "properties": {}}
            }
        }),
    ]
}

impl AiClient {
    pub fn new(base_url: String, api_key: String, model: String, temperature: f32) -> Self {
        Self { base_url: base_url.trim_end_matches('/').to_string(), api_key, model, temperature }
    }

    pub async fn test_connection(&self) -> Result<String> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(20))
            .build()?;
        let url = format!("{}/chat/completions", self.base_url);
        let body = json!({
            "model": self.model,
            "messages": [{"role": "user", "content": "ping"}],
            "max_tokens": 5,
        });
        let mut req = client.post(&url).json(&body);
        if !self.api_key.is_empty() {
            req = req.bearer_auth(&self.api_key);
        }
        let resp = req.send().await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("API error {}: {}", status, text));
        }
        Ok(format!("Connected OK ({}, model: {})", status, self.model))
    }

    /// Full chat with tool-calling loop. `db` and `plugins_dir` let the AI act on the case.
    pub async fn chat(
        &self,
        history: &[ChatMessage],
        db: Option<Arc<GraphDb>>,
        plugins_dir: Option<std::path::PathBuf>,
        api_keys: HashMap<String, String>,
    ) -> Result<(String, Vec<Value>)> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(180))
            .build()?;
        let url = format!("{}/chat/completions", self.base_url);

        let mut messages: Vec<Value> = vec![json!({"role": "system", "content": SYSTEM_PROMPT})];
        for m in history {
            messages.push(json!({"role": m.role, "content": m.content}));
        }

        let mut actions_log: Vec<Value> = Vec::new();
        let tools = tools_schema();

        for _round in 0..8 {
            let body = json!({
                "model": self.model,
                "messages": messages,
                "tools": tools,
                "tool_choice": "auto",
                "temperature": self.temperature,
            });
            let mut req = client.post(&url).json(&body);
            if !self.api_key.is_empty() {
                req = req.bearer_auth(&self.api_key);
            }
            let resp = req.send().await?;
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if !status.is_success() {
                return Err(anyhow!("API error {}: {}", status, text));
            }
            let v: Value = serde_json::from_str(&text)?;
            let choice = v["choices"].get(0)
                .ok_or_else(|| anyhow!("No choices in API response: {}", text))?;
            let msg = &choice["message"];

            let tool_calls: Vec<Value> = msg["tool_calls"].as_array().cloned().unwrap_or_default();

            if tool_calls.is_empty() {
                let content = msg["content"].as_str().unwrap_or("").to_string();
                return Ok((content, actions_log));
            }

            // echo assistant message including tool_calls
            messages.push(msg.clone());

            for tc in tool_calls {
                let call_id = tc["id"].as_str().unwrap_or("").to_string();
                let fname = tc["function"]["name"].as_str().unwrap_or("").to_string();
                let args_str = tc["function"]["arguments"].as_str().unwrap_or("{}").to_string();
                let args: Value = serde_json::from_str(&args_str).unwrap_or(json!({}));

                let result = execute_tool(&fname, &args, db.as_deref(), plugins_dir.as_deref(), &api_keys).await;
                let result_json = match result {
                    Ok(v) => v,
                    Err(e) => json!({"error": e.to_string()}),
                };
                actions_log.push(json!({"tool": fname, "args": args, "result": result_json}));
                messages.push(json!({
                    "role": "tool",
                    "tool_call_id": call_id,
                    "content": serde_json::to_string(&result_json)?,
                }));
            }
        }

        Err(anyhow!("Tool loop exceeded maximum rounds"))
    }
}

async fn execute_tool(
    name: &str,
    args: &Value,
    db: Option<&GraphDb>,
    plugins_dir: Option<&std::path::Path>,
    api_keys: &HashMap<String, String>,
) -> Result<Value> {
    let arg_str = |k: &str| args[k].as_str().unwrap_or("").to_string();

    match name {
        "list_entities" => {
            let db = db.ok_or_else(|| anyhow!("No case is open"))?;
            let entities = match arg_str("entity_type") {
                t if !t.is_empty() => db.search_entities_by_type(&t)?,
                _ => db.get_all_entities()?,
            };
            let brief: Vec<Value> = entities.iter().take(100).map(|e| json!({
                "id": e.id, "type": e.entity_type, "label": e.label,
            })).collect();
            Ok(json!({"count": entities.len(), "entities": brief}))
        }
        "search_entities" => {
            let db = db.ok_or_else(|| anyhow!("No case is open"))?;
            let entities = db.search_entities_by_label(&arg_str("query"), 50)?;
            let brief: Vec<Value> = entities.iter().map(|e| json!({
                "id": e.id, "type": e.entity_type, "label": e.label,
            })).collect();
            Ok(json!({"count": entities.len(), "entities": brief}))
        }
        "create_entity" => {
            let db = db.ok_or_else(|| anyhow!("No case is open"))?;
            let etype = arg_str("entity_type");
            let label = arg_str("label");
            if etype.is_empty() || label.is_empty() {
                return Err(anyhow!("entity_type and label are required"));
            }
            let entity = if let Some(id) = db.entity_exists_by_label(&etype, &label)? {
                db.get_entity_by_id(&id)?.ok_or_else(|| anyhow!("Lookup failed"))?
            } else {
                let mut e = Entity::new(&etype, &label);
                if let Some(Value::Object(map)) = args.get("properties") {
                    for (k, v) in map { e.properties.insert(k.clone(), v.clone()); }
                }
                db.insert_entity(&e)?;
                e
            };
            Ok(json!({"created": true, "id": entity.id, "type": entity.entity_type, "label": entity.label}))
        }
        "link_entities" => {
            let db = db.ok_or_else(|| anyhow!("No case is open"))?;
            let src_label = arg_str("source_label");
            let tgt_label = arg_str("target_label");
            let rel_type = arg_str("rel_type");
            // resolve labels: match any type
            let find = |label: &str| -> Result<String> {
                let all = db.get_all_entities()?;
                all.iter().find(|e| e.label.eq_ignore_ascii_case(label))
                    .map(|e| e.id.clone())
                    .ok_or_else(|| anyhow!("Entity not found: {}", label))
            };
            let src = find(&src_label)?;
            let tgt = find(&tgt_label)?;
            let rel = Relationship::new(&rel_type, &src, &tgt);
            db.add_relationship(&rel)?;
            Ok(json!({"linked": true, "source": src_label, "target": tgt_label, "rel_type": rel_type}))
        }
        "list_plugins" => {
            let dir = plugins_dir.ok_or_else(|| anyhow!("Plugins dir unavailable"))?;
            let engine = PluginEngine::new(dir);
            let manifests = engine.discover_plugins()?;
            let brief: Vec<Value> = manifests.iter().map(|m| json!({
                "id": m.id, "name": m.name, "description": m.description,
                "input_types": m.input_types,
            })).collect();
            Ok(json!({"plugins": brief}))
        }
        "run_plugin" => {
            let db = db.ok_or_else(|| anyhow!("No case is open"))?;
            let dir = plugins_dir.ok_or_else(|| anyhow!("Plugins dir unavailable"))?;
            let engine = PluginEngine::new(dir);
            let manifests = engine.discover_plugins()?;
            let plugin_id = arg_str("plugin_id");
            let manifest = manifests.iter().find(|m| m.id == plugin_id)
                .ok_or_else(|| anyhow!("Plugin '{}' not found", plugin_id))?;
            let label = arg_str("entity_label");
            let all = db.get_all_entities()?;
            let entity = all.iter().find(|e| e.label.eq_ignore_ascii_case(&label))
                .ok_or_else(|| anyhow!("Entity not found: {}", label))?;

            let mut config: HashMap<String, String> = api_keys.clone();
            let output: PluginOutput = engine.execute(manifest, entity, &config).await?;

            // merge results into graph
            let mut ids: HashMap<(String, String), String> = HashMap::new();
            ids.insert((entity.entity_type.clone(), entity.label.clone()), entity.id.clone());
            for pe in &output.entities {
                let id = if let Some(existing) = db.entity_exists_by_label(&pe.entity_type, &pe.label)? {
                    existing
                } else {
                    let mut e = Entity::new(&pe.entity_type, &pe.label);
                    e.properties = pe.properties.clone();
                    db.insert_entity(&e)?;
                    e.id
                };
                ids.insert((pe.entity_type.clone(), pe.label.clone()), id);
            }
            let mut rel_count = 0;
            for pr in &output.relationships {
                if let (Some(s), Some(t)) = (
                    ids.get(&(pr.source_type.clone(), pr.source_label.clone())),
                    ids.get(&(pr.target_type.clone(), pr.target_label.clone())),
                ) {
                    db.add_relationship(&Relationship::new(&pr.rel_type, s, t))?;
                    rel_count += 1;
                }
            }
            Ok(json!({
                "plugin": plugin_id,
                "entities_added": output.entities.len(),
                "relationships_added": rel_count,
                "entities": output.entities,
            }))
        }
        "case_summary" => {
            let db = db.ok_or_else(|| anyhow!("No case is open"))?;
            let entities = db.get_all_entities()?;
            let rels = db.get_relationships()?;
            let mut by_type: HashMap<String, i64> = HashMap::new();
            for e in &entities {
                *by_type.entry(e.entity_type.clone()).or_default() += 1;
            }
            let sample: Vec<Value> = entities.iter().take(20).map(|e| json!({
                "type": e.entity_type, "label": e.label})).collect();
            Ok(json!({
                "entity_count": entities.len(),
                "relationship_count": rels.len(),
                "by_type": by_type,
                "sample": sample,
            }))
        }
        _ => Err(anyhow!("Unknown tool: {}", name)),
    }
}