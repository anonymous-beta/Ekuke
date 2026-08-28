use crate::entity::Entity;
use anyhow::Result;
use regex::Regex;

pub struct Collector;

impl Collector {
    pub fn new() -> Self {
        Self
    }

    pub fn extract_entities_from_text(&self, text: &str) -> Result<Vec<Entity>> {
        let mut entities = Vec::new();
        
        let email_re = Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap();
        for mat in email_re.find_iter(text) {
            let mut e = Entity::new("Email", mat.as_str());
            let start = mat.start().saturating_sub(20);
            let end = mat.end().min(text.len());
            e.properties.insert("source_text".to_string(), text[start..end].to_string().into());
            entities.push(e);
        }
        
        let ip_re = Regex::new(r"\b(?:[0-9]{1,3}\.){3}[0-9]{1,3}\b").unwrap();
        for mat in ip_re.find_iter(text) {
            if mat.as_str().split('.').all(|o| o.parse::<u8>().is_ok()) {
                let mut e = Entity::new("IPv4", mat.as_str());
                e.properties.insert("source_text".to_string(), mat.as_str().to_string().into());
                entities.push(e);
            }
        }
        
        let domain_re = Regex::new(r"\b(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}\b").unwrap();
        for mat in domain_re.find_iter(text) {
            let s = mat.as_str();
            if !s.contains('@') && !ip_re.is_match(s) {
                let mut e = Entity::new("Domain", s);
                e.properties.insert("source_text".to_string(), s.to_string().into());
                entities.push(e);
            }
        }
        
        let phone_re = Regex::new(r"\+?[1-9]\d{1,14}(?:[-\s]?\d{2,4}){2,4}").unwrap();
        for mat in phone_re.find_iter(text) {
            let mut e = Entity::new("Phone", mat.as_str());
            e.properties.insert("source_text".to_string(), mat.as_str().to_string().into());
            entities.push(e);
        }
        
        let handle_re = Regex::new(r"@[a-zA-Z0-9_]{3,32}").unwrap();
        for mat in handle_re.find_iter(text) {
            let mut e = Entity::new("Handle", mat.as_str());
            e.properties.insert("source_text".to_string(), mat.as_str().to_string().into());
            entities.push(e);
        }
        
        Ok(entities)
    }
}
