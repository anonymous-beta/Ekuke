use std::fs;
use std::path::Path;
use anyhow::Result;

/// Extracts text content from a file based on its extension
pub fn extract_text_from_file(path: &Path) -> Result<Option<String>> {
    let ext = path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    let content = match ext.as_str() {
        "txt" | "md" | "log" | "csv" | "json" | "xml" | "yaml" | "yml" => {
            fs::read_to_string(path)?
        }
        "rs" | "py" | "js" | "ts" | "html" | "css" | "c" | "cpp" | "h" | "go" | "rb" => {
            fs::read_to_string(path)?
        }
        _ => return Ok(None),
    };

    Ok(Some(content))
}