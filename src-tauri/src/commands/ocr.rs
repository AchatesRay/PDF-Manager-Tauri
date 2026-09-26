use crate::db::{Db, get_setting, SETTING_OCR_MAX_IMAGE_DIMENSION, SETTING_OCR_PREPROCESS_MODE};
use crate::services::memory_monitor::{can_start_task, estimate_task_memory, get_system_memory_info, MemoryInfo};
use crate::services::model_manager::{DownloadGuide, ModelManager, ModelType};
use crate::services::ocr_service::{OcrService, PreprocessMode};
use crate::services::pdf_service::PdfService;
use crate::services::search_service::{PageIndexEntry, SearchService};
use crate::services::task_queue::{QueueStatus, TaskQueue};
use crate::commands::settings::DEFAULT_OCR_MAX_IMAGE_DIMENSION;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, Manager, State};
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrStatus {
    pub available: bool,
    pub models_ready: bool,
    pub missing_files: Vec<String>,
    pub models_dir: String,
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

    debug!(
        "OCR状态: available={}, models_ready={}, missing={:?}",
        status.available, status.models_ready, status.missing_files
    );

    Ok(OcrStatus {
        available: status.available,
        models_ready: status.models_ready,
        missing_files: status.missing_files,
        models_dir: status.models_dir,
    })
}

/// 获取下载指导
#[tauri::command]
pub fn get_ocr_download_guide() -> Vec<DownloadGuide> {
    ModelManager::get_download_guide(ModelType::Balanced)
}

/// 重新检测模型状态并尝试加载
#[tauri::command]
pub fn refresh_ocr_status(
    ocr_service: State<'_, Mutex<OcrService>>,
) -> Result<OcrStatus, String> {
    info!("重新检测 OCR 模型状态");

    // 检查并更新服务
    let mut svc = ocr_service.lock().map_err(|e| {
        error!("获取OCR服务锁失败: {}", e);
        format!("OCR服务锁定失败: {}", e)
    })?;

    // 如果模型文件存在但未加载，尝试加载
    let status = svc.get_status();
    if status.models_ready && !svc.is_available() {
        info!("模型文件存在但未加载，尝试加载");
        if let Err(e) = svc.init_ocr() {
            warn!("模型加载失败: {}", e);
        }
    }

    let final_status = svc.get_status();
    info!("OCR 状态检测完成: models_ready={}, available={}",
          final_status.models_ready, final_status.available);

    Ok(OcrStatus {
        available: final_status.available,
        models_ready: final_status.models_ready,
        missing_files: final_status.missing_files,
        models_dir: final_status.models_dir,
    })
}

/// 下载 OCR 模型
#[tauri::command]
pub async fn download_ocr_models(
    ocr_service: State<'_, Mutex<OcrService>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("开始下载 OCR 模型");

    let model_manager = {
        let svc = ocr_service.lock().map_err(|e| {
            error!("获取OCR服务锁失败: {}", e);
            format!("OCR服务锁定失败: {}", e)
        })?;
        svc.model_manager()
    };

    // 使用固定的 Balanced 模型
    model_manager.download_models(app_handle, ModelType::Balanced).await?;

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
    db: State<'_, Db>,
    task_queue: State<'_, Mutex<TaskQueue>>,
) -> Result<bool, String> {
    info!("取消OCR任务: pdf_id={}", pdf_id);

    let cancelled = {
        let mut queue = task_queue.lock().map_err(|e| {
            error!("获取任务队列锁失败: {}", e);
            format!("任务队列锁定失败: {}", e)
        })?;
        queue.cancel(pdf_id)
    };

    // 持久化同步：取消成功即从 ocr_queue 移除
    if cancelled {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;
        if let Err(e) = remove_queue_row(&conn, pdf_id) {
            warn!("移除持久化任务失败: pdf_id={}, {}", pdf_id, e);
        }
    }

    Ok(cancelled)
}

/// 获取系统内存信息
#[tauri::command]
pub fn get_memory_info() -> MemoryInfo {
    get_system_memory_info()
}

