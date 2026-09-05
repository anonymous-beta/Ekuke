use std::path::{Path, PathBuf};
use anyhow::Result;
use tantivy::collector::TopDocs;
use tantivy::query::{AllQuery, QueryParser};
use tantivy::schema::{Field, Schema, FAST, STORED, TEXT};
use tantivy::{doc, Index, ReloadPolicy};
use walkdir::WalkDir;

use crate::models::{Note, SearchOptions, SearchResultItem};
use crate::utils::extract_text_from_file;

/// Public wrapper for the search engine
pub struct SearchIndex {
    engine: SearchEngine,
}

impl SearchIndex {
    pub fn new(index_path: &Path) -> Result<Self> {
        let engine = SearchEngine::new(index_path)?;
        Ok(Self { engine })
    }

    pub fn index_directory(&mut self, dir_path: &Path, extensions: &[&str]) -> Result<usize> {
        self.engine.index_directory(dir_path, extensions)
    }

    pub fn add_note(&mut self, note: &Note) -> Result<()> {
        self.engine.add_note(note)
    }

    pub fn remove_note(&mut self, path: &Path) -> Result<()> {
        self.engine.remove_note(path)
    }

    pub fn update_note(&mut self, old_path: &Path, new_note: &Note) -> Result<()> {
        self.engine.update_note(old_path, new_note)
    }

    pub fn search(&self, query: &str, options: &SearchOptions) -> Result<Vec<SearchResultItem>> {
        self.engine.search(query, options)
    }

    pub fn all_notes(&self) -> Result<Vec<SearchResultItem>> {
        self.engine.all_notes()
    }

    pub fn optimize(&mut self) -> Result<()> {
        self.engine.optimize()
    }

    pub fn clear(&mut self) -> Result<()> {
        self.engine.clear()
    }
}

/// Internal search engine
struct SearchEngine {
    index: Index,
    schema: Schema,
    title_field: Field,
    content_field: Field,
    path_field: Field,
    timestamp_field: Field,
    index_path: PathBuf,
}

impl SearchEngine {
    fn new(index_path: &Path) -> Result<Self> {
        let mut schema_builder = Schema::builder();
        let title_field = schema_builder.add_text_field("title", TEXT | STORED);
        let content_field = schema_builder.add_text_field("content", TEXT | STORED);
        let path_field = schema_builder.add_text_field("path", STORED);
        let timestamp_field = schema_builder.add_i64_field("timestamp", FAST | STORED);

        let schema = schema_builder.build();
        let index = Index::create_in_dir(index_path, schema.clone())?;

        Ok(Self {
            index,
            schema,
            title_field,
            content_field,
            path_field,
            timestamp_field,
            index_path: index_path.to_path_buf(),
        })
    }

    fn index_directory(&mut self, dir_path: &Path, extensions: &[&str]) -> Result<usize> {
        let mut writer = self.index.writer(50_000_000)?;
        let mut count = 0;

        for entry in WalkDir::new(dir_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }

            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !extensions.is_empty() && !extensions.contains(&ext) {
                continue;
            }

            if let Some(text) = extract_text_from_file(path)? {
                let title = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Untitled")
                    .to_string();

                let timestamp = path.metadata()
                    .and_then(|m| m.modified())
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64)
                    .unwrap_or(0);

                let doc = doc!(
                    self.title_field => title,
                    self.content_field => text,
                    self.path_field => path.display().to_string(),
                    self.timestamp_field => timestamp,
                );

                writer.add_document(doc)?;
                count += 1;
            }
        }

        writer.commit()?;
        Ok(count)
    }

    fn add_note(&mut self, note: &Note) -> Result<()> {
        let mut writer = self.index.writer(50_000_000)?;
        let doc = doc!(
            self.title_field => note.title.clone(),
            self.content_field => note.content.clone(),
            self.path_field => note.path.display().to_string(),
            self.timestamp_field => note.timestamp,
        );
        writer.add_document(doc)?;
        writer.commit()?;
        Ok(())
    }

    fn remove_note(&mut self, path: &Path) -> Result<()> {
        let mut writer = self.index.writer(50_000_000)?;
        let path_str = path.display().to_string();
        let term = tantivy::Term::from_field_text(self.path_field, &path_str);
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    fn update_note(&mut self, old_path: &Path, new_note: &Note) -> Result<()> {
        if old_path != &new_note.path {
            self.remove_note(old_path)?;
        }
        self.add_note(new_note)?;
        Ok(())
    }

    fn search(&self, query_str: &str, options: &SearchOptions) -> Result<Vec<SearchResultItem>> {
        let reader = self.index.reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;

        let searcher = reader.searcher();
        let query_parser = QueryParser::for_index(&self.index, vec![self.title_field, self.content_field]);
        let query = query_parser.parse_query(query_str)?;

        let limit = if options.limit > 0 { options.limit } else { 50 };
        let collector = TopDocs::with_limit(limit);
        let top_docs = searcher.search(&query, &collector)?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let retrieved = searcher.doc(doc_address)?;

            let title = retrieved
                .get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            let content = retrieved
                .get_first(self.content_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let path_str = retrieved
                .get_first(self.path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let timestamp = retrieved
                .get_first(self.timestamp_field)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            let snippet = if !content.is_empty() {
                let words: Vec<&str> = content.split_whitespace().collect();
                let query_words: Vec<&str> = query_str.split_whitespace().collect();
                let mut best_pos = 0;
                let mut best_score = 0;

                for (i, word) in words.iter().enumerate() {
                    let score = query_words.iter().filter(|q| word.to_lowercase().contains(&q.to_lowercase())).count();
                    if score > best_score {
                        best_score = score;
                        best_pos = i;
                    }
                }

                let start = best_pos.saturating_sub(30);
                let end = (best_pos + 30).min(words.len());
                let snippet = words[start..end].join(" ");
                if start > 0 { format!("...{}", snippet) } else { snippet }
            } else {
                String::new()
            };

            results.push(SearchResultItem {
                title,
                path: PathBuf::from(path_str),
                content: snippet,
                timestamp,
                score: score as f32,
            });
        }

        Ok(results)
    }

    fn all_notes(&self) -> Result<Vec<SearchResultItem>> {
        let reader = self.index.reader_builder()
            .reload_policy(ReloadPolicy::OnCommit)
            .try_into()?;

        let searcher = reader.searcher();
        let collector = TopDocs::with_limit(usize::MAX);
        let top_docs = searcher.search(&AllQuery, &collector)?;

        let mut results = Vec::new();
        for (_, doc_address) in top_docs {
            let retrieved = searcher.doc(doc_address)?;
            let title = retrieved
                .get_first(self.title_field)
                .and_then(|v| v.as_str())
                .unwrap_or("Untitled")
                .to_string();

            let path_str = retrieved
                .get_first(self.path_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let timestamp = retrieved
                .get_first(self.timestamp_field)
                .and_then(|v| v.as_i64())
                .unwrap_or(0);

            results.push(SearchResultItem {
                title,
                path: PathBuf::from(path_str),
                content: String::new(),
                timestamp,
                score: 1.0,
            });
        }

        Ok(results)
    }

    fn optimize(&mut self) -> Result<()> {
        let mut writer = self.index.writer(50_000_000)?;
        writer.merge_segments(writer.get_merge_policy().clone())?;
        writer.commit()?;
        Ok(())
    }

    fn clear(&mut self) -> Result<()> {
        std::fs::remove_dir_all(&self.index_path)?;
        std::fs::create_dir_all(&self.index_path)?;
        let index = Index::create_in_dir(&self.index_path, self.schema.clone())?;
        self.index = index;
        Ok(())
    }
}