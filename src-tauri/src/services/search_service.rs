use jieba_rs::Jieba;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tantivy::collector::TopDocs;
use tantivy::query::{BooleanQuery, Occur, QueryParser, TermQuery};
use tantivy::schema::*;
use tantivy::{Index, IndexReader, TantivyDocument, Term};
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
    pub match_count: u32,
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
        // 使用 STRING 类型，每个分词作为一个独立的 term 存储
        builder.add_text_field("content", STRING);
        // 保存原始内容用于生成 snippet
        builder.add_text_field("raw_content", STORED);
        builder.build()
    }

    pub fn open(index_path: &Path) -> Result<Self, SearchError> {
        info!("初始化搜索服务, index_path={:?}", index_path);

        let schema = Self::create_schema();

        // 索引版本文件，用于检测 schema 变化
        let version_file = index_path.join(".version");
        let current_version = "3"; // 更新版本号当 schema 变化时

        // 检查版本是否匹配，不匹配则删除旧索引
        let needs_rebuild = if version_file.exists() {
            let existing_version = std::fs::read_to_string(&version_file).unwrap_or_default();
            if existing_version != current_version {
                info!("索引版本不匹配 ({} != {})，重建索引", existing_version, current_version);
                true
            } else {
                false
            }
        } else {
            // 没有版本文件，可能是旧版本或新安装
            if index_path.exists() {
                info!("未找到索引版本文件，重建索引");
                true
            } else {
                false
            }
        };

        // 如果需要重建，删除旧索引目录
        if needs_rebuild && index_path.exists() {
            info!("删除旧索引目录: {:?}", index_path);
            std::fs::remove_dir_all(index_path)?;
        }

        // 检查是否存在有效的 Tantivy 索引（需要 meta.json 文件）
        let meta_json_path = index_path.join("meta.json");
        let has_valid_index = meta_json_path.exists();

        let index = if has_valid_index {
            debug!("打开现有索引: {:?}", index_path);
            Index::open_in_dir(index_path)?
        } else {
            info!("创建新索引: {:?}", index_path);
            std::fs::create_dir_all(index_path)?;
            let idx = Index::create_in_dir(index_path, schema.clone())?;
            // 写入版本文件
            std::fs::write(&version_file, current_version)?;
            idx
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

    /// 清理 OCR 文本中的多余空格
    /// OCR 输出的中文文本通常每个字符之间都有空格，如 "奇 安 信 集 团"
    /// 需要去除中文字符之间的空格，同时保留英文单词之间的空格
    fn clean_ocr_text(text: &str) -> String {
        let chars: Vec<char> = text.chars().collect();
        let mut result = String::with_capacity(text.len());
        let mut i = 0;

        while i < chars.len() {
            let c = chars[i];

            // 如果当前字符是空格
            if c == ' ' {
                // 检查空格前后是否都是中文字符
                let prev_is_chinese = if i > 0 {
                    Self::is_chinese(chars[i - 1])
                } else {
                    false
                };

                let next_is_chinese = if i + 1 < chars.len() {
                    Self::is_chinese(chars[i + 1])
                } else {
                    false
                };

                // 如果空格两边都是中文字符，跳过这个空格
                if prev_is_chinese && next_is_chinese {
                    i += 1;
                    continue;
                }

                // 否则保留空格（英文单词之间、数字之间等）
                result.push(c);
            } else {
                result.push(c);
            }

            i += 1;
        }

        result
    }

    /// 判断字符是否为中文字符
    fn is_chinese(c: char) -> bool {
        // CJK Unified Ideographs range: U+4E00 to U+9FFF
        // CJK Unified Ideographs Extension A: U+3400 to U+4DBF
        // CJK Unified Ideographs Extension B-F: U+20000 to U+2CEAF
        matches!(c, '\u{4E00}'..='\u{9FFF}' | '\u{3400}'..='\u{4DBF}')
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
        let raw_content_field = self.schema.get_field("raw_content").unwrap();

        // 清理 OCR 文本中的多余空格
        let cleaned_content = Self::clean_ocr_text(content);

        // 中文分词
        let tokens: Vec<String> = self.jieba.cut(&cleaned_content, true).into_iter().map(|s| s.to_string()).collect();
        info!("分词完成: {} tokens, 原始内容前100字符: {:?}, 清理后前100字符: {:?}",
            tokens.len(),
            &content.chars().take(100).collect::<String>(),
            &cleaned_content.chars().take(100).collect::<String>());
        debug!("分词结果前20个: {:?}", tokens.iter().take(20).collect::<Vec<_>>());

        let mut doc = TantivyDocument::default();
        doc.add_u64(page_id_field, page_id);
        doc.add_u64(pdf_id_field, pdf_id);
        if let Some(fid) = folder_id {
            doc.add_u64(folder_id_field, fid as u64);
        }
        doc.add_u64(page_number_field, page_number as u64);
        doc.add_text(filename_field, filename);

        // 为每个分词结果添加一个 STRING 字段值
        // STRING 字段会将整个值作为一个 term 存储
        for token in &tokens {
            doc.add_text(content_field, token);
        }

        // 保存清理后的内容用于生成 snippet
        doc.add_text(raw_content_field, &cleaned_content);

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
        info!("搜索: query='{}', folder_id={:?}, limit={}", query, folder_id, limit);

        let searcher = self.reader.searcher();

        let content_field = self.schema.get_field("content").unwrap();
        let filename_field = self.schema.get_field("filename").unwrap();

        // 中文分词
        let tokens: Vec<String> = self.jieba.cut(query, true).into_iter().map(|s| s.to_string()).collect();
        info!("搜索查询分词: '{}' -> {:?}", query, tokens);

        // 构建 BooleanQuery：每个分词结果作为一个 TermQuery，使用 Should 组合
        let mut queries: Vec<(Occur, Box<dyn tantivy::query::Query>)> = Vec::new();

        for token in &tokens {
            // 跳过空 token 和单字符空格
            if token.trim().is_empty() {
                continue;
            }
            info!("添加搜索词: '{}'", token);
            let term = Term::from_field_text(content_field, token);
            let term_query = Box::new(TermQuery::new(term, IndexRecordOption::WithFreqs));
            queries.push((Occur::Should, term_query));
        }

        // 也搜索文件名
        let query_parser = QueryParser::for_index(&self.index, vec![filename_field]);
        if let Ok(filename_query) = query_parser.parse_query(query) {
            queries.push((Occur::Should, filename_query));
        }

        if queries.is_empty() {
            debug!("查询为空，返回空结果");
            return Ok(Vec::new());
        }

        let boolean_query = BooleanQuery::new(queries);

        let top_docs = match searcher.search(&boolean_query, &TopDocs::with_limit(limit)) {
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
            // 获取原始内容用于生成 snippet
            let raw_content = doc.get_first(self.schema.get_field("raw_content").unwrap())
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // 文件夹过滤
            if let Some(target_fid) = folder_id {
                if fid != Some(target_fid) {
                    continue;
                }
            }

            // 使用原始查询词生成 snippet 并高亮
            let (snippet, match_count) = Self::generate_snippet(&raw_content, query, 100);

            results.push(SearchResult {
                page_id,
                pdf_id,
                folder_id: fid,
                page_number,
                filename,
                score,
                snippet,
                match_count,
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

        // 刷新 reader 以使删除立即生效
        if let Err(e) = self.reader.reload() {
            warn!("刷新索引reader失败: {}", e);
        } else {
            debug!("索引reader刷新成功");
        }

        Ok(())
    }

    fn generate_snippet(content: &str, query: &str, max_len: usize) -> (String, u32) {
        // 在原始内容中查找查询词的位置
        // 使用字符索引而不是字节索引来正确处理 UTF-8
        let content_chars: Vec<char> = content.chars().collect();
        let query_chars: Vec<char> = query.chars().collect();

        if query_chars.is_empty() {
            let end = max_len.min(content_chars.len());
            return (format!("{}...", content_chars[..end].iter().collect::<String>()), 0);
        }

        // 统计关键字在整个内容中出现的次数
        let mut match_count = 0u32;
        let mut i = 0;
        while i <= content_chars.len().saturating_sub(query_chars.len()) {
            let mut found = true;
            for j in 0..query_chars.len() {
                if content_chars[i + j] != query_chars[j] {
                    found = false;
                    break;
                }
            }
            if found {
                match_count += 1;
                i += query_chars.len(); // 跳过已匹配的部分，避免重复计数
            } else {
                i += 1;
            }
        }

        // 找到第一个匹配位置用于生成 snippet
        let mut found_pos = None;
        'outer: for i in 0..content_chars.len().saturating_sub(query_chars.len()) {
            for j in 0..query_chars.len() {
                if content_chars[i + j] != query_chars[j] {
                    continue 'outer;
                }
            }
            found_pos = Some(i);
            break;
        }

        let snippet = if let Some(pos) = found_pos {
            let start = pos.saturating_sub(30);
            let end = (pos + query_chars.len() + 30).min(content_chars.len());

            let snippet_str: String = content_chars[start..end].iter().collect();

            // 高亮显示匹配的关键词
            let query_in_snippet: String = query_chars.iter().collect();
            let highlighted = snippet_str.replace(&query_in_snippet, &format!("**{}**", query_in_snippet));
            format!("...{}...", highlighted)
        } else {
            let end = max_len.min(content_chars.len());
            format!("{}...", content_chars[..end].iter().collect::<String>())
        };

        (snippet, match_count)
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
            match_count: 3,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SearchResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.page_id, 1);
        assert_eq!(deserialized.pdf_id, 100);
        assert_eq!(deserialized.filename, "test.pdf");
        assert_eq!(deserialized.match_count, 3);
    }

    #[test]
    fn test_generate_snippet_found() {
        let content = "这是一段很长的测试文本，包含一些重要内容，我们希望找到关键词并生成摘要";
        let (snippet, match_count) = SearchService::generate_snippet(content, "关键词", 100);

        assert!(snippet.contains("关键词"));
        assert!(snippet.starts_with("..."));
        assert!(snippet.ends_with("..."));
        // 验证高亮标记
        assert!(snippet.contains("**关键词**"));
        assert_eq!(match_count, 1);
    }

    #[test]
    fn test_generate_snippet_not_found() {
        let content = "这是一段测试文本";
        let (snippet, match_count) = SearchService::generate_snippet(content, "不存在", 10);

        assert!(snippet.ends_with("..."));
        assert!(snippet.len() <= 15); // 10 chars + "..."
        assert_eq!(match_count, 0);
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