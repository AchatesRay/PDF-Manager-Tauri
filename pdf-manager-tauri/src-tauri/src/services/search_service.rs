use jieba_rs::Jieba;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("Index error: {0}")]
    IndexError(#[from] tantivy::TantivyError),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub page_id: u64,
    pub pdf_id: u64,
    pub folder_id: Option<i64>,
    pub page_number: u32,
    pub filename: String,
    pub score: f32,
    pub snippet: String,
}

pub struct SearchService {
    index: Index,
    reader: IndexReader,
    schema: Schema,
    jieba: Jieba,
}

impl SearchService {
    fn create_schema() -> Schema {
        let mut builder = Schema::builder();
        builder.add_u64_field("page_id", INDEXED | STORED);
        builder.add_u64_field("pdf_id", STORED);
        builder.add_u64_field("folder_id", STORED);
        builder.add_u64_field("page_number", STORED);
        builder.add_text_field("filename", TEXT | STORED);
        builder.add_text_field("content", TEXT | STORED);
        builder.build()
    }

    pub fn open(index_path: &Path) -> Result<Self, SearchError> {
        let schema = Self::create_schema();
        let index = if index_path.exists() {
            Index::open_in_dir(index_path)?
        } else {
            std::fs::create_dir_all(index_path)?;
            Index::create_in_dir(index_path, schema.clone())?
        };

        let reader = index.reader()?;

        Ok(Self {
            index,
            reader,
            schema,
            jieba: Jieba::new(),
        })
    }

    pub fn index_page(
        &mut self,
        page_id: u64,
        pdf_id: u64,
        folder_id: Option<i64>,
        page_number: u32,
        filename: &str,
        content: &str,
    ) -> Result<(), SearchError> {
        let mut writer = self.index.writer(50_000_000)?;

        let page_id_field = self.schema.get_field("page_id").unwrap();
        let pdf_id_field = self.schema.get_field("pdf_id").unwrap();
        let folder_id_field = self.schema.get_field("folder_id").unwrap();
        let page_number_field = self.schema.get_field("page_number").unwrap();
        let filename_field = self.schema.get_field("filename").unwrap();
        let content_field = self.schema.get_field("content").unwrap();

        let tokens: Vec<String> = self.jieba.cut(content, true).into_iter().map(|s| s.to_string()).collect();
        let tokenized_content = tokens.join(" ");

        let mut doc = Document::new();
        doc.add_u64(page_id_field, page_id);
        doc.add_u64(pdf_id_field, pdf_id);
        if let Some(fid) = folder_id {
            doc.add_u64(folder_id_field, fid as u64);
        }
        doc.add_u64(page_number_field, page_number as u64);
        doc.add_text(filename_field, filename);
        doc.add_text(content_field, &tokenized_content);

        writer.add_document(doc)?;
        writer.commit()?;

        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        folder_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, SearchError> {
        let searcher = self.reader.searcher();

        let content_field = self.schema.get_field("content").unwrap();
        let filename_field = self.schema.get_field("filename").unwrap();

        let query_parser = QueryParser::for_index(&self.index, vec![content_field, filename_field]);

        let tokens: Vec<String> = self.jieba.cut(query, true).into_iter().map(|s| s.to_string()).collect();
        let query_text = tokens.join(" ");

        let query = query_parser.parse_query(&query_text)?;
        let top_docs = searcher.search(&query, &TopDocs::with_limit(limit))?;

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc = searcher.doc(doc_address)?;

            let page_id = doc.get_first(self.schema.get_field("page_id").unwrap())
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let pdf_id = doc.get_first(self.schema.get_field("pdf_id").unwrap())
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let fid = doc.get_first(self.schema.get_field("folder_id").unwrap())
                .and_then(|v| v.as_u64())
                .map(|v| v as i64);
            let page_number = doc.get_first(self.schema.get_field("page_number").unwrap())
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u32;
            let filename = doc.get_first(self.schema.get_field("filename").unwrap())
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();
            let content = doc.get_first(self.schema.get_field("content").unwrap())
                .and_then(|v| v.as_text())
                .unwrap_or("")
                .to_string();

            if let Some(target_fid) = folder_id {
                if fid != Some(target_fid) {
                    continue;
                }
            }

            let snippet = Self::generate_snippet(&content, query, 100);

            results.push(SearchResult {
                page_id,
                pdf_id,
                folder_id: fid,
                page_number,
                filename,
                score,
                snippet,
            });
        }

        Ok(results)
    }

    pub fn delete_pdf(&mut self, pdf_id: u64) -> Result<(), SearchError> {
        let mut writer = self.index.writer(50_000_000)?;

        let pdf_id_field = self.schema.get_field("pdf_id").unwrap();
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_u64(pdf_id_field, pdf_id),
            tantivy::query::IndexRecordOption::Basic,
        );

        writer.delete_query(Box::new(query))?;
        writer.commit()?;

        Ok(())
    }

    fn generate_snippet(content: &str, query: &str, max_len: usize) -> String {
        let content: String = content.chars().filter(|c| !c.is_whitespace()).collect();

        if let Some(pos) = content.find(query) {
            let start = pos.saturating_sub(30);
            let end = (pos + query.len() + 30).min(content.len());
            let snippet: String = content.chars().skip(start).take(end - start).collect();
            format!("...{}...", snippet)
        } else {
            let end = max_len.min(content.len());
            format!("{}...", content.chars().take(end).collect::<String>())
        }
    }
}