/// 读取单个 PDF 的 OCR 任务信息
fn load_pdf_job_info(
    db: &State<'_, Db>,
    pdf_id: i64,
) -> Result<(String, i32, String, Option<i64>), String> {
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    conn.query_row(
        "SELECT storage_path, page_count, filename, folder_id FROM pdfs WHERE id = ?1",
        rusqlite::params![pdf_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )
    .map_err(|e| {
        error!("PDF不存在 (pdf_id={}): {}", pdf_id, e);
        format!("PDF不存在: {}", e)
    })
}

/// 将 PDF 状态写入数据库
fn set_pdf_status(
    db: &State<'_, Db>,
    pdf_id: i64,
    status: &str,
    error_message: Option<&str>,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    match error_message {
        Some(msg) => {
            conn.execute(
                "UPDATE pdfs SET status = ?1, error_message = ?2, updated_at = datetime('now') WHERE id = ?3",
                rusqlite::params![status, msg, pdf_id],
            )
            .map(|_| ())
        }
        None => {
            conn.execute(
                "UPDATE pdfs SET status = ?1, error_message = NULL, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![status, pdf_id],
            )
            .map(|_| ())
        }
    }
    .map_err(|e| {
        error!("更新PDF状态失败: pdf_id={}, {}", pdf_id, e);
        format!("更新状态失败: {}", e)
    })
}

/// 持久化：任务入队成功后写入 ocr_queue（顺序号 = 当前最大 +1，完成后删行）
fn insert_queue_row(conn: &rusqlite::Connection, pdf_id: i64) -> Result<(), rusqlite::Error> {
    conn.execute(
        "INSERT OR IGNORE INTO ocr_queue (pdf_id, position, created_at)
         VALUES (?1, (SELECT COALESCE(MAX(position), -1) + 1 FROM ocr_queue), datetime('now'))",
        rusqlite::params![pdf_id],
    )?;
    Ok(())
}

/// 持久化：任务完成/取消/加载失败后移除
fn remove_queue_row(conn: &rusqlite::Connection, pdf_id: i64) -> Result<(), rusqlite::Error> {
    conn.execute("DELETE FROM ocr_queue WHERE pdf_id = ?1", rusqlite::params![pdf_id])?;
    Ok(())
}

/// 宽松解析 ocr_queue.created_at（datetime('now') 为 `YYYY-MM-DD HH:MM:SS` UTC）
fn parse_queue_created_at(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    use chrono::TimeZone;
    chrono::DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&chrono::Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
                .ok()
                .map(|n| chrono::Utc.from_utc_datetime(&n))
        })
}

/// 启动时恢复持久化 OCR 队列（T4）。
///
/// 步骤：① processing → pending（崩溃恢复）② 清理已删除/已完结 PDF 的队列行
/// ③ 按 position 顺序装载到 TaskQueue（running 保持 None）。
/// 返回恢复的 pdf_id 列表；调用方随后决定是否自动续跑。
pub fn restore_persisted_queue(
    conn: &rusqlite::Connection,
    queue: &mut TaskQueue,
) -> Vec<i64> {
    // ① 崩溃恢复：上次执行中被打断的任务置回 pending
    match conn.execute("UPDATE pdfs SET status = 'pending', error_message = NULL WHERE status = 'processing'", []) {
        Ok(n) if n > 0 => info!("崩溃恢复: {} 个 processing 任务置回 pending", n),
        Ok(_) => {}
        Err(e) => warn!("重置 processing 状态失败: {}", e),
    }

    // ② 清理失效行：PDF 已不存在；或已 done/error（不自动重跑已完结任务）
    if let Err(e) = conn.execute("DELETE FROM ocr_queue WHERE pdf_id NOT IN (SELECT id FROM pdfs)", []) {
        warn!("清理失效队列行失败: {}", e);
    }
    if let Err(e) = conn.execute(
        "DELETE FROM ocr_queue WHERE pdf_id IN (SELECT id FROM pdfs WHERE status NOT IN ('pending', 'processing'))",
        [],
    ) {
        warn!("清理已完结队列行失败: {}", e);
    }

    // ③ 装载
    let rows: Vec<(i64, String)> = match conn.prepare("SELECT pdf_id, created_at FROM ocr_queue ORDER BY position, created_at") {
        Ok(mut stmt) => stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map(|it| it.filter_map(|r| r.ok()).collect())
            .unwrap_or_default(),
        Err(e) => {
            warn!("读取持久化队列失败: {}", e);
            Vec::new()
        }
    };

    if rows.is_empty() {
        return Vec::new();
    }

    let tasks: Vec<crate::services::task_queue::OcrTask> = rows
        .iter()
        .map(|(pdf_id, created)| crate::services::task_queue::OcrTask {
            pdf_id: *pdf_id,
            created_at: parse_queue_created_at(created).unwrap_or_else(chrono::Utc::now),
        })
        .collect();
    let ids: Vec<i64> = tasks.iter().map(|t| t.pdf_id).collect();

    queue.restore(tasks);
    info!("恢复持久化 OCR 队列: {:?}", ids);
    ids
}

