use crate::db::{Db, get_setting, SETTING_OCR_MAX_IMAGE_DIMENSION};
use crate::services::memory_monitor::{can_start_task, estimate_task_memory, get_system_memory_info, MemoryInfo};
use crate::services::model_manager::{DownloadGuide, ModelManager, ModelType};
use crate::services::ocr_service::OcrService;
use crate::services::pdf_service::PdfService;
use crate::services::search_service::SearchService;
use crate::services::task_queue::{QueueStatus, TaskQueue};
use crate::commands::settings::DEFAULT_OCR_MAX_IMAGE_DIMENSION;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, State};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrStatus {
    pub available: bool,
    pub models_ready: bool,
    pub missing_files: Vec<String>,
    pub models_dir: String,
    pub model_type: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OcrProgress {
    pub pdf_id: i64,
    pub current: u32,
    pub total: u32,
    pub status: String,
}

/// 获取 OCR 状态
#[tauri::command]
pub fn get_ocr_status(
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<OcrStatus, String> {
    debug!("开始获取OCR状态");

    let svc = match ocr_service.lock() {
        Ok(s) => s,
        Err(e) => {
            error!("获取OCR服务锁失败: {}", e);
            return Err(format!("OCR服务锁定失败: {}", e));
        }
    };

    let status = svc.get_status();
    let model_type = svc.model_type();

    debug!(
        "OCR状态: available={}, models_ready={}, missing={:?}, model_type={}",
        status.available, status.models_ready, status.missing_files, model_type
    );

    Ok(OcrStatus {
        available: status.available,
        models_ready: status.models_ready,
        missing_files: status.missing_files,
        models_dir: status.models_dir,
        model_type: model_type.to_string(),
    })
}

/// 获取下载指导
#[tauri::command]
pub fn get_ocr_download_guide(
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Vec<DownloadGuide> {
    let model_type = ocr_service.lock()
        .map(|s| s.model_type())
        .unwrap_or(ModelType::Mobile);
    ModelManager::get_download_guide(model_type)
}

/// 设置模型类型
#[tauri::command]
pub fn set_ocr_model_type(
    model_type: String,
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<(), String> {
    let model_type: ModelType = model_type.parse()
        .map_err(|e| format!("无效的模型类型: {}", e))?;

    let mut svc = ocr_service.lock().map_err(|e| {
        error!("获取OCR服务锁失败: {}", e);
        format!("OCR服务锁定失败: {}", e)
    })?;

    svc.set_model_type(model_type);
    info!("模型类型已设置为: {}", model_type);
    Ok(())
}

/// 下载 OCR 模型
#[tauri::command]
pub async fn download_ocr_models(
    ocr_service: State<'_, Mutex<OcrService>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("开始下载 OCR 模型");

    let (model_manager, model_type) = {
        let svc = ocr_service.lock().map_err(|e| {
            error!("获取OCR服务锁失败: {}", e);
            format!("OCR服务锁定失败: {}", e)
        })?;
        (svc.model_manager(), svc.model_type())
    };

    model_manager.download_models(app_handle, model_type).await?;

    info!("OCR 模型下载完成");
    Ok(())
}

/// 取消模型下载
#[tauri::command]
pub fn cancel_ocr_download(
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<(), String> {
    info!("取消 OCR 模型下载");

    let model_manager = {
        let svc = ocr_service.lock().map_err(|e| {
            error!("获取OCR服务锁失败: {}", e);
            format!("OCR服务锁定失败: {}", e)
        })?;
        svc.model_manager()
    };

    model_manager.cancel_download();

    Ok(())
}

/// 获取队列状态
#[tauri::command]
pub fn get_ocr_queue_status(
    task_queue: State<'_, Mutex<TaskQueue>>,
) -> Result<QueueStatus, String> {
    let queue = task_queue.lock().map_err(|e| {
        error!("获取任务队列锁失败: {}", e);
        format!("任务队列锁定失败: {}", e)
    })?;

    Ok(queue.get_status())
}

/// 取消排队中的任务
#[tauri::command]
pub fn cancel_ocr_task(
    pdf_id: i64,
    task_queue: State<'_, Mutex<TaskQueue>>,
) -> Result<bool, String> {
    info!("取消OCR任务: pdf_id={}", pdf_id);

    let mut queue = task_queue.lock().map_err(|e| {
        error!("获取任务队列锁失败: {}", e);
        format!("任务队列锁定失败: {}", e)
    })?;

    Ok(queue.cancel(pdf_id))
}

/// 获取系统内存信息
#[tauri::command]
pub fn get_memory_info() -> MemoryInfo {
    get_system_memory_info()
}

/// 开始 OCR 处理
#[tauri::command]
pub async fn start_ocr(
    pdf_id: i64,
    db: State<'_, Db>,
    ocr_service: State<'_, Mutex<OcrService>>,
    pdf_service: State<'_, Mutex<PdfService>>,
    search_service: State<'_, Mutex<SearchService>>,
    task_queue: State<'_, Mutex<TaskQueue>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("请求OCR处理: pdf_id={}", pdf_id);

    // 获取 OCR 最大图像尺寸设置
    let max_image_dimension: u32 = {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;
        get_setting(&conn, SETTING_OCR_MAX_IMAGE_DIMENSION)
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(DEFAULT_OCR_MAX_IMAGE_DIMENSION)
    };
    info!("OCR 最大图像尺寸: {}", max_image_dimension);

    // 获取 PDF 信息
    let (storage_path, page_count, filename, folder_id): (String, i32, String, Option<i64>) = {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        conn.query_row(
            "SELECT storage_path, page_count, filename, folder_id FROM pdfs WHERE id = ?1",
            rusqlite::params![pdf_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).map_err(|e| {
            error!("PDF不存在 (pdf_id={}): {}", pdf_id, e);
            format!("PDF不存在: {}", e)
        })?
    };

    info!("PDF信息: filename={}, pages={}, storage={}", filename, page_count, storage_path);

    // 检查 OCR 服务是否可用
    {
        let mut ocr_svc = ocr_service.lock().map_err(|e| {
            error!("获取OCR服务锁失败: {}", e);
            format!("OCR服务锁定失败: {}", e)
        })?;

        if !ocr_svc.is_available() {
            if let Err(e) = ocr_svc.init_ocr() {
                error!("OCR模型初始化失败: {}", e);
                return Err(format!("OCR服务不可用: {}", e));
            }
        }

        if !ocr_svc.is_available() {
            error!("OCR服务不可用");
            return Err("OCR服务不可用，请先下载模型文件".to_string());
        }
    }

    // 估算所需内存
    let required_memory = estimate_task_memory(page_count as u32, max_image_dimension);
    info!("估算任务内存: {} MB", required_memory / 1024 / 1024);

    // 检查内存是否足够
    if !can_start_task(required_memory) {
        warn!("内存不足，任务将加入队列等待");
    }

    // 将任务加入队列
    let position = {
        let mut queue = task_queue.lock().map_err(|e| {
            error!("获取任务队列锁失败: {}", e);
            format!("任务队列锁定失败: {}", e)
        })?;

        match queue.enqueue(pdf_id) {
            Ok(pos) => {
                if pos > 0 {
                    info!("任务加入队列，位置: {}", pos);
                    // 发送排队状态事件
                    let _ = app_handle.emit("ocr-queued", serde_json::json!({
                        "pdf_id": pdf_id,
                        "position": pos
                    }));
                }
                pos
            }
            Err(e) => {
                warn!("任务入队失败: {}", e);
                return Err(e);
            }
        }
    };

    // 如果任务在队列中等待，直接返回成功（后续会自动处理）
    if position > 0 {
        return Ok(());
    }

    // 任务立即开始执行
    // 更新状态为 processing
    {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        conn.execute(
            "UPDATE pdfs SET status = 'processing', updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![pdf_id],
        ).map_err(|e| {
            error!("更新PDF状态失败: {}", e);
            format!("更新状态失败: {}", e)
        })?;
    }

    // 发送进度事件
    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: 0,
        total: page_count as u32,
        status: "processing".to_string(),
    });

    let mut success_count = 0;
    let mut error_count = 0;

    // 处理每一页
    for page_num in 1..=page_count {
        info!("处理PDF页面: pdf_id={}, page={}/{}", pdf_id, page_num, page_count);

        // 动态调整图像尺寸：如果内存紧张，降低渲染尺寸
        let current_dimension = if crate::services::memory_monitor::is_low_memory() {
            warn!("内存紧张，降低渲染尺寸: {} -> 500", max_image_dimension);
            500
        } else {
            max_image_dimension
        };

        match process_page(
            pdf_id,
            page_num,
            &storage_path,
            &ocr_service,
            &pdf_service,
            &db,
            &search_service,
            &filename,
            folder_id,
            current_dimension,
        ) {
            Ok(_) => {
                success_count += 1;
                info!("页面处理成功: pdf_id={}, page={}", pdf_id, page_num);
            }
            Err(e) => {
                error_count += 1;
                error!("页面处理失败: pdf_id={}, page={}, 错误: {}", pdf_id, page_num, e);
            }
        }

        // 显式释放内存：触发垃圾回收
        // 在 Rust 中，drop 会释放内存，但实际释放时机取决于分配器
        // 这里我们通过记录内存状态来监控
        let mem_info = crate::services::memory_monitor::get_system_memory_info();
        info!("内存状态: 可用={:.1}GB, 已用={:.1}%",
            mem_info.available as f64 / 1024.0 / 1024.0 / 1024.0,
            mem_info.used_percent
        );

        // 发送进度
        let _ = app_handle.emit("ocr-progress", OcrProgress {
            pdf_id,
            current: page_num as u32,
            total: page_count as u32,
            status: "processing".to_string(),
        });
    }

    // 更新最终状态
    let final_status = if error_count == 0 {
        "done"
    } else if success_count == 0 {
        "error"
    } else {
        "done"
    };

    let error_message = if error_count > 0 {
        Some(format!("{}个页面处理失败", error_count))
    } else {
        None
    };

    {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        match error_message {
            Some(ref msg) => {
                conn.execute(
                    "UPDATE pdfs SET status = ?1, error_message = ?2, updated_at = datetime('now') WHERE id = ?3",
                    rusqlite::params![final_status, msg, pdf_id],
                ).map_err(|e| format!("更新状态失败: {}", e))?;
            }
            None => {
                conn.execute(
                    "UPDATE pdfs SET status = ?1, error_message = NULL, updated_at = datetime('now') WHERE id = ?2",
                    rusqlite::params![final_status, pdf_id],
                ).map_err(|e| format!("更新状态失败: {}", e))?;
            }
        }
    }

    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: page_count as u32,
        total: page_count as u32,
        status: final_status.to_string(),
    });

    info!("OCR处理完成: pdf_id={}, filename={}, 成功={}, 失败={}", pdf_id, filename, success_count, error_count);

    // 智能内存管理：根据内存状态决定是否释放模型
    {
        let mem_info = crate::services::memory_monitor::get_system_memory_info();
        const MEMORY_THRESHOLD: u64 = 3 * 1024 * 1024 * 1024; // 3GB

        if mem_info.available < MEMORY_THRESHOLD {
            info!("内存紧张 (可用: {:.1}GB)，释放 OCR 模型以节省内存",
                mem_info.available as f64 / 1024.0 / 1024.0 / 1024.0);
            let mut ocr_svc = ocr_service.lock().map_err(|e| {
                error!("获取OCR服务锁失败: {}", e);
                format!("OCR服务锁定失败: {}", e)
            })?;
            ocr_svc.unload_ocr();
        } else {
            info!("内存充足 (可用: {:.1}GB)，保留 OCR 模型以加速后续识别",
                mem_info.available as f64 / 1024.0 / 1024.0 / 1024.0);
        }
    }

    // 标记任务完成，开始下一个
    {
        let mut queue = task_queue.lock().map_err(|e| {
            error!("获取任务队列锁失败: {}", e);
            format!("任务队列锁定失败: {}", e)
        })?;
        queue.complete(pdf_id);
        let next_task = queue.get_next();
        // 注意：这里简化处理，不自动启动下一个任务
        // 用户需要手动点击下一个任务的 OCR 按钮
        if let Some(next) = next_task {
            info!("队列中有下一个任务: pdf_id={}", next.pdf_id);
            let _ = app_handle.emit("ocr-queued", serde_json::json!({
                "pdf_id": next.pdf_id,
                "position": 0
            }));
        }
    }

    Ok(())
}

fn process_page(
    pdf_id: i64,
    page_num: i32,
    storage_path: &str,
    ocr_service: &State<'_, Mutex<OcrService>>,
    pdf_service: &State<'_, Mutex<PdfService>>,
    db: &State<'_, Db>,
    search_service: &State<'_, Mutex<SearchService>>,
    filename: &str,
    folder_id: Option<i64>,
    max_image_dimension: u32,
) -> Result<(), String> {
    debug!("渲染PDF页面: page={}, path={}, max_dimension={}", page_num, storage_path, max_image_dimension);

    // 渲染 PDF 页面为图像（使用动态尺寸）
    let image = {
        let pdf_svc = pdf_service.lock().map_err(|e| {
            error!("获取PDF服务锁失败: {}", e);
            format!("PDF服务锁定失败: {}", e)
        })?;
        pdf_svc.render_page_with_limit(std::path::Path::new(storage_path), page_num as u32, max_image_dimension)
            .map_err(|e| {
                error!("渲染PDF页面失败: page={}, 错误: {}", page_num, e);
                format!("渲染页面失败: {}", e)
            })?
    };

    debug!("PDF页面渲染成功: page={}, size={}x{}", page_num, image.width(), image.height());

    // OCR 识别
    let text = {
        let mut ocr_svc = ocr_service.lock().map_err(|e| {
            error!("获取OCR服务锁失败: {}", e);
            format!("OCR服务锁定失败: {}", e)
        })?;
        ocr_svc.recognize_with_limit(&image, max_image_dimension)
            .map_err(|e| {
                error!("OCR识别失败: page={}, 错误: {}", page_num, e);
                format!("OCR识别失败: {}", e)
            })?
    };

    info!("OCR识别成功: page={}, 文本长度={}", page_num, text.len());

    // 保存到数据库
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    conn.execute(
        "INSERT OR REPLACE INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status)
         VALUES (?1, ?2, ?3, 'done')",
        rusqlite::params![pdf_id, page_num, text],
    )
    .map_err(|e| {
        error!("保存OCR结果失败: page={}, 错误: {}", page_num, e);
        format!("保存结果失败: {}", e)
    })?;

    // 获取 page_id
    let page_id: i64 = conn.query_row(
        "SELECT id FROM pdf_pages WHERE pdf_id = ?1 AND page_number = ?2",
        rusqlite::params![pdf_id, page_num],
        |row| row.get(0),
    ).unwrap_or(0);

    debug!("OCR结果已保存: page={}, page_id={}", page_num, page_id);

    // 添加到搜索索引
    if page_id > 0 {
        let mut search_svc = search_service.lock().map_err(|e| {
            error!("获取搜索服务锁失败: {}", e);
            format!("搜索服务锁定失败: {}", e)
        })?;
        match search_svc.index_page(
            page_id as u64,
            pdf_id as u64,
            folder_id,
            page_num as u32,
            filename,
            &text,
        ) {
            Ok(_) => info!("页面已添加到搜索索引: page_id={}, pdf_id={}", page_id, pdf_id),
            Err(e) => warn!("添加搜索索引失败: page_id={}, 错误: {}", page_id, e),
        }
    }

    Ok(())
}