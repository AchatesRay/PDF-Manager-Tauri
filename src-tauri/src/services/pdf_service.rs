use crate::models::PdfType;
use image::DynamicImage;
use pdfium_render::prelude::*;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// 默认最大渲染尺寸
const DEFAULT_MAX_RENDER_DIMENSION: u32 = 2000;

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("无法打开PDF: {0}")]
    OpenError(String),
    #[error("渲染页面失败: {0}")]
    RenderError(String),
    #[error("提取文本失败: {0}")]
    TextError(String),
    #[error("IO错误: {0}")]
    IoError(#[from] std::io::Error),
}

/// PDF 服务
///
/// 注意: Pdfium 实例不在结构体中存储，因为 PdfiumLibraryBindings 不是 Send，
/// 无法在多线程环境中共享。每次渲染时创建新的 Pdfium 实例。
pub struct PdfService;

impl PdfService {
    pub fn new() -> Result<Self, PdfError> {
        info!("初始化PDF服务");
        Ok(Self)
    }

    /// 创建 Pdfium 实例
    fn create_pdfium() -> Result<Pdfium, PdfError> {
        // 使用系统库绑定
        // 注意: 需要系统上安装 pdfium.dll 或在应用目录中放置该 DLL
        Pdfium::bind_to_system_library()
            .map(|bindings| Pdfium::new(bindings))
            .map_err(|e| {
                error!("Pdfium绑定失败: {}。请确保 pdfium.dll 在系统 PATH 或应用目录中。", e);
                PdfError::RenderError(format!(
                    "无法绑定Pdfium: {}。请确保 pdfium.dll 已安装。",
                    e
                ))
            })
    }

    /// 获取 PDF 页数
    pub fn page_count(&self, pdf_path: &Path) -> Result<u32, PdfError> {
        debug!("获取PDF页数: {:?}", pdf_path);

        if !pdf_path.exists() {
            error!("PDF文件不存在: {:?}", pdf_path);
            return Err(PdfError::OpenError(format!("文件不存在: {}", pdf_path.display())));
        }

        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| {
                error!("加载PDF失败: {:?}, 错误: {}", pdf_path, e);
                PdfError::OpenError(format!("无法加载PDF: {}", e))
            })?;