/// 应用启动时自动续跑恢复的队列（由 setup 后台线程调用）
///
/// 模型文件未就绪时不做任何处理：任务保留在持久化队列中等待用户下载模型，
/// 不会被标 error（避免误伤），UI 队列状态可见。
pub fn resume_ocr_queue(app_handle: tauri::AppHandle) {
    info!("启动恢复：检查持久化 OCR 队列");

    // 模型文件就绪才自动续跑
    let models_ready = app_handle
        .state::<Mutex<OcrService>>()
        .lock()
        .map(|svc| {
            let st = svc.get_status();
            st.models_ready || st.available
        })
        .unwrap_or(false);
    if !models_ready {
        warn!("OCR 模型未就绪，持久化队列保持等待（下载模型后任意一次 OCR 会继续调度）");
        return;
    }

    // 取队首（置为 running）
    let first = app_handle
        .state::<Mutex<TaskQueue>>()
        .lock()
        .map(|mut q| q.get_next().map(|t| t.pdf_id))
        .unwrap_or(None);
    let Some(first) = first else {
        debug!("无可恢复任务");
        return;
    };
    info!("恢复执行 OCR 队列，队首 pdf_id={}", first);

    // 与 start_ocr 共用工作线程主体（含致命错误兜底清理）
    run_queue_worker(app_handle, first);
}

