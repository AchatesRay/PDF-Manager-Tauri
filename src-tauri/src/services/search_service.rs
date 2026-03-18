use jieba_rs::Jieba;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::QueryParser;
use tantivy::schema::*;
use tantivy::{Index, IndexReader, TantivyDocument};
use thiserror::Error;
use tracing::{debug, error, info, warn};

#[derive(Error, Debug)]
pub enum SearchError {
    #[error("索引错误: {0}")]
    IndexError(#[from] tantivy::TantivyError),
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
    #[error("查询解析错误: {0}")]
    QueryParseError(#[from] tantivy::query::QueryParserError),
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
        info!("初始化搜索服务, index_path={:?}", index_path);

        let schema = Self::create_schema();
        let index = if index_path.exists() {
            debug!("打开现有索引: {:?}", index_path);
            Index::open_in_dir(index_path)?
        } else {
            info!("创建新索引: {:?}", index_path);
            std::fs::create_dir_all(index_path)?;
            Index::create_in_dir(index_path, schema.clone())?
        };

        let reader = index.reader()?;

        info!("搜索服务初始化成功");
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
        debug!("索引页面: page_id={}, pdf_id={}, filename={}, content_len={}",
            page_id, pdf_id, filename, content.len());

        let mut writer: tantivy::IndexWriter<TantivyDocument> = match self.index.writer(50_000_000) {
            Ok(w) => w,
            Err(e) => {
                error!("创建索引写入器失败: {}", e);
                return Err(SearchError::IndexError(e));
            }
        };

        let page_id_field = self.schema.get_field("page_id").unwrap();
        let pdf_id_field = self.schema.get_field("pdf_id").unwrap();
        let folder_id_field = self.schema.get_field("folder_id").unwrap();
        let page_number_field = self.schema.get_field("page_number").unwrap();
        let filename_field = self.schema.get_field("filename").unwrap();
        let content_field = self.schema.get_field("content").unwrap();

        // 中文分词
        let tokens: Vec<String> = self.jieba.cut(content, true).into_iter().map(|s| s.to_string()).collect();
        let tokenized_content = tokens.join(" ");
        debug!("分词完成: {} tokens", tokens.len());

        let mut doc = TantivyDocument::default();
        doc.add_u64(page_id_field, page_id);
        doc.add_u64(pdf_id_field, pdf_id);
        if let Some(fid) = folder_id {
            doc.add_u64(folder_id_field, fid as u64);
        }
        doc.add_u64(page_number_field, page_number as u64);
        doc.add_text(filename_field, filename);
        doc.add_text(content_field, &tokenized_content);

        match writer.add_document(doc) {
            Ok(_) => debug!("文档添加成功"),
            Err(e) => {
                error!("添加文档失败: {}", e);
                return Err(SearchError::IndexError(e));
            }
        }

        match writer.commit() {
            Ok(_) => info!("页面索引成功: page_id={}, filename={}", page_id, filename),
            Err(e) => {
                error!("提交索引失败: {}", e);
                return Err(SearchError::IndexError(e));
            }
        }

        Ok(())
    }

    pub fn search(
        &self,
        query: &str,
        folder_id: Option<i64>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, SearchError> {
        debug!("搜索: query='{}', folder_id={:?}, limit={}", query, folder_id, limit);

        let searcher = self.reader.searcher();

        let content_field = self.schema.get_field("content").unwrap();
        let filename_field = self.schema.get_field("filename").unwrap();

        let query_parser = QueryParser::for_index(&self.index, vec![content_field, filename_field]);

        // 中文分词
        let tokens: Vec<String> = self.jieba.cut(query, true).into_iter().map(|s| s.to_string()).collect();
        let query_text = tokens.join(" ");
        debug!("搜索查询分词: '{}' -> '{}'", query, query_text);

        let parsed_query = match query_parser.parse_query(&query_text) {
            Ok(q) => q,
            Err(e) => {
                error!("解析查询失败: '{}', 错误: {}", query_text, e);
                return Err(SearchError::QueryParseError(e));
            }
        };

        let top_docs = match searcher.search(&parsed_query, &TopDocs::with_limit(limit)) {
            Ok(docs) => docs,
            Err(e) => {
                error!("执行搜索失败: {}", e);
                return Err(SearchError::IndexError(e));
            }
        };

        debug!("搜索返回 {} 条结果", top_docs.len());

        let mut results = Vec::new();
        for (score, doc_address) in top_docs {
            let doc: TantivyDocument = match searcher.doc(doc_address) {
                Ok(d) => d,
                Err(e) => {
                    warn!("获取文档失败: {:?}, 错误: {}", doc_address, e);
                    continue;
                }
            };

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
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let content = doc.get_first(self.schema.get_field("content").unwrap())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // 文件夹过滤
            if let Some(target_fid) = folder_id {
                if fid != Some(target_fid) {
                    continue;
                }
            }

            let snippet = Self::generate_snippet(&content, &query_text, 100);

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

        info!("搜索完成: query='{}', 结果数={}", query, results.len());
        Ok(results)
    }

    pub fn delete_pdf(&mut self, pdf_id: u64) -> Result<(), SearchError> {
        info!("删除PDF索引: pdf_id={}", pdf_id);

        let mut writer: tantivy::IndexWriter<TantivyDocument> = match self.index.writer(50_000_000) {
            Ok(w) => w,
            Err(e) => {
                error!("创建索引写入器失败: {}", e);
                return Err(SearchError::IndexError(e));
            }
        };

        let pdf_id_field = self.schema.get_field("pdf_id").unwrap();
        let query = tantivy::query::TermQuery::new(
            tantivy::Term::from_field_u64(pdf_id_field, pdf_id),
            IndexRecordOption::Basic,
        );

        match writer.delete_query(Box::new(query)) {
            Ok(_) => debug!("删除查询执行成功"),
            Err(e) => {
                error!("删除PDF索引失败: pdf_id={}, 错误: {}", pdf_id, e);
                return Err(SearchError::IndexError(e));
            }
        }

        match writer.commit() {
            Ok(_) => info!("PDF索引删除成功: pdf_id={}", pdf_id),
            Err(e) => {
                error!("提交删除失败: {}", e);
                return Err(SearchError::IndexError(e));
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_search_error_display() {
        let err = SearchError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "not found"));
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_search_result_serialization() {
        let result = SearchResult {
            page_id: 1,
            pdf_id: 100,
            folder_id: Some(5),
            page_number: 10,
            filename: "test.pdf".to_string(),
            score: 0.95,
            snippet: "...测试内容...".to_string(),
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.page_id, 1);
        assert_eq!(deserialized.pdf_id, 100);
        assert_eq!(deserialized.filename, "test.pdf");
    }

    #[test]
    fn test_generate_snippet_found() {
        let content = "这是一段很长的测试文本，包含一些重要内容，我们希望找到关键词并生成摘要";
        let snippet = SearchService::generate_snippet(content, "关键词", 100);

        assert!(snippet.contains("关键词"));
        assert!(snippet.starts_with("..."));
        assert!(snippet.ends_with("..."));
    }

    #[test]
    fn test_generate_snippet_not_found() {
        let content = "这是一段测试文本";
        let snippet = SearchService::generate_snippet(content, "不存在", 10);

        assert!(snippet.ends_with("..."));
        assert!(snippet.len() <= 15); // 10 chars + "..."
    }

    #[test]
    fn test_search_service_open() {
        let temp_dir = tempdir().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let result = SearchService::open(&index_path);
        assert!(result.is_ok());

        // 验证目录已创建
        assert!(index_path.exists());
    }

    #[test]
    fn test_index_and_search() {
        let temp_dir = tempdir().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let mut service = SearchService::open(&index_path).unwrap();

        // 索引一个文档
        let result = service.index_page(
            1,
            100,
            Some(5),
            1,
            "测试文档.pdf",
            "这是一份测试文档，包含重要内容。"
        );
        assert!(result.is_ok());

        // 搜索
        let results = service.search("测试", None, 10).unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0].page_id, 1);
        assert_eq!(results[0].filename, "测试文档.pdf");
    }

    #[test]
    fn test_search_with_folder_filter() {
        let temp_dir = tempdir().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let mut service = SearchService::open(&index_path).unwrap();

        // 索引两个文档，不同文件夹
        service.index_page(1, 100, Some(1), 1, "doc1.pdf", "测试内容一").unwrap();
        service.index_page(2, 101, Some(2), 1, "doc2.pdf", "测试内容二").unwrap();

        // 搜索文件夹1
        let results = service.search("测试", Some(1), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_id, 1);

        // 搜索文件夹2
        let results = service.search("测试", Some(2), 10).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].page_id, 2);
    }

    #[test]
    fn test_delete_pdf() {
        let temp_dir = tempdir().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let mut service = SearchService::open(&index_path).unwrap();

        // 索引文档
        service.index_page(1, 100, None, 1, "test.pdf", "测试内容").unwrap();

        // 验证可以搜索到
        let results = service.search("测试", None, 10).unwrap();
        assert!(!results.is_empty());

        // 删除 PDF
        service.delete_pdf(100).unwrap();

        // 搜索应该返回空（需要重新打开 reader 来看到删除效果）
        service.reader.reload().unwrap();
        let results = service.search("测试", None, 10).unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_chinese_tokenization() {
        let temp_dir = tempdir().unwrap();
        let index_path = temp_dir.path().join("test_index");

        let mut service = SearchService::open(&index_path).unwrap();

        // 索引中文文档
        service.index_page(
            1,
            100,
            None,
            1,
            "中文文档.pdf",
            "自然语言处理是人工智能的重要分支。"
        ).unwrap();

        // 搜索中文关键词
        let results = service.search("自然语言", None, 10).unwrap();
        assert!(!results.is_empty());

        let results = service.search("人工智能", None, 10).unwrap();
        assert!(!results.is_empty());
    }
}