        let pages = doc.get_pages();
        let count = pages.len() as u32;
        debug!("PDF页数: {} ({:?})", count, pdf_path);
        Ok(count)
    }

    /// 检测 PDF 类型
    pub fn detect_type(&self, pdf_path: &Path) -> Result<PdfType, PdfError> {
        debug!("检测PDF类型: {:?}", pdf_path);

        match self.extract_text(pdf_path) {
            Ok(text) => {
                let char_count = text.trim().len();
                debug!("提取文本字符数: {}", char_count);

                if char_count > 100 {
                    info!("PDF类型: 文字型 ({}字符) - {:?}", char_count, pdf_path);
                    Ok(PdfType::Text)
                } else {
                    info!("PDF类型: 扫描型 ({}字符) - {:?}", char_count, pdf_path);
                    Ok(PdfType::Scanned)
                }
            }
            Err(e) => {
                warn!("提取文本失败，假定为扫描型: {:?}, 错误: {}", pdf_path, e);
                Ok(PdfType::Scanned)
            }
        }
    }

    /// 提取 PDF 文本
    pub fn extract_text(&self, pdf_path: &Path) -> Result<String, PdfError> {
        debug!("提取PDF文本: {:?}", pdf_path);

        let text = pdf_extract::extract_text(pdf_path)
            .map_err(|e| {
                error!("提取PDF文本失败: {:?}, 错误: {}", pdf_path, e);
                PdfError::TextError(format!("文本提取失败: {}", e))
            })?;

        debug!("文本提取成功: {} 字符", text.len());
        Ok(text)
    }

    /// 获取 PDF 元信息
    pub fn get_metadata(&self, pdf_path: &Path) -> Result<PdfMetadata, PdfError> {
        debug!("获取PDF元数据: {:?}", pdf_path);

        if !pdf_path.exists() {
            error!("PDF文件不存在: {:?}", pdf_path);
            return Err(PdfError::OpenError(format!("文件不存在: {}", pdf_path.display())));
        }

        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| {
                error!("加载PDF失败: {:?}, 错误: {}", pdf_path, e);
                PdfError::OpenError(format!("无法加载PDF: {}", e))
            })?;

        let pages = doc.get_pages();
        let file_size = match std::fs::metadata(pdf_path) {
            Ok(meta) => meta.len() as i64,
            Err(e) => {
                warn!("获取文件大小失败: {:?}, 错误: {}", pdf_path, e);
                0
            }
        };

        let metadata = PdfMetadata {
            page_count: pages.len() as i32,
            file_size,
        };

        debug!("PDF元数据: pages={}, size={} bytes", metadata.page_count, metadata.file_size);
        Ok(metadata)
    }

    /// 渲染 PDF 页面为图像（使用默认尺寸）
    ///
    /// 参数:
    /// - pdf_path: PDF 文件路径
    /// - page_num: 页码 (1-indexed, 用户视角)
    ///
    /// 返回:
    /// - 渲染后的图像
    pub fn render_page(&self, pdf_path: &Path, page_num: u32) -> Result<DynamicImage, PdfError> {
        self.render_page_with_limit(pdf_path, page_num, DEFAULT_MAX_RENDER_DIMENSION)
    }

    /// 渲染 PDF 页面为图像（指定最大尺寸）
    ///
    /// 参数:
    /// - pdf_path: PDF 文件路径
    /// - page_num: 页码 (1-indexed, 用户视角)
    /// - max_dimension: 最大尺寸限制（宽度或高度的最大值）
    ///
    /// 返回:
    /// - 渲染后的图像
    pub fn render_page_with_limit(&self, pdf_path: &Path, page_num: u32, max_dimension: u32) -> Result<DynamicImage, PdfError> {
        debug!("渲染PDF页面: page={}, path={:?}, max_dimension={}", page_num, pdf_path, max_dimension);

        if !pdf_path.exists() {
            error!("PDF文件不存在: {:?}", pdf_path);
            return Err(PdfError::RenderError(format!("文件不存在: {}", pdf_path.display())));
        }

        // 每次渲染创建新的 Pdfium 实例
        // 这是为了避免线程安全问题 (PdfiumLibraryBindings 不是 Send)
        let pdfium = Self::create_pdfium()?;
        info!("Pdfium实例创建成功，开始渲染...");

        // 打开 PDF 文档
        let document = pdfium
            .load_pdf_from_file(pdf_path, None)
            .map_err(|e| {
                error!("加载PDF失败: {:?}, 错误: {}", pdf_path, e);
                PdfError::RenderError(format!("无法加载PDF: {}", e))
            })?;

        // 获取页面 (用户输入是 1-indexed，pdfium 使用 0-indexed)
        let page_index = page_num.saturating_sub(1);
        let total_pages = document.pages().len() as u32;

        if page_index >= total_pages {
            error!("页码超出范围: page={}, total={}", page_num, total_pages);
            return Err(PdfError::RenderError(
                format!("页码 {} 超出范围 (总页数: {})", page_num, total_pages)
            ));
        }

        let page = document
            .pages()
            .get(page_index as u16)
            .map_err(|e| {
                error!("获取页面失败: page={}, 错误: {}", page_num, e);
                PdfError::RenderError(format!("无法获取页面 {}: {}", page_num, e))
            })?;

        // 获取页面原始尺寸
        let page_width = page.width().value as u32;
        let page_height = page.height().value as u32;

        // 计算渲染尺寸，保持宽高比
        let (render_width, render_height) = if page_width > page_height {
            let scale = max_dimension as f64 / page_width as f64;
            let width = max_dimension;
            let height = (page_height as f64 * scale) as u32;
            (width, height)
        } else {
            let scale = max_dimension as f64 / page_height as f64;
            let height = max_dimension;
            let width = (page_width as f64 * scale) as u32;
            (width, height)
        };

        // 渲染配置
        let render_config = PdfRenderConfig::new()
            .set_target_width(render_width as i32)
            .set_maximum_height(render_height as i32);

        debug!("开始渲染页面: page={}, config={}x{}, max_dimension={}", page_num, render_width, render_height, max_dimension);

        // 渲染页面为位图
        let bitmap = page
            .render_with_config(&render_config)
            .map_err(|e| {
                error!("渲染页面失败: page={}, 错误: {}", page_num, e);
                PdfError::RenderError(format!("渲染失败: {}", e))
            })?;

        // 转换为 image::DynamicImage
        let width = bitmap.width() as u32;
        let height = bitmap.height() as u32;
        let pixels = bitmap.as_raw_bytes();

        debug!("位图大小: {}x{}, {} bytes", width, height, pixels.len());

        let buffer = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(width, height, pixels.to_vec())
            .ok_or_else(|| {
                error!("创建图像缓冲区失败: {}x{}", width, height);
                PdfError::RenderError("无法创建图像缓冲区".to_string())
            })?;

        info!("页面渲染成功: page={}, size={}x{}", page_num, width, height);
        Ok(image::DynamicImage::ImageRgba8(buffer))
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