/// 开始 OCR 处理
///
/// 队列语义：命令内只做校验/清理/入队，**执行转交独立工作线程**（P0-9）；
/// 工作线程内 `complete` + `get_next` 后由后端继续处理下一任务，
/// 不依赖前端再次调用 `start_ocr`。
///
/// 之所以不在命令内循环：同步命令是串行分发的，长任务会阻塞全部其它 IPC
/// （UI 冻结、排队/取消不可达）——详见 `spawn_queue_worker` 注释。
#[tauri::command]
pub fn start_ocr(
    pdf_id: i64,
    force: Option<bool>,
    db: State<'_, Db>,
    ocr_service: State<'_, Mutex<OcrService>>,
    search_service: State<'_, Mutex<SearchService>>,
    task_queue: State<'_, Mutex<TaskQueue>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("请求OCR处理: pdf_id={}, force={:?}", pdf_id, force);

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

    // 获取 PDF 信息（日志、force 清理与内存估算使用）
    let (storage_path, page_count, filename, _folder_id) = load_pdf_job_info(&db, pdf_id)?;

    info!("PDF信息: filename={}, pages={}, storage={}", filename, page_count, storage_path);

    // 如果是强制重新识别，清除已有结果
    if force.unwrap_or(false) {
        info!("强制重新识别，清除已有 OCR 结果: pdf_id={}", pdf_id);

        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        // 清除 OCR 结果
        conn.execute(
            "DELETE FROM pdf_pages WHERE pdf_id = ?1",
            rusqlite::params![pdf_id],
        ).map_err(|e| {
            error!("清除 OCR 结果失败: {}", e);
            format!("清除结果失败: {}", e)
        })?;

        // 重置状态为 pending
        conn.execute(
            "UPDATE pdfs SET status = 'pending', error_message = NULL, updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![pdf_id],
        ).map_err(|e| {
            error!("重置状态失败: {}", e);
            format!("重置状态失败: {}", e)
        })?;

        drop(conn);

        // 同步删除搜索索引，避免 force 后仍能搜到旧文本
        if let Ok(mut ss) = search_service.lock() {
            if let Err(e) = ss.delete_pdf(pdf_id as u64) {
                warn!("force 重识别时删除搜索索引失败: pdf_id={}, {}", pdf_id, e);
            }
        } else {
            warn!("force 重识别时获取搜索服务锁失败，跳过索引清理");
        }
    }

    // 获取模型类型并检查 OCR 服务是否可用
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

    // 估算所需内存（使用 Balanced 模型）
    let required_memory = estimate_task_memory(page_count as u32, max_image_dimension, &ModelType::Balanced);
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

    // 持久化入队（T4：重启恢复 pending 任务；完成/取消/删除时删行）
    {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;
        if let Err(e) = insert_queue_row(&conn, pdf_id) {
            warn!("持久化 OCR 任务失败（内存队列仍有效）: pdf_id={}, {}", pdf_id, e);
        }
    }

    // 如果任务在队列中等待，直接返回成功（当前运行中的任务结束后由后端调度）
    if position > 0 {
        return Ok(());
    }

    // 队首任务：转交独立工作线程执行（P0-9）
    //
    // 本应用的同步命令是**串行分发**的（Phase 5 v2 实测：OCR 期间首次探测延迟=整轮 OCR 时长 9194ms，
    // 其后样本才是 7~10ms——报告当时误读为「OCR 期间 IPC 无卡顿」）。若在命令内跑完整个队列：
    //   ① OCR 期间所有其它 IPC（列表/排队/取消/设置）被阻塞 → UI 冻结；
    //   ② 第二个 start_ocr 无法入队 → pending 队列、「排队中」、取消路径全部不可达。
    // 转交工作线程后命令立即返回，分发线程恢复响应，队列语义（后端自动串行调度）保持不变。
    spawn_queue_worker(app_handle, pdf_id);
    Ok(())
}

/// 把队列执行转交独立工作线程（不占用命令分发线程）
pub fn spawn_queue_worker(app_handle: tauri::AppHandle, head: i64) {
    let spawned = std::thread::Builder::new()
        .name(format!("ocr-worker-{}", head))
        .spawn(move || run_queue_worker(app_handle, head));
    match spawned {
        Ok(_) => info!("OCR 工作线程已启动: head={}", head),
        Err(e) => error!("启动 OCR 工作线程失败: head={}, {}", head, e),
    }
}

/// 工作线程主体：执行队列循环；致命错误时兜底清理（防任务悬挂 / 重启后反复崩溃）。
///
/// 外层 catch_unwind：工作线程 panic 不会有人观察到（stderr 被丢弃），线程静默死亡后
/// 任务永久卡在 processing —— 必须把 panic 转成日志 + 兜底清理。
fn run_queue_worker(app_handle: tauri::AppHandle, head: i64) {
    let handle_for_inner = app_handle.clone();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
        run_queue_worker_inner(handle_for_inner, head)
    }));
    match result {
        Ok(Ok(())) => debug!("OCR 工作线程正常结束: head={}", head),
        Ok(Err(e)) => error!("OCR 工作线程异常: head={}, err={}", head, e),
        Err(panic_payload) => {
            let msg = panic_payload
                .downcast_ref::<String>()
                .map(|s| s.as_str())
                .or_else(|| panic_payload.downcast_ref::<&str>().copied())
                .unwrap_or("unknown panic")
                .to_string();
            error!("OCR 工作线程 PANIC（已捕获）: head={}, {}", head, msg);
            // panic 时 inner 的 states 借用已失效，重新从 handle 取
            fail_fast_worker_by_handle(&app_handle, head, &msg);
        }
    }
}

