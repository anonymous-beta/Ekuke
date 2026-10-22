
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

const MAX_TEXT_FILE_BYTES: u64 = 5 * 1024 * 1024;

pub fn extract_text_from_file(path: &Path) -> Result<Option<String>> {
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let supported = matches!(
        extension.as_str(),
        "txt" | "md" | "markdown" | "csv" | "json" | "xml" | "html" | "htm" | "log"
    );

    if !supported {
        return Ok(None);
    }

    let metadata = fs::metadata(path)
        .with_context(|| format!("Could not inspect file: {}", path.display()))?;

    if !metadata.is_file() {
        bail!("The selected path is not a regular file");
    }

    if metadata.len() > MAX_TEXT_FILE_BYTES {
        bail!(
            "File is too large. Maximum supported size is {} MiB",
            MAX_TEXT_FILE_BYTES / (1024 * 1024)
        );
    }

    let bytes = fs::read(path)
        .with_context(|| format!("Could not read file: {}", path.display()))?;

    // Recheck after reading in case the file changed after metadata().
    if bytes.len() as u64 > MAX_TEXT_FILE_BYTES {
        bail!("File exceeded the allowed size while being read");
    }

    let text = String::from_utf8(bytes)
        .context("File is not valid UTF-8 text")?;

    Ok(Some(text))
}
