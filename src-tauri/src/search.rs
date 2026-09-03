use crate::entity::Entity;
use anyhow::Result;
use std::path::Path;
use tantivy::{
    collector::TopDocs,
    doc,
    query::QueryParser,
    schema::{Schema, STORED, TEXT, STRING},
    Index, IndexReader, IndexWriter, ReloadPolicy, TantivyDocument,
};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SearchIndex {
    index: Arc<Mutex<Index>>,
    reader: Arc<Mutex<IndexReader>>,
    writer: Arc<Mutex<IndexWriter>>,
    schema: Schema,
}

impl SearchIndex {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut schema_builder = Schema::builder();
        schema_builder.add_text_field("id", STRING | STORED);
        schema_builder.add_text_field("entity_type", STRING | STORED);
        schema_builder.add_text_field("label", TEXT | STORED);
        schema_builder.add_text_field("properties", TEXT);
        let schema = schema_builder.build();

        let index = Index::create_in_dir(path, schema.clone())
            .or_else(|_| Index::open_in_dir(path))?;

        let writer = index.writer(50_000_000)?;
        let reader: IndexReader = index
            .reader_builder()
            .reload_policy(ReloadPolicy::OnCommitWithDelay)
            .try_into()?;

        Ok(Self {
            index: Arc::new(Mutex::new(index)),
            reader: Arc::new(Mutex::new(reader)),
            writer: Arc::new(Mutex::new(writer)),
            schema,
        })
    }

    pub fn index_entity(&self, entity: &Entity) -> Result<()> {
        let mut writer = self.writer.blocking_lock();
        let id_field = self.schema.get_field("id").unwrap();
        let type_field = self.schema.get_field("entity_type").unwrap();
        let label_field = self.schema.get_field("label").unwrap();
        let props_field = self.schema.get_field("properties").unwrap();

        let props_str = serde_json::to_string(&entity.properties).unwrap_or_default();

        writer.add_document(doc!(
            id_field => entity.id.clone(),
            type_field => entity.entity_type.clone(),
            label_field => entity.label.clone(),
            props_field => props_str,
        ))?;

        writer.commit()?;
        Ok(())
    }

    pub fn remove_entity(&self, entity_id: &str) -> Result<()> {
        let mut writer = self.writer.blocking_lock();
        let id_field = self.schema.get_field("id").unwrap();
        let term = tantivy::Term::from_field_text(id_field, entity_id);
        writer.delete_term(term);
        writer.commit()?;
        Ok(())
    }

    pub fn search(&self, query_str: &str, limit: usize) -> Result<Vec<SearchResult>> {
        let reader = self.reader.blocking_lock();
        let searcher = reader.searcher();
        let index = self.index.blocking_lock();

        let id_field = self.schema.get_field("id").unwrap();
        let type_field = self.schema.get_field("entity_type").unwrap();
        let label_field = self.schema.get_field("label").unwrap();

        let query_parser = QueryParser::for_index(&index, vec![label_field, type_field]);
        let query = query_parser.parse_query(query_str)?;

        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (_score, doc_address) in top_docs {
            let retrieved_doc: TantivyDocument = searcher.doc(doc_address)?;
            let id = retrieved_doc.get_first(id_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let entity_type = retrieved_doc.get_first(type_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let label = retrieved_doc.get_first(label_field)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            results.push(SearchResult {
                id,
                entity_type,
                label,
            });
        }

        Ok(results)
    }

    pub fn reindex_all(&self, entities: &[Entity]) -> Result<()> {
        let mut writer = self.writer.blocking_lock();
        writer.delete_all_documents()?;

        let id_field = self.schema.get_field("id").unwrap();
        let type_field = self.schema.get_field("entity_type").unwrap();
        let label_field = self.schema.get_field("label").unwrap();
        let props_field = self.schema.get_field("properties").unwrap();

        for entity in entities {
            let props_str = serde_json::to_string(&entity.properties).unwrap_or_default();
            writer.add_document(doc!(
                id_field => entity.id.clone(),
                type_field => entity.entity_type.clone(),
                label_field => entity.label.clone(),
                props_field => props_str,
            ))?;
        }

        writer.commit()?;
        Ok(())
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct SearchResult {
    pub id: String,
    pub entity_type: String,
    pub label: String,
}