fn run_queue_worker_inner(app_handle: tauri::AppHandle, head: i64) -> Result<(), String> {
    let db = app_handle.state::<Db>();
    let ocr_service = app_handle.state::<Mutex<OcrService>>();
    let pdf_service = app_handle.state::<Mutex<PdfService>>();
    let search_service = app_handle.state::<Mutex<SearchService>>();
    let task_queue = app_handle.state::<Mutex<TaskQueue>>();

    let result = run_queue_loop(
        head,
        &db,
        &ocr_service,
        &pdf_service,
        &search_service,
        &task_queue,
        &app_handle,
    );
    if let Err(ref e) = result {
        error!("OCR 工作线程执行失败: head={}, err={}", head, e);
        fail_fast_worker(&app_handle, &db, &task_queue, head, e);
    }
    result
}

/// panic 路径的兜底（重新从 handle 取 states，因为 panic 时 inner 的借用已失效）
fn fail_fast_worker_by_handle(app_handle: &tauri::AppHandle, head: i64, err: &str) {
    let db = app_handle.state::<Db>();
    let task_queue = app_handle.state::<Mutex<TaskQueue>>();
    fail_fast_worker(app_handle, &db, &task_queue, head, err);
}

/// 致命错误兜底：processing→error、清空内存队列与持久化队列、通知前端。
/// 选择清空而非重试：模型缺失/磁盘故障等致命条件在本进程内不会自愈，
/// 保留任务只会让重启后反复走进同一错误（崩溃环）。
fn fail_fast_worker(
    app_handle: &tauri::AppHandle,
    db: &State<'_, Db>,
    task_queue: &State<'_, Mutex<TaskQueue>>,
    head: i64,
    err: &str,
) {
    let msg = format!("OCR 执行失败: {}", err);
    match db.lock() {
        Ok(conn) => {
            let _ = conn.execute(
                "UPDATE pdfs SET status = 'error', error_message = ?1 WHERE status = 'processing'",
                rusqlite::params![msg],
            );
            let _ = conn.execute("DELETE FROM ocr_queue", []);
        }
        Err(e) => warn!("兜底清理获取数据库锁失败: {}", e),
    }
    if let Ok(mut q) = task_queue.lock() {
        q.clear();
    }
    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id: head,
        current: 0,
        total: 0,
        status: "error".to_string(),
    });
    warn!("OCR 工作线程兜底清理完成: head={}", head);
}

