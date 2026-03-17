use crate::models::PdfType;
use image::DynamicImage;
use std::path::Path;
use thiserror::Error;
use tracing::debug;

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
        debug!("Detecting PDF type for: {:?}", pdf_path);
        // 尝试提取文本，如果有足够文本则为文字型
        if let Ok(text) = self.extract_text(pdf_path) {
            if text.trim().len() > 100 {
                debug!("PDF type detected: Text ({} chars)", text.trim().len());
                return Ok(PdfType::Text);
            }
        }
        debug!("PDF type detected: Scanned");
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
        debug!("Getting PDF metadata for: {:?}", pdf_path);
        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| PdfError::OpenError(e.to_string()))?;
        let pages = doc.get_pages();
        let file_size = std::fs::metadata(pdf_path)?.len() as i64;

        debug!("PDF metadata retrieved: pages={}, size={}", pages.len(), file_size);
        Ok(PdfMetadata {
            page_count: pages.len() as i32,
            file_size,
        })
    }

    /// 渲染 PDF 页面为图像
    /// 注意：这是一个占位实现，需要添加 pdfium 或类似的 PDF 渲染库
    pub fn render_page(&self, _pdf_path: &Path, _page_num: u32) -> Result<DynamicImage, PdfError> {
        // TODO: 使用 pdfium 或 pdf-render 库实现 PDF 页面渲染
        // 目前返回一个占位错误
        Err(PdfError::RenderError(
            "PDF rendering not implemented. Please add pdfium or similar library.".to_string(),
        ))
    }
}

#[derive(Debug, Clone)]
pub struct PdfMetadata {
    pub page_count: i32,
    pub file_size: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_pdf_service_creation() {
        let service = PdfService::new();
        assert!(service.is_ok());
    }

    #[test]
    fn test_pdf_error_display() {
        let err = PdfError::OpenError("test error".to_string());
        assert!(err.to_string().contains("test error"));

        let err = PdfError::TextError("extract failed".to_string());
        assert!(err.to_string().contains("extract failed"));
    }

    #[test]
    fn test_pdf_error_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let pdf_err: PdfError = io_err.into();
        assert!(matches!(pdf_err, PdfError::IoError(_)));
    }

    #[test]
    fn test_page_count_nonexistent_file() {
        let service = PdfService::new().unwrap();
        let result = service.page_count(Path::new("/nonexistent/path/file.pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_detect_type_nonexistent_file() {
        let service = PdfService::new().unwrap();
        let result = service.detect_type(Path::new("/nonexistent/path/file.pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_get_metadata_nonexistent_file() {
        let service = PdfService::new().unwrap();
        let result = service.get_metadata(Path::new("/nonexistent/path/file.pdf"));
        assert!(result.is_err());
    }

    #[test]
    fn test_pdf_metadata_struct() {
        let metadata = PdfMetadata {
            page_count: 10,
            file_size: 1024,
        };

        assert_eq!(metadata.page_count, 10);
        assert_eq!(metadata.file_size, 1024);
    }
}