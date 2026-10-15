//! Built-in collection & enrichment engine (no external plugins needed).
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use md5::{Digest, Md5};
use regex::Regex;
use serde_json::{json, Value};
use tauri::State;
use tokio::net::lookup_host;
use crate::ai::{AiClient, ChatMessage};
use crate::collect::Collector;
use crate::db::GraphDb;
use crate::entity::{Entity, Relationship};
use crate::AppState;

type Cmd<T> = Result<T, String>;
const UA: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
(KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36 EKUKE/0.2";

struct FetchedPage { status: u16, final_url: String, server: String, content_type: String,
    title: String, description: String, links: Vec<String>, text: String }

fn http_client() -> Cmd<reqwest::Client> {
    reqwest::Client::builder().timeout(Duration::from_secs(25))
        .connect_timeout(Duration::from_secs(10)).user_agent(UA).build().map_err(|e| e.to_string())
}
async fn require_db(state: &State<'_, AppState>) -> Cmd<Arc<GraphDb>> {
    state.db.lock().await.clone().ok_or_else(|| "No case is open. Create or open a case first.".to_string())
}
fn normalize_url(raw: &str) -> Cmd<String> {
    let mut u = raw.trim().to_string();
    if u.is_empty() { return Err("URL is required".into()); }
    if !u.contains("://") { u = format!("https://{}", u); }
    let parsed = reqwest::Url::parse(&u).map_err(|e| format!("Invalid URL: {}", e))?;
    match parsed.host_str() {
        Some(h) if h.contains('.') || h == "localhost" => Ok(u),
        _ => Err(format!("URL has no valid host: {}", u)),
    }
}
fn host_of(url: &str) -> Option<String> {
    reqwest::Url::parse(url).ok().and_then(|u| u.host_str().map(|h| h.to_lowercase()))
}
fn extract_title(html: &str) -> String {
    Regex::new(r"(?is)<title[^>]*>(.*?)</title>").ok()
        .and_then(|re| re.captures(html))
        .map(|c| c.get(1).map(|m| m.as_str()).unwrap_or("").trim().to_string())
        .unwrap_or_default().chars().take(300).collect()
}
fn extract_meta_description(html: &str) -> String {
    Regex::new(r#"(?is)<meta[^>]+name=["']description["'][^>]+content=["']([^"']*)"#).ok()
        .and_then(|re| re.captures(html))
        .map(|c| c.get(1).map(|m| m.as_str()).unwrap_or("").trim().to_string())
        .unwrap_or_default().chars().take(500).collect()
}
fn strip_html(html: &str) -> String {
    let s = Regex::new(r"(?is)<script[^>]*>.*?</script>").unwrap().replace_all(html, " ");
    let s = Regex::new(r"(?is)<style[^>]*>.*?</style>").unwrap().replace_all(&s, " ");
    let s = Regex::new(r"(?s)<[^>]+>").unwrap().replace_all(&s, " ");
    s.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">")
        .replace("&quot;", "\"").replace("&#39;", "'").replace("&nbsp;", " ")
        .split_whitespace().collect::<Vec<_>>().join(" ")
}
fn extract_links(html: &str, base: &str) -> Vec<String> {
    let re = match Regex::new(r#"(?i)href\s*=\s*["']([^"'#]+)"#) { Ok(r) => r, Err(_) => return vec![] };
    let base_url = reqwest::Url::parse(base).ok();
    let mut seen = HashSet::new(); let mut out = Vec::new();
    for cap in re.captures_iter(html) {
        if let Some(m) = cap.get(1) {
            let href = m.as_str().trim();
            if href.is_empty() || href.starts_with("javascript:") || href.starts_with("mailto:") || href.starts_with("tel:") { continue; }
            let abs = base_url.as_ref().and_then(|b| b.join(href).ok()).map(|u| u.to_string()).unwrap_or_else(|| href.to_string());
            if seen.insert(abs.clone()) && out.len() < 150 { out.push(abs); }
        }
    }
    out
}
async fn do_fetch(url: String) -> Cmd<FetchedPage> {
    let url = normalize_url(&url)?;
    let resp = http_client()?.get(&url).send().await.map_err(|e| format!("Request failed: {}", e))?;
    let status = resp.status().as_u16();
    let final_url = resp.url().to_string();
    let header = |n: &str| resp.headers().get(n).and_then(|v| v.to_str().ok()).unwrap_or("").to_string();
    let (server, content_type) = (header("server"), header("content-type"));
    let mut body = resp.text().await.map_err(|e| e.to_string())?;
    if body.len() > 1_500_000 { body.truncate(1_500_000); }
    let (title, description) = (extract_title(&body), extract_meta_description(&body));
    let links = extract_links(&body, &final_url);
    let text = strip_html(&body);
    Ok(FetchedPage { status, final_url, server, content_type, title, description, links, text })
}
fn upsert(db: &GraphDb, t: &str, label: &str, props: HashMap<String, Value>) -> Cmd<Entity> {
    if let Some(id) = db.entity_exists_by_label(t, label).map_err(|e| e.to_string())? {
        let mut e = db.get_entity_by_id(&id).map_err(|e| e.to_string())?.ok_or("Entity vanished during upsert")?;
        for (k, v) in props { e.properties.insert(k, v); }
        e.updated_at = chrono::Utc::now();
        db.update_entity(&e).map_err(|e| e.to_string())?;
        Ok(e)
    } else {
        let mut e = Entity::new(t, label);
        e.properties = props;
        db.insert_entity(&e).map_err(|e| e.to_string())?;
        Ok(e)
    }
}
fn link(db: &GraphDb, s: &Entity, t: &Entity, rel: &str) -> Cmd<()> {
    db.add_relationship(&Relationship::new(rel, &s.id, &t.id)).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn fetch_url(url: String) -> Cmd<Value> {
    let page = do_fetch(url).await?;
    let found = Collector::new().extract_entities_from_text(&page.text).map_err(|e| e.to_string())?;
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut samples: HashMap<String, Vec<String>> = HashMap::new();
    for e in &found {
        *counts.entry(e.entity_type.clone()).or_default() += 1;
        let s = samples.entry(e.entity_type.clone()).or_default();
        if s.len() < 10 { s.push(e.label.clone()); }
    }
    Ok(json!({ "status": page.status, "final_url": page.final_url, "server": page.server,
        "content_type": page.content_type, "title": page.title, "description": page.description,
        "links": page.links, "entity_counts": counts, "entity_samples": samples,
        "text_preview": page.text.chars().take(800).collect::<String>() }))
}

#[tauri::command]
pub async fn scrape_and_import(state: State<'_, AppState>, url: String) -> Cmd<Value> {
    let db = require_db(&state).await?;
    let page = do_fetch(url.clone()).await?;
    let normalized = normalize_url(&url)?;
    let mut props = HashMap::new();
    props.insert("final_url".into(), page.final_url.clone().into());
    props.insert("http_status".into(), page.status.into());
    props.insert("title".into(), page.title.clone().into());
    props.insert("server".into(), page.server.clone().into());
    props.insert("content_type".into(), page.content_type.clone().into());
    if !page.description.is_empty() { props.insert("description".into(), page.description.clone().into()); }
    props.insert("scraped_at".into(), chrono::Utc::now().to_rfc3339().into());
    let page_entity = upsert(&db, "URL", &normalized, props)?;
    let mut (added, rels) = (0usize, 0usize);
    let host = host_of(&page.final_url);
    let found = Collector::new().extract_entities_from_text(&page.text).map_err(|e| e.to_string())?;
    for e in found {
        let is_new = db.entity_exists_by_label(&e.entity_type, &e.label).map_err(|er| er.to_string())?.is_none();
        let saved = upsert(&db, &e.entity_type, &e.label, e.properties.clone())?;
        if is_new { added += 1; }
        let rt = match e.entity_type.as_str() {
            "Domain" => match &host { Some(h) if e.label == *h || e.label.ends_with(&format!(".{}", h)) => "serves", _ => "links_to" },
            "IPv4" => "hosts", "Email" | "Phone" | "Handle" => "mentions", _ => "linked_to" };
        link(&db, &page_entity, &saved, rt)?; rels += 1;
    }
    for l in &page.links {
        if let Some(d) = host_of(l) {
            if Some(&d) != host.as_ref() && d.contains('.') {
                let is_new = db.entity_exists_by_label("Domain", &d).map_err(|er| er.to_string())?.is_none();
                let de = upsert(&db, "Domain", &d, HashMap::new())?;
                if is_new { added += 1; }
                link(&db, &page_entity, &de, "links_to")?; rels += 1;
            }
        }
    }
    Ok(json!({ "status": page.status, "final_url": page.final_url, "title": page.title,
        "server": page.server, "entities_added": added, "relationships_added": rels, "links_found": page.links.len() }))
}

#[tauri::command]
pub async fn enrich_entity(state: State<'_, AppState>, id: String) -> Cmd<Value> {
    let db = require_db(&state).await?;
    let cfg = state.config.lock().await.clone();
    let mut entity = db.get_entity_by_id(&id).map_err(|e| e.to_string())?.ok_or("Entity not found")?;
    let mut notes: Vec<String> = Vec::new();
    let (mut ec, mut rc, mut pc) = (0usize, 0usize, 0usize);
    macro_rules! sp { ($k:expr,$v:expr) => {{ entity.properties.insert($k.to_string(), $v); pc += 1; }}; }
    macro_rules! uc { ($t:expr,$l:expr,$p:expr) => {{ let n = db.entity_exists_by_label($t,$l).map_err(|e|e.to_string())?.is_none(); let x = upsert(&db,$t,$l,$p)?; if n { ec += 1; } x }}; }

    match entity.entity_type.as_str() {
        "Domain" => {
            let domain = entity.label.trim().to_lowercase();
            match lookup_host(format!("{}:443", domain)).await {
                Ok(addrs) => {
                    let ips: HashSet<String> = addrs.map(|a| a.ip().to_string()).collect();
                    for ip in &ips { let ie = uc!("IPv4", ip, HashMap::new()); link(&db, &entity, &ie, "resolves_to")?; rc += 1; }
                    sp!("dns_addresses", ips.iter().cloned().collect::<Vec<_>>().into());
                    notes.push(format!("DNS: {} address(es)", ips.len()));
                } Err(e) => notes.push(format!("DNS lookup failed: {}", e)),
            }
            match rdap_lookup(&http_client()?, &domain).await {
                Ok((mut rp, registrar, registrant)) => {
                    for (k, v) in rp.drain() { sp!(k, v); }
                    if let Some(reg) = registrar {
                        let oe = uc!("Organization", &reg, HashMap::new());
                        link(&db, &entity, &oe, "registrar")?; rc += 1;
                        notes.push(format!("Registrar: {}", reg));
                    }
                    if let Some((org, person)) = registrant {
                        if let Some(o) = org { let oe = uc!("Organization", &o, HashMap::new()); link(&db, &entity, &oe, "registered_to")?; rc += 1; notes.push(format!("Registrant: {}", o)); }
                        if let Some(p) = person { let pe = uc!("Person", &p, HashMap::new()); link(&db, &entity, &pe, "registered_to")?; rc += 1; }
                    }
                } Err(e) => notes.push(format!("RDAP failed: {}", e)),
            }
            match crtsh_subdomains(&http_client()?, &domain).await {
                Ok(subs) => { let n = subs.len();
                    for s in subs { let de = uc!("Domain", &s, HashMap::new()); link(&db, &de, &entity, "subdomain_of")?; rc += 1; }
                    sp!("ct_subdomain_count", n.into()); notes.push(format!("crt.sh: {} subdomain(s)", n));
                } Err(e) => notes.push(format!("crt.sh failed: {}", e)),
            }
            match do_fetch(format!("https://{}", domain)).await {
                Ok(p) => { sp!("http_status", p.status.into());
                    if !p.server.is_empty() { sp!("server", p.server.clone().into()); }
                    if !p.title.is_empty() { sp!("title", p.title.clone().into()); }
                    notes.push(format!("HTTP probe: {}", p.status)); }
                Err(e) => notes.push(format!("HTTP probe failed: {}", e)),
            }
        }
        "IPv4" => {
            let ip = entity.label.trim().to_string();
            if ip.parse::<std::net::Ipv4Addr>().is_err() { return Err(format!("'{}' is not a valid IPv4", ip)); }
            match ip_api_lookup(&http_client()?, &ip).await {
                Ok(mut p) => {
                    let (isp, org_name, reverse) = (p.remove("isp"), p.remove("org"), p.remove("reverse"));
                    for (k, v) in p.drain() { sp!(format!("geoip.{}", k), v); }
                    for name in [isp, org_name].into_iter().flatten() {
                        if !name.is_empty() && name != "-" { let oe = uc!("Organization", &name, HashMap::new()); link(&db, &entity, &oe, "announced_by")?; rc += 1; }
                    }
                    if let Some(r) = reverse { sp!("reverse_dns", r.clone().into()); notes.push(format!("Reverse DNS: {}", r)); }
                    notes.push("GeoIP retrieved (ip-api.com)".into());
                } Err(e) => notes.push(format!("ip-api failed: {}", e)),
            }
            match cfg.api_keys.get("shodan") {
                Some(key) => match shodan_lookup(&http_client()?, &ip, key).await {
                    Ok(mut p) => {
                        let hostnames: Vec<String> = p.remove("hostnames").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                        let vulns: Vec<String> = p.remove("vulns").and_then(|v| serde_json::from_value(v).ok()).unwrap_or_default();
                        for (k, v) in p.drain() { sp!(format!("shodan.{}", k), v); }
                        for h in hostnames.iter().take(50) { let de = uc!("Domain", h, HashMap::new()); link(&db, &de, &entity, "hosted_on")?; rc += 1; }
                        if !vulns.is_empty() { sp!("shodan.vulns", vulns.clone().into()); notes.push(format!("Shodan: {} CVE(s)", vulns.len())); }
                        notes.push(format!("Shodan: {} hostnames", hostnames.len()));
                    } Err(e) => notes.push(format!("Shodan failed: {}", e)),
                },
                None => notes.push("Shodan skipped: add 'shodan' key in Settings → API Keys".into()),
            }
        }
        "Email" => {
            let email = entity.label.trim().to_lowercase();
            let hash = format!("{:x}", Md5::digest(email.as_bytes()));
            match gravatar_lookup(&http_client()?, &hash).await {
                Ok(Some((name, loc, url))) => {
                    sp!("gravatar.name", name.clone().into());
                    if let Some(l) = loc { sp!("gravatar.location", l.into()); }
                    if let Some(u) = url { sp!("gravatar.profile", u.into()); }
                    let pe = uc!("Person", &name, HashMap::new());
                    link(&db, &entity, &pe, "belongs_to")?; rc += 1;
                    notes.push(format!("Gravatar found: {}", name));
                } Ok(None) => notes.push("Gravatar: no public profile".into()),
                Err(e) => notes.push(format!("Gravatar failed: {}", e)),
            }
            match cfg.api_keys.get("hibp") {
                Some(key) => match hibp_lookup(&http_client()?, &email, key).await {
                    Ok(n) => { sp!("breaches", n.into()); notes.push(format!("HIBP: {} breach(es)", n)); }
                    Err(e) => notes.push(format!("HIBP failed: {}", e)),
                },
                None => notes.push("HIBP skipped: add key named 'hibp' in Settings → API Keys".into()),
            }
        }
        "URL" => {
            let page = do_fetch(entity.label.clone()).await?;
            sp!("final_url", page.final_url.clone().into());
            sp!("http_status", page.status.into());
            if !page.title.is_empty() { sp!("title", page.title.clone().into()); }
            if !page.server.is_empty() { sp!("server", page.server.clone().into()); }
            let found = Collector::new().extract_entities_from_text(&page.text).map_err(|e| e.to_string())?;
            for e2 in found { let se = uc!(&e2.entity_type, &e2.label, e2.properties.clone()); link(&db, &entity, &se, "mentions")?; rc += 1; }
            notes.push(format!("Scraped '{}': {} entities linked", page.final_url, rc));
        }
        "Handle" => {
            let h = entity.label.trim().trim_start_matches('@').to_string();
            let sites = vec![("GitHub", format!("https://github.com/{}", h)), ("X", format!("https://x.com/{}", h)),
                ("Instagram", format!("https://www.instagram.com/{}/", h)), ("TikTok", format!("https://www.tiktok.com/@{}", h)),
                ("Reddit", format!("https://www.reddit.com/user/{}", h))];
            let client = http_client()?;
            for (name, u) in sites {
                match client.get(&u).send().await {
                    Ok(r) => { let f = r.status().is_success(); sp!(format!("social.{}", name.to_lowercase()), f.into());
                        if f { notes.push(format!("{}: profile likely exists (HTTP {})", name, r.status().as_u16())); } }
                    Err(e) => notes.push(format!("{}: check failed ({})", name, e)),
                }
            }
            notes.push("Note: some sites return 200 for soft-404s — verify manually.".into());
        }
        other => return Err(format!("No built-in enrichment for type '{}'. Try AI or plugins.", other)),
    }
    entity.updated_at = chrono::Utc::now();
    db.update_entity(&entity).map_err(|e| e.to_string())?;
    Ok(json!({ "entity_id": entity.id, "properties_added": pc, "entities_created": ec,
        "relationships_created": rc, "notes": notes }))
}

#[tauri::command]
pub async fn ai_summarize_case(state: State<'_, AppState>) -> Cmd<String> {
    let cfg = state.config.lock().await.clone();
    if !cfg.ai_enabled { return Err("AI assistant is disabled. Enable it in Settings.".into()); }
    if cfg.ai_api_key.is_empty() && !cfg.ai_base_url.contains("localhost") && !cfg.ai_base_url.contains("127.0.0.1") {
        return Err("No AI API key configured. Set one in Settings.".into());
    }
    let db = require_db(&state).await?;
    let entities = db.get_all_entities().map_err(|e| e.to_string())?;
    let rels = db.get_relationships().map_err(|e| e.to_string())?;
    let by_id: HashMap<String, (&str, &str)> = entities.iter().map(|e| (e.id.clone(), (&*e.entity_type, &*e.label))).collect();
    let summary = json!({
        "entity_count": entities.len(), "relationship_count": rels.len(),
        "entities": entities.iter().map(|e| json!({"type": e.entity_type, "label": e.label,
            "properties": e.properties.keys().cloned().collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "relationships": rels.iter().map(|r| json!({"type": r.rel_type,
            "source": by_id.get(&r.source_id).map(|t| t.1).unwrap_or("?"),
            "target": by_id.get(&r.target_id).map(|t| t.1).unwrap_or("?")})).collect::<Vec<_>>(),
    });
    let client = AiClient::new(cfg.ai_base_url.clone(), cfg.ai_api_key.clone(), cfg.ai_model.clone(), cfg.ai_temperature);
    let msg = ChatMessage { role: "user".into(), content: json!(format!(
        "You are an OSINT analyst. Write a concise structured summary of this case: key entities, infrastructure, notable links, next steps.\n\nCASE DATA:\n{}",
        serde_json::to_string_pretty(&summary).unwrap_or_default())) };
    let (reply, _) = client.chat(&[msg], None, None, cfg.api_keys.clone()).await.map_err(|e| e.to_string())?;
    Ok(reply)
}

#[tauri::command]
pub async fn entity_neighborhood(state: State<'_, AppState>, id: String) -> Cmd<Value> {
    let db = require_db(&state).await?;
    let entity = db.get_entity_by_id(&id).map_err(|e| e.to_string())?.ok_or("Entity not found")?;
    let rels = db.get_relationships_for_entity(&id).map_err(|e| e.to_string())?;
    let all = db.get_all_entities().map_err(|e| e.to_string())?;
    let by_id: HashMap<String, (&str, &str)> = all.iter().map(|e| (e.id.clone(), (e.entity_type.as_str(), e.label.as_str()))).collect();
    let neighbors: Vec<Value> = rels.iter().map(|r| {
        let outgoing = r.source_id == id;
        let other = if outgoing { &r.target_id } else { &r.source_id };
        let (ot, ol) = by_id.get(other).copied().unwrap_or(("?", "?"));
        json!({ "rel_id": r.id, "rel_type": r.rel_type, "direction": if outgoing { "out" } else { "in" },
            "other_id": other, "other_label": ol, "other_type": ot })
    }).collect();
    Ok(json!({ "entity": serde_json::to_value(&entity).map_err(|e| e.to_string())?, "neighbors": neighbors }))
}

async fn rdap_lookup(client: &reqwest::Client, domain: &str)
    -> Cmd<(HashMap<String, Value>, Option<String>, Option<(Option<String>, Option<String>)>)> {
    let resp = client.get(format!("https://rdap.org/domain/{}", domain)).send().await
        .map_err(|e| format!("RDAP request failed: {}", e))?;
    if !resp.status().is_success() { return Err(format!("RDAP returned HTTP {}", resp.status())); }
    let v: Value = resp.json().await.map_err(|e| format!("RDAP JSON parse failed: {}", e))?;
    let mut props = HashMap::new();
    if let Some(events) = v["events"].as_array() {
        for ev in events {
            let (a, d) = (ev["eventAction"].as_str().unwrap_or(""),
                ev["eventDate"].as_str().unwrap_or("").chars().take(10).collect::<String>());
            match a { "registration" => { props.insert("created".into(), d.into()); }
                "expiration" => { props.insert("expires".into(), d.into()); }
                "last changed" => { props.insert("updated".into(), d.into()); } _ => {} }
        }
    }
    if let Some(ds) = v["secureDNS"].get("delegationSigned").and_then(|b| b.as_bool()) {
        props.insert("dnssec".into(), ds.to_string().into());
    }
    let (mut registrar, mut registrant) = (None, None);
    if let Some(ents) = v["entities"].as_array() {
        for en in ents {
            let roles: Vec<&str> = en["roles"].as_array()
                .map(|r| r.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
            let (f, o) = parse_vcard(&en["vcardArray"]);
            if roles.contains(&"registrar") { registrar = o.or(f); }
            if roles.contains(&"registrant") || roles.contains(&"administrative") { registrant = Some((o.clone(), f.clone())); }
        }
    }
    Ok((props, registrar, registrant))
}
fn parse_vcard(v: &Value) -> (Option<String>, Option<String>) {
    let (mut f, mut o) = (None, None);
    if let Some(entries) = v.get(1).and_then(|a| a.as_array()) {
        for e in entries {
            if let Some(kind) = e.get(0).and_then(|k| k.as_str()) {
                let val = e.get(3).cloned().unwrap_or(Value::Null);
                let s = val.as_str().map(|x| x.to_string())
                    .or_else(|| val.get(0).and_then(|x| x.as_str()).map(|x| x.to_string()));
                match kind { "fn" => f = s, "org" => o = s, _ => {} }
            }
        }
    }
    (f, o)
}
async fn crtsh_subdomains(client: &reqwest::Client, domain: &str) -> Cmd<Vec<String>> {
    let resp = client.get(format!("https://crt.sh/?q=%25.{}&output=json", domain))
        .timeout(Duration::from_secs(45)).send().await
        .map_err(|e| format!("crt.sh request failed: {}", e))?;
    if !resp.status().is_success() { return Err(format!("crt.sh returned HTTP {}", resp.status())); }
    let arr: Value = resp.json().await.map_err(|e| format!("crt.sh JSON parse failed: {}", e))?;
    let mut set: HashSet<String> = HashSet::new();
    if let Some(items) = arr.as_array() {
        for it in items {
            if let Some(nv) = it["name_value"].as_str() {
                for n in nv.lines() {
                    let n = n.trim().trim_start_matches("*.").to_lowercase();
                    if n == domain || n.ends_with(&format!(".{}", domain)) { set.insert(n); }
                }
            }
        }
    }
    let mut out: Vec<String> = set.into_iter().collect();
    out.sort(); out.truncate(200);
    Ok(out)
}
async fn ip_api_lookup(client: &reqwest::Client, ip: &str) -> Cmd<HashMap<String, Value>> {
    let v: Value = client.get(format!(
        "http://ip-api.com/json/{}?fields=status,message,country,regionName,city,zip,lat,lon,timezone,isp,org,asname,reverse,proxy,hosting", ip))
        .send().await.map_err(|e| format!("ip-api failed: {}", e))?
        .json().await.map_err(|e| format!("ip-api JSON failed: {}", e))?;
    if v["status"] != "success" { return Err(format!("ip-api error: {}", v["message"].as_str().unwrap_or("unknown"))); }
    let mut out = HashMap::new();
    for k in ["country","regionName","city","zip","timezone","isp","org","asname","reverse","proxy","hosting"] {
        if let Some(val) = v.get(k) { out.insert(k.to_string(), val.clone()); }
    }
    Ok(out)
}
async fn shodan_lookup(client: &reqwest::Client, ip: &str, key: &str) -> Cmd<HashMap<String, Value>> {
    let v: Value = client.get(format!("https://api.shodan.io/shodan/host/{}?key={}", ip, key))
        .send().await.map_err(|e| format!("Shodan failed: {}", e))?
        .json().await.map_err(|e| format!("Shodan JSON failed: {}", e))?;
    if let Some(err) = v["error"].as_str() { return Err(format!("Shodan error: {}", err)); }
    let mut out = HashMap::new();
    if let Some(p) = v["ports"].as_array() { out.insert("ports".into(), json!(p.iter().filter_map(|x| x.as_u64()).collect::<Vec<_>>())); }
    if let Some(h) = v["hostnames"].as_array() { out.insert("hostnames".into(), json!(h.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>())); }
    if let Some(u) = v["vulns"].as_object() { out.insert("vulns".into(), json!(u.keys().cloned().collect::<Vec<_>>())); }
    if let Some(i) = v["isp"].as_str() { out.insert("isp".into(), i.into()); }
    if let Some(o) = v["os"].as_str() { out.insert("os".into(), o.into()); }
    Ok(out)
}
async fn gravatar_lookup(client: &reqwest::Client, hash: &str) -> Cmd<Option<(String, Option<String>, Option<String>)>> {
    let resp = client.get(format!("https://www.gravatar.com/{}.json", hash)).send().await
        .map_err(|e| format!("Gravatar failed: {}", e))?;
    if resp.status().as_u16() == 404 { return Ok(None); }
    if !resp.status().is_success() { return Err(format!("Gravatar HTTP {}", resp.status())); }
    let v: Value = resp.json().await.map_err(|e| e.to_string())?;
    let e0 = &v["entry"][0];
    let name = e0["displayName"].as_str().unwrap_or("").to_string();
    if name.is_empty() { return Ok(None); }
    Ok(Some((name, e0["location"].as_str().map(|s| s.to_string()),
        e0["profileUrl"].as_str().map(|s| s.to_string()))))
}
async fn hibp_lookup(client: &reqwest::Client, email: &str, key: &str) -> Cmd<usize> {
    let resp = client.get(format!("https://haveibeenpwned.com/api/v3/breachedaccount/{}?truncateResponse=true", email))
        .header("hibp-api-key", key).header("User-Agent", "EKUKE-OSINT").send().await
        .map_err(|e| format!("HIBP failed: {}", e))?;
    match resp.status().as_u16() {
        200 => { let arr: Value = resp.json().await.map_err(|e| e.to_string())?;
            Ok(arr.as_array().map(|a| a.len()).unwrap_or(0)) }
        404 => Ok(0),
        s => Err(format!("HIBP returned HTTP {}", s)),
    }
}