/// 队首任务执行循环：处理当前任务，完成后由后端取下一任务继续执行。
///
/// 前置条件：`first_pdf_id` 已由 `enqueue`/`get_next` 置为 running。
#[allow(clippy::too_many_arguments)]
fn run_queue_loop(
    first_pdf_id: i64,
    db: &State<'_, Db>,
    ocr_service: &State<'_, Mutex<OcrService>>,
    pdf_service: &State<'_, Mutex<PdfService>>,
    search_service: &State<'_, Mutex<SearchService>>,
    task_queue: &State<'_, Mutex<TaskQueue>>,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    // 确保模型已加载（启动恢复路径没有 start_ocr 的前置检查）
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

    // T6：读取并应用预处理模式（每次执行时读取，设置修改对下一任务生效）
    let preprocess_mode = {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;
        get_setting(&conn, SETTING_OCR_PREPROCESS_MODE)
            .and_then(|v| PreprocessMode::parse(&v))
            .unwrap_or_default()
    };
    if let Ok(mut ocr_svc) = ocr_service.lock() {
        ocr_svc.set_preprocess_mode(preprocess_mode);
    }
    info!("OCR 预处理模式: {}", preprocess_mode.as_str());

    let (mut storage_path, mut page_count, mut filename, mut folder_id) =
        load_pdf_job_info(db, first_pdf_id)?;
    info!(
        "队首任务开始执行: pdf_id={}, filename={}, pages={}",
        first_pdf_id, filename, page_count
    );

    // 每次执行时读取设置（启动恢复路径同样生效，设置修改对下一任务立即生效）
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

    let mut current_pdf_id = first_pdf_id;

    loop {
        // 每个任务开始前清理该 pdf 的旧索引：保证「索引 ⊆ 本次识别结果」，
        // 并避免崩溃恢复重跑产生重复文档（原 force 独占清理改为通用不变式）
        {
            match search_service.lock() {
                Ok(mut ss) => {
                    if let Err(e) = ss.delete_pdf(current_pdf_id as u64) {
                        warn!("清理旧索引失败: pdf_id={}, {}", current_pdf_id, e);
                    }
                }
                Err(e) => warn!("清理旧索引时获取搜索服务锁失败: {}", e),
            }
        }

        // 更新状态为 processing
        set_pdf_status(db, current_pdf_id, "processing", None)?;

        // 发送进度事件
        let _ = app_handle.emit("ocr-progress", OcrProgress {
            pdf_id: current_pdf_id,
            current: 0,
            total: page_count as u32,
            status: "processing".to_string(),
        });

        let mut success_count = 0;
        let mut error_count = 0;
        // 本 PDF 成功页缓冲：循环结束后一次 commit（替代每页 50MB writer）
        let mut pending_index: Vec<PageIndexEntry> = Vec::new();

        // 处理每一页
        for page_num in 1..=page_count {
            info!("处理PDF页面: pdf_id={}, page={}/{}", current_pdf_id, page_num, page_count);

            // 动态调整图像尺寸：如果内存紧张，降低渲染尺寸
            let current_dimension = if crate::services::memory_monitor::is_low_memory() {
                warn!("内存紧张，降低渲染尺寸: {} -> 500", max_image_dimension);
                500
            } else {
                max_image_dimension
            };

            match process_page(
                current_pdf_id,
                page_num,
                &storage_path,
                ocr_service,
                pdf_service,
                db,
                &filename,
                folder_id,
                current_dimension,
            ) {
                Ok(entry) => {
                    success_count += 1;
                    if let Some(e) = entry {
                        pending_index.push(e);
                    }
                    info!("页面处理成功: pdf_id={}, page={}", current_pdf_id, page_num);
                }
                Err(e) => {
                    error_count += 1;
                    error!("页面处理失败: pdf_id={}, page={}, 错误: {}", current_pdf_id, page_num, e);
                }
            }

            let mem_info = get_system_memory_info();
            info!("内存状态: 可用={:.1}GB, 已用={:.1}%",
                mem_info.available as f64 / 1024.0 / 1024.0 / 1024.0,
                mem_info.used_percent
            );

            // 发送进度
            let _ = app_handle.emit("ocr-progress", OcrProgress {
                pdf_id: current_pdf_id,
                current: page_num as u32,
                total: page_count as u32,
                status: "processing".to_string(),
            });
        }

        // 批量写入搜索索引（单次 commit）
        if !pending_index.is_empty() {
            let mut search_svc = search_service.lock().map_err(|e| {
                error!("获取搜索服务锁失败: {}", e);
                format!("搜索服务锁定失败: {}", e)
            })?;
            if let Err(e) = search_svc.index_pages(&pending_index) {
                warn!("批量索引失败: pdf_id={}, count={}, {}", current_pdf_id, pending_index.len(), e);
            } else {
                info!("批量索引成功: pdf_id={}, count={}", current_pdf_id, pending_index.len());
            }
        }

        // 最终状态：任一页失败 → error（schema 无 partial，禁止部分失败标 done）
        let final_status = if error_count == 0 { "done" } else { "error" };
        let error_message = if error_count > 0 {
            Some(format!(
                "{}个页面处理失败（成功 {}/{}）",
                error_count,
                success_count,
                page_count
            ))
        } else {
            None
        };

        set_pdf_status(db, current_pdf_id, final_status, error_message.as_deref())?;

        let _ = app_handle.emit("ocr-progress", OcrProgress {
            pdf_id: current_pdf_id,
            current: page_count as u32,
            total: page_count as u32,
            status: final_status.to_string(),
        });

        info!(
            "OCR处理完成: pdf_id={}, filename={}, 成功={}, 失败={}, status={}",
            current_pdf_id, filename, success_count, error_count, final_status
        );

        // 标记当前任务完成，并取出下一任务（后端真正继续执行）
        // 加载失败的任务标 error 后跳过，继续取更后面的任务
        let mut next_loaded = false;
        // 首轮需 complete 已处理完的 current；后续轮次 current 已是 get_next 设为 running 的失败任务
        let mut need_complete = true;
        while !next_loaded {
            let mut completed_id: Option<i64> = None;
            let next_task = {
                let mut queue = task_queue.lock().map_err(|e| {
                    error!("获取任务队列锁失败: {}", e);
                    format!("任务队列锁定失败: {}", e)
                })?;
                if need_complete {
                    queue.complete(current_pdf_id);
                    completed_id = Some(current_pdf_id);
                    need_complete = false;
                }
                queue.get_next()
            };

            // 持久化同步：完成的任务出队（锁外操作 DB，避免锁序交叉）
            if let Some(cid) = completed_id {
                let conn = db.lock().map_err(|e| {
                    error!("获取数据库锁失败: {}", e);
                    format!("数据库锁定失败: {}", e)
                })?;
                if let Err(e) = remove_queue_row(&conn, cid) {
                    warn!("移除持久化任务失败: pdf_id={}, {}", cid, e);
                }
            }

            let Some(next) = next_task else {
                // 仅在队列确实空闲时释放模型：若有新工作线程已接管（enqueue 已置 running），
                // 卸载会让新线程重新加载模型（recognize 会自愈，但白费数秒）
                let busy = task_queue.lock().map(|q| q.is_busy()).unwrap_or(false);
                if busy {
                    info!("队列已有新任务接管，跳过模型释放");
                } else {
                    info!("队列已空，释放 OCR 模型以节省内存");
                    let mut ocr_svc = ocr_service.lock().map_err(|e| {
                        error!("获取OCR服务锁失败: {}", e);
                        format!("OCR服务锁定失败: {}", e)
                    })?;
                    ocr_svc.unload_ocr();
                }
                return Ok(());
            };

            info!("调度队列中下一任务: pdf_id={}", next.pdf_id);
            let _ = app_handle.emit("ocr-queued", serde_json::json!({
                "pdf_id": next.pdf_id,
                "position": 0
            }));

            match load_pdf_job_info(db, next.pdf_id) {
                Ok((sp, pc, fname, fid)) => {
                    current_pdf_id = next.pdf_id;
                    storage_path = sp;
                    page_count = pc;
                    filename = fname;
                    folder_id = fid;
                    info!("下一任务信息: filename={}, pages={}", filename, page_count);
                    next_loaded = true;
                }
                Err(e) => {
                    error!("加载下一 OCR 任务失败: pdf_id={}, {}", next.pdf_id, e);
                    let _ = set_pdf_status(
                        db,
                        next.pdf_id,
                        "error",
                        Some(&format!("加载任务信息失败: {}", e)),
                    );
                    let _ = app_handle.emit("ocr-progress", OcrProgress {
                        pdf_id: next.pdf_id,
                        current: 0,
                        total: 0,
                        status: "error".to_string(),
                    });
                    // get_next 已将该任务设为 running；下一轮 complete 它后再取下一个
                    current_pdf_id = next.pdf_id;
                    need_complete = true;
                }
            }
        }
    }
}

