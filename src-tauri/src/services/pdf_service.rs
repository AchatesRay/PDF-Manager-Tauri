use crate::models::PdfType;
use image::DynamicImage;
use pdfium_render::prelude::*;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, error, info, warn};

/// 默认最大渲染尺寸
const DEFAULT_MAX_RENDER_DIMENSION: u32 = 2000;

/// 渲染请求超时（秒）：渲染线程若被底层库卡死，调用方最多等待这么久即报错，
/// 避免任务无限悬挂（P0-10 兜底）
const RENDER_TIMEOUT_SECS: u64 = 120;

/// 渲染请求：(路径, 页码, 最大尺寸, 应答通道)
type RenderRequest = (
    std::path::PathBuf,
    u32,
    u32,
    std::sync::mpsc::Sender<Result<DynamicImage, PdfError>>,
);

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

    /// 获取线程本地 Pdfium 实例（每线程绑定一次，避免每页重复 bind）
    ///
    /// PdfiumLibraryBindings 非 Send，故用 thread_local；文档仍按页加载
    /// （PdfDocument 借用 Pdfium 生命周期，无法跨调用缓存）。
    fn with_pdfium<R>(f: impl FnOnce(&Pdfium) -> R) -> Result<R, PdfError> {
        thread_local! {
            static PDFIUM: std::cell::OnceCell<Pdfium> = const { std::cell::OnceCell::new() };
        }

        PDFIUM
            .with(|cell| {
                let pdfium = match cell.get() {
                    Some(p) => p,
                    None => {
                        info!("首次在本线程绑定 Pdfium（thread_local 为空）");
                        let created = Self::create_pdfium()?;
                        info!("Pdfium 绑定成功（本线程）");
                        let _ = cell.set(created);
                        cell.get().expect("OnceCell set just now")
                    }
                };
                Ok(f(pdfium))
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
    ///
    /// P0-8（2026-09-26 真实文件实测）：pdf-extract 0.7.12 对部分真实 PDF 的字体
    /// CMap 会直接 `assert!(name == "Identity-H")` —— **panic 而非返回 Err**，
    /// 沿命令线程传播导致整个应用退出（真实合同导入必现，Phase 5 合成样本不触发）。
    /// 此处用 catch_unwind 兜底：捕获点在 `PdfService` 互斥锁之内、
    /// `detect_type` 的 Err 回退分支之下，panic 不外泄 ⇒ 不毒化锁、应用不退出，
    /// 按「提取失败 → 扫描型」处理（真实合同正是扫描件，语义正确）。
    pub fn extract_text(&self, pdf_path: &Path) -> Result<String, PdfError> {
        debug!("提取PDF文本: {:?}", pdf_path);

        let path_buf = pdf_path.to_path_buf();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            pdf_extract::extract_text(&path_buf)
        }));

        match result {
            Ok(Ok(text)) => {
                debug!("文本提取成功: {} 字符", text.len());
                Ok(text)
            }
            Ok(Err(e)) => {
                error!("提取PDF文本失败: {:?}, 错误: {}", pdf_path, e);
                Err(PdfError::TextError(format!("文本提取失败: {}", e)))
            }
            Err(panic_payload) => {
                let msg = panic_payload
                    .downcast_ref::<String>()
                    .map(|s| s.as_str())
                    .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                    .unwrap_or("unknown panic");
                warn!(
                    "pdf-extract panic 已捕获（第三方库断言，不中断导入）: path={:?}, {}",
                    pdf_path, msg
                );
                Err(PdfError::TextError(format!("文本提取库内部断言: {}", msg)))
            }
        }
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
    /// P0-10：所有渲染经 channel 汇聚到**全进程唯一的渲染线程**执行。
    /// pdfium 的 `Pdfium::new` 每次都会调用 `FPDF_InitLibrary`，而对
    /// 「已初始化且实例仍存活（另一线程 thread_local 持有）」的 pdfium 二次初始化会
    /// **永久死锁**（`test_render_across_threads_no_deadlock` 确定性复现；
    /// 串行命令模型下历史上只有命令线程绑定过，P0-9 工作线程化后立即踩中）。
    /// 单一绑定线程保证全进程只初始化一次；`recv_timeout` 兜底防止调用方无限悬挂。
    pub fn render_page_with_limit(&self, pdf_path: &Path, page_num: u32, max_dimension: u32) -> Result<DynamicImage, PdfError> {
        debug!("渲染PDF页面: page={}, path={:?}, max_dimension={}", page_num, pdf_path, max_dimension);

        if !pdf_path.exists() {
            error!("PDF文件不存在: {:?}", pdf_path);
            return Err(PdfError::RenderError(format!("文件不存在: {}", pdf_path.display())));
        }

        let (reply_tx, reply_rx) = std::sync::mpsc::channel();
        Self::render_worker_sender()
            .send((pdf_path.to_path_buf(), page_num, max_dimension, reply_tx))
            .map_err(|_| {
                error!("渲染线程已退出: {:?}", pdf_path);
                PdfError::RenderError("渲染线程已退出".to_string())
            })?;

        match reply_rx.recv_timeout(std::time::Duration::from_secs(RENDER_TIMEOUT_SECS)) {
            Ok(result) => result,
            Err(_) => {
                error!("渲染超时({}s): page={}, path={:?}", RENDER_TIMEOUT_SECS, page_num, pdf_path);
                Err(PdfError::RenderError(format!("渲染超时({}秒)", RENDER_TIMEOUT_SECS)))
            }
        }
    }

    /// 全进程唯一的 pdfium 渲染线程（懒启动，进程级单例）
    ///
    /// 这是全进程**唯一**允许绑定 pdfium 的线程：`FPDF_InitLibrary` 生命周期内只调用一次。
    fn render_worker_sender() -> &'static std::sync::mpsc::Sender<RenderRequest> {
        static WORKER: std::sync::OnceLock<std::sync::mpsc::Sender<RenderRequest>> =
            std::sync::OnceLock::new();
        WORKER.get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel::<RenderRequest>();
            std::thread::Builder::new()
                .name("pdfium-render".to_string())
                .spawn(move || {
                    info!("pdfium 渲染线程启动（全进程唯一 FPDF_InitLibrary 绑定线程）");
                    while let Ok((path, page_num, max_dimension, reply)) = rx.recv() {
                        let result = Self::render_on_worker_thread(&path, page_num, max_dimension);
                        let _ = reply.send(result);
                    }
                    warn!("pdfium 渲染线程退出：请求通道已关闭");
                })
                .expect("启动 pdfium 渲染线程失败");
            tx
        })
    }

    /// 渲染线程内部实现（唯一允许绑定/使用 pdfium 的线程）
    fn render_on_worker_thread(
        pdf_path: &Path,
        page_num: u32,
        max_dimension: u32,
    ) -> Result<DynamicImage, PdfError> {
        if !pdf_path.exists() {
            error!("PDF文件不存在: {:?}", pdf_path);
            return Err(PdfError::RenderError(format!("文件不存在: {}", pdf_path.display())));
        }

        // 线程本地复用 Pdfium 实例（每线程只 bind 一次）
        // 文档仍按次加载：PdfDocument 借用 Pdfium，无法安全跨调用缓存
        Self::with_pdfium(|pdfium| {
            Self::render_with_pdfium(pdfium, pdf_path, page_num, max_dimension)
        })?
    }

    fn render_with_pdfium(
        pdfium: &Pdfium,
        pdf_path: &Path,
        page_num: u32,
        max_dimension: u32,
    ) -> Result<DynamicImage, PdfError> {
        info!("使用线程本地 Pdfium 实例渲染...");

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
        // 文本提取失败时按设计回退为扫描型（不返回 Err）
        let result = service.detect_type(Path::new("/nonexistent/path/file.pdf"));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), crate::models::PdfType::Scanned);
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

    /// P0-8 回归：真实合同 PDF（非 Identity-H 字体 CMap 触发 pdf-extract 断言）
    /// 不得把 panic 传播出来，detect_type 必须正常返回（回退扫描型）。
    /// 仓库未携带样本时跳过（不判失败）。
    #[test]
    fn test_detect_type_real_contract_pdf_no_panic() {
        let first = std::fs::read_dir(Path::new("../PDF file"))
            .ok()
            .and_then(|it| {
                it.filter_map(|e| e.ok()).find(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |x| x.eq_ignore_ascii_case("pdf"))
                })
            });
        let Some(entry) = first else {
            eprintln!("仓库无真实样本（../PDF file），跳过 P0-8 回归");
            return;
        };

        let service = PdfService::new().unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            service.detect_type(&entry.path())
        }));
        assert!(result.is_ok(), "detect_type 不得传播 panic: {:?}", result.err());
        assert!(result.unwrap().is_ok(), "detect_type 必须返回 Ok（提取失败回退扫描型）");
    }

    /// P0-10 取证：pdfium **跨线程二次绑定 + 渲染**是否死锁/panic。
    ///
    /// 生产现象（串行命令模型下从未暴露）：命令线程先绑定并**长期持有**其实例
    /// （thread_local 随线程存活），OCR 工作线程随后 `bind_to_system_library` →
    /// `Pdfium::new` → **第二次 `FPDF_InitLibrary`（对已初始化的 pdfium）→ 永久挂起**
    /// （take6/8 实测：`首次在本线程绑定 Pdfium` 之后再无 `绑定成功`，卡 3 分钟+）。
    ///
    /// 本测试复刻生产时序：线程 A 渲染成功后**保持存活**（等价命令线程），随后
    /// 线程 B 渲染（30s 超时判死锁）。修复（全局唯一渲染线程）后本测试必须通过。
    /// 需要 `../PDF file` 真实样本与 `src-tauri/pdfium.dll`；缺样本时跳过。
    #[test]
    fn test_render_across_threads_no_deadlock() {
        // 测试二进制位于 target/debug/deps：把 pdfium.dll 所在目录塞进 DLL 搜索路径
        let tauri_dir = std::fs::canonicalize(Path::new("..")).unwrap_or_default();
        let path_old = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", format!("{};{}", tauri_dir.to_string_lossy(), path_old));

        let pdf = std::fs::read_dir(Path::new("../PDF file"))
            .ok()
            .and_then(|it| {
                it.filter_map(|e| e.ok()).find(|e| {
                    e.path()
                        .extension()
                        .map_or(false, |x| x.eq_ignore_ascii_case("pdf"))
                })
            })
            .map(|e| e.path());
        let Some(pdf) = pdf else {
            eprintln!("仓库无真实样本（../PDF file），跳过 P0-10 取证");
            return;
        };

        fn render_in_thread(pdf: std::path::PathBuf) -> Result<u32, String> {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let svc = match PdfService::new() {
                    Ok(s) => s,
                    Err(e) => { let _ = tx.send(Err(format!("new failed: {}", e))); return; }
                };
                let r = svc
                    .render_page_with_limit(&pdf, 1, 1000)
                    .map(|img| img.width())
                    .map_err(|e| e.to_string());
                let _ = tx.send(r);
            });
            rx.recv_timeout(std::time::Duration::from_secs(30))
                .unwrap_or_else(|_| Err("TIMEOUT: 跨线程渲染 30s 未返回（二次 FPDF_InitLibrary 死锁）".to_string()))
        }

        // 场景 1（生产时序）：线程 A 渲染并**长期存活**（park 到测试结束，其
        // thread_local pdfium 实例不销毁 = 命令线程的真实状态），
        // 线程 B 再渲染 —— 旧实现在此挂起（对已初始化 pdfium 的第二次 FPDF_InitLibrary）
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();
        let (release_tx, release_rx) = std::sync::mpsc::channel::<()>();
        let keep_pdf = pdf.clone();
        let holder = std::thread::spawn(move || {
            let svc = PdfService::new().unwrap();
            let r = svc.render_page_with_limit(&keep_pdf, 1, 1000).map(|i| i.width());
            let _ = done_tx.send(());
            // 保持线程与其实例存活，直到测试发出释放信号
            let _ = release_rx.recv();
            r
        });
        done_rx
            .recv_timeout(std::time::Duration::from_secs(30))
            .expect("线程 A（持有存活实例）渲染超时");
        let b = render_in_thread(pdf.clone());
        let _ = release_tx.send(());
        assert!(b.is_ok(), "持有存活实例期间线程 B 渲染失败/死锁: {:?}", b);
        assert!(b.unwrap() > 0, "线程 B 渲染出空图");
        let a = holder.join().unwrap();
        assert!(a.is_ok(), "线程 A 渲染失败: {:?}", a);
        assert!(*a.as_ref().unwrap() > 0, "线程 A 渲染出空图: {:?}", a);

        // 场景 2：并发渲染（两调用线程同时请求）
        let p1 = pdf.clone();
        let p2 = pdf.clone();
        let t1 = std::thread::spawn(move || render_in_thread(p1));
        let b2 = render_in_thread(p2);
        assert!(b2.is_ok(), "并发期间线程渲染失败/死锁: {:?}", b2);
        let a2 = t1.join().unwrap();
        assert!(a2.is_ok(), "并发期间另一线程渲染失败/死锁: {:?}", a2);
    }
}