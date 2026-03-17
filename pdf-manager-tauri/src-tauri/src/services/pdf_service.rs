use crate::models::PdfType;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("Failed to open PDF: {0}")]
    OpenError(String),
    #[error("Failed to render page: {0}")]
    RenderError(String),
    #[error("Failed to extract text: {0}")]
    TextError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct PdfService;

impl PdfService {
    pub fn new() -> Result<Self, PdfError> {
        Ok(Self {})
    }

    /// 获取 PDF 页数
    pub fn page_count(&self, pdf_path: &Path) -> Result<u32, PdfError> {
        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| PdfError::OpenError(e.to_string()))?;
        let pages = doc.get_pages();
        Ok(pages.len() as u32)
    }

    /// 检测 PDF 类型
    pub fn detect_type(&self, pdf_path: &Path) -> Result<PdfType, PdfError> {
        // 尝试提取文本，如果有足够文本则为文字型
        if let Ok(text) = self.extract_text(pdf_path) {
            if text.trim().len() > 100 {
                return Ok(PdfType::Text);
            }
        }
        Ok(PdfType::Scanned)
    }

    /// 提取 PDF 文本
    pub fn extract_text(&self, pdf_path: &Path) -> Result<String, PdfError> {
        let text = pdf_extract::extract_text(pdf_path)
            .map_err(|e| PdfError::TextError(e.to_string()))?;
        Ok(text)
    }

    /// 获取 PDF 元信息
    pub fn get_metadata(&self, pdf_path: &Path) -> Result<PdfMetadata, PdfError> {
        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| PdfError::OpenError(e.to_string()))?;
        let pages = doc.get_pages();
        let file_size = std::fs::metadata(pdf_path)?.len() as i64;

        Ok(PdfMetadata {
            page_count: pages.len() as i32,
            file_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PdfMetadata {
    pub page_count: i32,
    pub file_size: i64,
}