fn process_page(
    pdf_id: i64,
    page_num: i32,
    storage_path: &str,
    ocr_service: &State<'_, Mutex<OcrService>>,
    pdf_service: &State<'_, Mutex<PdfService>>,
    db: &State<'_, Db>,
    filename: &str,
    folder_id: Option<i64>,
    max_image_dimension: u32,
) -> Result<Option<PageIndexEntry>, String> {
    debug!("渲染PDF页面: page={}, path={}, max_dimension={}", page_num, storage_path, max_image_dimension);

    // 渲染 PDF 页面为图像（使用动态尺寸）
    let image = {
        let pdf_svc = pdf_service.lock().map_err(|e| {
            error!("获取PDF服务锁失败: {}", e);
            format!("PDF服务锁定失败: {}", e)
        })?;
        info!("已获取PDF服务锁，开始渲染: page={}", page_num);
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

    // 空文本不再静默标 done：空白页合法，但大面积空白往往意味着预处理/尺寸问题（P0-7），
    // 必须在日志留下可排查的告警信号
    if text.trim().is_empty() {
        warn!(
            "OCR 结果为空白文本: pdf_id={}, page={}, max_dimension={}（若非空白页，请检查渲染尺寸与图像预处理）",
            pdf_id, page_num, max_image_dimension
        );
    }

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

    // 返回待批量索引条目（不在每页单独 commit）
    if page_id > 0 {
        Ok(Some(PageIndexEntry {
            page_id: page_id as u64,
            pdf_id: pdf_id as u64,
            folder_id,
            page_number: page_num as u32,
            filename: filename.to_string(),
            content: text,
        }))
    } else {
        warn!("page_id 获取失败，跳过索引: pdf_id={}, page={}", pdf_id, page_num);
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        conn
    }

    /// T4：入队顺序号单调递增、重复入队忽略、完成即删行
    #[test]
    fn test_queue_row_persistence() {
        let conn = mem_conn();

        insert_queue_row(&conn, 5).unwrap();
        insert_queue_row(&conn, 6).unwrap();
        insert_queue_row(&conn, 5).unwrap(); // 幂等：已存在不重排

        let rows: Vec<(i64, i64)> = conn
            .prepare("SELECT pdf_id, position FROM ocr_queue ORDER BY position")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        assert_eq!(rows, vec![(5, 0), (6, 1)]);

        remove_queue_row(&conn, 5).unwrap();
        insert_queue_row(&conn, 7).unwrap();
        let rows: Vec<(i64, i64)> = conn
            .prepare("SELECT pdf_id, position FROM ocr_queue ORDER BY position")
            .unwrap()
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap()
            .filter_map(|r| r.ok())
            .collect();
        // 5 已删，新任务 7 拿到 max+1 = 2，顺序语义保持
        assert_eq!(rows, vec![(6, 1), (7, 2)]);
    }

    /// T4：启动恢复——崩溃 processing 置回 pending、done/孤儿行清理、按序装载
    #[test]
    fn test_restore_persisted_queue() {
        let conn = mem_conn();
        conn.execute(
            "INSERT INTO pdfs (id, filename, storage_path, status) VALUES (1, 'a.pdf', '/x/a.pdf', 'pending')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO pdfs (id, filename, storage_path, status) VALUES (2, 'b.pdf', '/x/b.pdf', 'processing')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO pdfs (id, filename, storage_path, status) VALUES (3, 'c.pdf', '/x/c.pdf', 'done')",
            [],
        ).unwrap();
        conn.execute("INSERT INTO ocr_queue (pdf_id, position) VALUES (1, 0)", []).unwrap();
        conn.execute("INSERT INTO ocr_queue (pdf_id, position) VALUES (2, 1)", []).unwrap();
        conn.execute("INSERT INTO ocr_queue (pdf_id, position) VALUES (3, 2)", []).unwrap();
        conn.execute("INSERT INTO ocr_queue (pdf_id, position) VALUES (99, 3)", []).unwrap(); // 孤儿行

        let mut queue = TaskQueue::new();
        let ids = restore_persisted_queue(&conn, &mut queue);
        assert_eq!(ids, vec![1, 2], "done 与孤儿行不应恢复，顺序按 position");

        // processing 已被崩溃恢复为 pending
        let st: String = conn
            .query_row("SELECT status FROM pdfs WHERE id = 2", [], |r| r.get(0))
            .unwrap();
        assert_eq!(st, "pending");

        // 失效行已清理，仅剩 2 行
        let cnt: i64 = conn
            .query_row("SELECT COUNT(*) FROM ocr_queue", [], |r| r.get(0))
            .unwrap();
        assert_eq!(cnt, 2);

        // 装载顺序正确且 running 未被占用（等待 resume 取队首）
        assert_eq!(queue.len(), 2);
        assert!(!queue.is_busy());
        assert_eq!(queue.get_next().unwrap().pdf_id, 1);
        queue.complete(1);
        assert_eq!(queue.get_next().unwrap().pdf_id, 2);
    }

    /// 空队列恢复：不产生任何副作用
    #[test]
    fn test_restore_empty() {
        let conn = mem_conn();
        let mut queue = TaskQueue::new();
        assert!(restore_persisted_queue(&conn, &mut queue).is_empty());
        assert_eq!(queue.len(), 0);
    }
}