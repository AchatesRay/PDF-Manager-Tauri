use crate::db::{get_pdfs_dir, Db, get_setting, SETTING_DATA_DIR};
use crate::models::{Pdf, PdfInfo, PdfStatus, PdfType};
use crate::services::pdf_service::PdfService;
use crate::services::search_service::{PageIndexEntry, SearchService};
use chrono::Utc;
use rusqlite::params;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::State;
use tracing::{debug, error, info, warn};
use uuid::Uuid;
use base64::{engine::general_purpose::STANDARD, Engine};

fn row_to_pdf_info(row: &rusqlite::Row) -> rusqlite::Result<PdfInfo> {
    let status_str: String = row.get(5)?;
    let pdf_type_str: String = row.get(4)?;
    Ok(PdfInfo {
        id: row.get(0)?,
        folder_id: row.get(1)?,
        filename: row.get(2)?,
        page_count: row.get(3)?,
        pdf_type: pdf_type_str.parse().unwrap_or(PdfType::Scanned),
        status: status_str.parse().unwrap_or(PdfStatus::Pending),
        progress: None,
    })
}

/// 获取文件夹的完整存储路径（向上遍历构建路径）
fn get_folder_storage_path(conn: &rusqlite::Connection, folder_id: Option<i64>) -> Option<PathBuf> {
    let mut path_parts: Vec<String> = Vec::new();
    let mut current_id = folder_id;
    let mut root_storage_path: Option<String> = None;

    // 向上遍历获取所有父文件夹名称
    while let Some(id) = current_id {
        let result: Result<(Option<i64>, String, Option<String>), _> = conn
            .query_row(
                "SELECT parent_id, name, storage_path FROM folders WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            );

        if let Ok((parent_id, name, storage_path)) = result {
            path_parts.push(name);
            // 根文件夹有 storage_path
            if storage_path.is_some() && root_storage_path.is_none() {
                root_storage_path = storage_path;
            }
            current_id = parent_id;
        } else {
            break;
        }
    }

    // 反转得到从根到当前的路径
    path_parts.reverse();

    // 组合完整路径
    if let Some(base) = root_storage_path {
        let mut full_path = PathBuf::from(base);
        for part in path_parts {
            full_path.push(&part);
        }
        debug!("Calculated folder storage path: {:?}", full_path);
        Some(full_path)
    } else {
        None
    }
}

/// 文件名查重计数：同一文件夹（含根级 NULL）内 filename 相同的数量。
/// COALESCE 双侧归一到 -1，保证「根级 vs 根级」判定为同级。
fn count_filename_duplicates(conn: &rusqlite::Connection, filename: &str, folder_id: Option<i64>) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM pdfs WHERE filename = ?1 AND COALESCE(folder_id, -1) = COALESCE(?2, -1)",
        params![filename, folder_id],
        |row| row.get(0),
    ).unwrap_or(0)
}

/// 添加 PDF 文件
#[tauri::command]
pub fn add_pdf(
    path: String,
    folder_id: Option<i64>,
    db: State<'_, Db>,
    pdf_service: State<'_, Mutex<PdfService>>,
    app_handle: tauri::AppHandle,
) -> Result<PdfInfo, String> {
    info!("开始添加PDF文件: path={}, folder_id={:?}", path, folder_id);

    let src_path = PathBuf::from(&path);
    if !src_path.exists() {
        error!("PDF文件不存在: {}", path);
        return Err(format!("文件不存在: {}", path));
    }
    debug!("源文件存在: {:?}", src_path);

    // 检查文件扩展名
    let extension = src_path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if extension != "pdf" {
        warn!("文件扩展名不是PDF: {}", extension);
        return Err(format!("不支持的文件类型: {}", extension));
    }

    let filename = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.pdf".to_string());
    debug!("提取文件名: {}", filename);

    // 检查是否已存在相同文件
    let conn_check = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;
    let existing_path: i64 = conn_check.query_row(
        "SELECT COUNT(*) FROM pdfs WHERE original_path = ?1",
        params![&path],
        |row| row.get(0),
    ).unwrap_or(0);

    // 文件名查重：同一文件夹（含根级 NULL）内 filename 相同即算重名。
    // 范围口径（用户确认）：不同文件夹允许同名，仅目标文件夹内禁止。
    let existing_name = count_filename_duplicates(&conn_check, &filename, folder_id);
    drop(conn_check);

    if existing_path > 0 {
        warn!("PDF文件已存在: {}", path);
        return Err("该PDF文件已添加过".to_string());
    }

    if existing_name > 0 {
        warn!("文件名已存在（同文件夹）: {} (folder_id={:?})", filename, folder_id);
        return Err(format!("文件名已存在: {}", filename));
    }

    // 获取存储路径：优先使用文件夹的 storage_path，否则使用用户配置的数据目录
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    // 持锁期间只读取数据库值；get_pdfs_dir() 内部会再次 lock(Db)（非可重入 std::sync::Mutex），
    // 若在持锁时求值会造成同线程二次加锁自死锁，导致后续所有 IPC 永久挂起（P0-6）
    let folder_path = get_folder_storage_path(&conn, folder_id);
    let data_dir_setting = get_setting(&conn, SETTING_DATA_DIR).map(|p| PathBuf::from(p).join("pdfs"));
    drop(conn);

    let storage_dir = if let Some(folder_path) = folder_path {
        debug!("使用文件夹存储路径: {:?}", folder_path);
        folder_path
    } else if let Some(data_dir) = data_dir_setting {
        debug!("使用数据目录存储路径: {:?}", data_dir);
        data_dir
    } else {
        // 锁已释放，此时回调 app_handle 取默认数据目录是安全的
        let data_dir = get_pdfs_dir(&app_handle);
        debug!("使用数据目录存储路径: {:?}", data_dir);
        data_dir
    };

    // 确保存储目录存在
    if !storage_dir.exists() {
        match std::fs::create_dir_all(&storage_dir) {
            Ok(_) => info!("创建存储目录: {:?}", storage_dir),
            Err(e) => {
                error!("创建存储目录失败: {:?}", e);
                return Err(format!("无法创建存储目录: {}", e));
            }
        }
    }

    // 使用原始文件名，如果存在同名文件则添加后缀
    let mut storage_path = storage_dir.join(&filename);
    if storage_path.exists() {
        // 同名文件已存在，添加 UUID 后缀
        let stem = src_path.file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let new_filename = format!("{}_{}.pdf", stem, Uuid::new_v4().simple());
        storage_path = storage_dir.join(&new_filename);
        warn!("同名文件已存在，使用新文件名: {:?}", storage_path);
    }
    debug!("存储路径: {:?}", storage_path);

    // 复制文件
    let file_size = match std::fs::metadata(&src_path) {
        Ok(meta) => meta.len() as i64,
        Err(e) => {
            warn!("无法获取文件大小: {}", e);
            0
        }
    };
    debug!("文件大小: {} bytes", file_size);

    match std::fs::copy(&src_path, &storage_path) {
        Ok(_) => info!("文件复制成功: {:?}", storage_path),
        Err(e) => {
            error!("复制文件失败: {} -> {:?}, 错误: {}", path, storage_path, e);
            return Err(format!("复制文件失败: {}", e));
        }
    }

    // 获取PDF服务锁
    let pdf_svc = match pdf_service.lock() {
        Ok(svc) => svc,
        Err(e) => {
            error!("获取PDF服务锁失败: {}", e);
            // 尝试删除已复制的文件
            let _ = std::fs::remove_file(&storage_path);
            return Err(format!("PDF服务锁定失败: {}", e));
        }
    };

    // 获取PDF元数据
    let metadata = match pdf_svc.get_metadata(&storage_path) {
        Ok(meta) => {
            debug!("PDF元数据: pages={}, size={}", meta.page_count, meta.file_size);
            meta
        }
        Err(e) => {
            error!("获取PDF元数据失败: {:?}, 错误: {}", storage_path, e);
            drop(pdf_svc);
            let _ = std::fs::remove_file(&storage_path);
            return Err(format!("无法读取PDF元数据: {}", e));
        }
    };

    // 检测PDF类型
    let pdf_type = match pdf_svc.detect_type(&storage_path) {
        Ok(t) => {
            debug!("PDF类型: {:?}", t);
            t
        }
        Err(e) => {
            warn!("检测PDF类型失败: {}, 默认为扫描型", e);
            PdfType::Scanned
        }
    };
    drop(pdf_svc);

    // 保存到数据库
    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            let _ = std::fs::remove_file(&storage_path);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };
    let now = Utc::now().to_rfc3339();

    match conn.execute(
        "INSERT INTO pdfs (folder_id, filename, original_path, storage_path, file_size, page_count, pdf_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            folder_id,
            filename,
            path,
            storage_path.to_string_lossy().to_string(),
            file_size,
            metadata.page_count,
            pdf_type.to_string(),
            "pending",
            now,
            now
        ],
    ) {
        Ok(_) => debug!("PDF记录插入成功"),
        Err(e) => {
            error!("插入PDF记录失败: {}", e);
            drop(conn);
            let _ = std::fs::remove_file(&storage_path);
            return Err(format!("数据库插入失败: {}", e));
        }
    }

    let id = conn.last_insert_rowid();
    drop(conn);

    info!("PDF添加成功: id={}, filename={}, pages={}, type={:?}", id, filename, metadata.page_count, pdf_type);

    Ok(PdfInfo {
        id,
        folder_id,
        filename,
        page_count: metadata.page_count,
        pdf_type,
        status: PdfStatus::Pending,
        progress: Some(0.0),
    })
}

/// 获取 PDF 列表
#[tauri::command]
pub fn get_pdf_list(
    folder_id: Option<i64>,
    db: State<'_, Db>,
) -> Result<Vec<PdfInfo>, String> {
    info!("开始获取PDF列表, folder_id={:?}", folder_id);

    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    let sql = match folder_id {
        Some(_) => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE folder_id = ?1 ORDER BY created_at DESC",
        None => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs ORDER BY created_at DESC",
    };

    let mut stmt = conn.prepare(sql).map_err(|e| {
        error!("准备SQL语句失败: {}", e);
        format!("数据库查询准备失败: {}", e)
    })?;

    let pdfs = if let Some(fid) = folder_id {
        stmt.query_map(params![fid], row_to_pdf_info)
    } else {
        stmt.query_map([], row_to_pdf_info)
    }
    .map_err(|e| {
        error!("执行查询失败: {}", e);
        format!("数据库查询执行失败: {}", e)
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| {
        error!("解析查询结果失败: {}", e);
        format!("数据解析失败: {}", e)
    })?;

    info!("成功获取 {} 个PDF文件", pdfs.len());
    Ok(pdfs)
}

/// 删除 PDF
#[tauri::command]
pub fn delete_pdf(
    pdf_id: i64,
    db: State<'_, Db>,
    search_service: State<'_, Mutex<SearchService>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("开始删除PDF: pdf_id={}", pdf_id);

    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    // 获取存储路径
    let storage_path: String = conn
        .query_row(
            "SELECT storage_path FROM pdfs WHERE id = ?1",
            params![pdf_id],
            |row| row.get(0),
        )
        .map_err(|e| {
            error!("PDF不存在 (id={}): {}", pdf_id, e);
            format!("PDF不存在: {}", e)
        })?;

    debug!("PDF存储路径: {}", storage_path);

    // 获取文件名用于日志
    let filename: String = conn
        .query_row("SELECT filename FROM pdfs WHERE id = ?1", params![pdf_id], |row| row.get(0))
        .unwrap_or_else(|_| "unknown".to_string());

    // 删除数据库记录（外键级联未开启，须显式清页面，避免孤儿行）
    match conn.execute("DELETE FROM pdfs WHERE id = ?1", params![pdf_id]) {
        Ok(rows) => debug!("删除了 {} 条PDF记录", rows),
        Err(e) => {
            error!("删除PDF记录失败 (id={}): {}", pdf_id, e);
            return Err(format!("数据库删除失败: {}", e));
        }
    }
    if let Err(e) = conn.execute("DELETE FROM pdf_pages WHERE pdf_id = ?1", params![pdf_id]) {
        warn!("清理页面记录失败 (pdf_id={}): {}", pdf_id, e);
    }

    // 同步清理持久化 OCR 队列（避免重启恢复出已删除的 PDF）
    if let Err(e) = conn.execute("DELETE FROM ocr_queue WHERE pdf_id = ?1", params![pdf_id]) {
        warn!("清理持久化队列行失败 (pdf_id={}): {}", pdf_id, e);
    }

    drop(conn);

    // 删除搜索索引
    {
        let mut ss = search_service.lock().map_err(|e| {
            error!("获取搜索服务锁失败: {}", e);
            format!("搜索服务锁定失败: {}", e)
        })?;
        if let Err(e) = ss.delete_pdf(pdf_id as u64) {
            warn!("删除搜索索引失败 (pdf_id={}): {}", pdf_id, e);
        } else {
            debug!("搜索索引删除成功: pdf_id={}", pdf_id);
        }
    }

    // 删除存储文件
    let path = PathBuf::from(&storage_path);
    if path.exists() {
        match std::fs::remove_file(&path) {
            Ok(_) => debug!("删除PDF文件: {:?}", path),
            Err(e) => warn!("删除PDF文件失败: {:?}, 错误: {}", path, e),
        }
    } else {
        warn!("PDF文件不存在: {:?}", path);
    }

    // 删除缩略图目录
    let thumbnails_dir = get_pdfs_dir(&app_handle).parent().unwrap().join("thumbnails").join(pdf_id.to_string());
    if thumbnails_dir.exists() {
        match std::fs::remove_dir_all(&thumbnails_dir) {
            Ok(_) => debug!("删除缩略图目录: {:?}", thumbnails_dir),
            Err(e) => warn!("删除缩略图目录失败: {:?}, 错误: {}", thumbnails_dir, e),
        }
    }

    info!("PDF删除成功: id={}, filename={}", pdf_id, filename);
    Ok(())
}

/// 批量移动结果
#[derive(Debug, Clone, serde::Serialize)]
pub struct MoveReport {
    /// 成功移动的 pdf_id
    pub moved: Vec<i64>,
    /// 被跳过的 (pdf_id, 原因)
    pub skipped: Vec<(i64, String)>,
}

/// 批量移动 PDF 到目标文件夹
///
/// 移动 = 更新 folder_id + 物理迁移存储文件 + **重建该 PDF 的搜索索引条目**
/// （索引含 folder_id 字段，不重建会导致按文件夹搜索结果错误）。
///
/// 锁纪律（防 P0-6 自死锁）：持 Db 锁期间只做 SQL 读，读完立即 drop，
/// 文件操作与索引重建都在锁外进行；`get_folder_storage_path` 只读传入的 conn。
#[tauri::command]
pub fn move_pdfs(
    pdf_ids: Vec<i64>,
    target_folder_id: Option<i64>,
    db: State<'_, Db>,
    search_service: State<'_, Mutex<SearchService>>,
) -> Result<MoveReport, String> {
    info!("批量移动PDF: ids={:?}, target_folder={:?}", pdf_ids, target_folder_id);

    if pdf_ids.is_empty() {
        return Ok(MoveReport { moved: Vec::new(), skipped: Vec::new() });
    }

    // ① 一次性读出目标文件夹路径与所有 pdf 行，读完立即放锁
    let mut skipped_busy: Vec<(i64, String)> = Vec::new();
    let (target_dir, pdf_rows, skipped_busy) = {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        // 目标文件夹不存在 → 直接报错（整个操作无副作用）
        if let Some(fid) = target_folder_id {
            let exists: i64 = conn
                .query_row("SELECT COUNT(*) FROM folders WHERE id = ?1", params![fid], |r| r.get(0))
                .unwrap_or(0);
            if exists == 0 {
                return Err(format!("目标文件夹不存在: {}", fid));
            }
        }

        let target_dir = match get_folder_storage_path(&conn, target_folder_id) {
            Some(p) => p,
            None => {
                // 根级（folder_id=None）或根文件夹缺 storage_path → 回退用户配置数据目录（与 add_pdf 一致）
                match get_setting(&conn, SETTING_DATA_DIR) {
                    Some(p) => PathBuf::from(p).join("pdfs"),
                    None => return Err("无法计算目标文件夹存储路径（根文件夹缺少 storage_path）".to_string()),
                }
            }
        };

        let mut rows = Vec::new();
        for pid in &pdf_ids {
            let row: Option<(i64, Option<i64>, String, String, bool)> = conn
                .query_row(
                    "SELECT p.id, p.folder_id, p.filename, p.storage_path,
                            EXISTS(SELECT 1 FROM ocr_queue q WHERE q.pdf_id = p.id) OR p.status = 'processing'
                     FROM pdfs p WHERE p.id = ?1",
                    params![pid],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get::<_, i64>(4)? != 0)),
                )
                .ok();
            match row {
                Some((id, folder_id, filename, storage_path, busy)) => {
                    if busy {
                        // 正在 OCR 或排队中：移动会让正在执行的任务读不到文件
                        warn!("移动跳过：PDF 正在 OCR 或排队中 (pdf_id={})", id);
                        skipped_busy.push((id, "OCR 处理中/排队中，请稍后再试".to_string()));
                    } else {
                        rows.push((id, folder_id, filename, storage_path));
                    }
                }
                None => warn!("PDF不存在，跳过移动: pdf_id={}", pid),
            }
        }
        (target_dir, rows, skipped_busy)
    }; // ← conn 锁在此释放

    let mut report = MoveReport { moved: Vec::new(), skipped: skipped_busy };

    if !target_dir.exists() {
        if let Err(e) = std::fs::create_dir_all(&target_dir) {
            return Err(format!("无法创建目标目录 {:?}: {}", target_dir, e));
        }
    }

    // ② 锁外做文件迁移 + DB 更新（每个 pdf 一个短事务式小锁）
    for (pdf_id, old_folder_id, filename, storage_path) in pdf_rows {
        // 已在目标文件夹 → 无需移动
        if old_folder_id == target_folder_id {
            report.moved.push(pdf_id);
            continue;
        }

        let src = PathBuf::from(&storage_path);
        let dst = target_dir.join(&filename);

        // 目标同名冲突 → 跳过（与 add_pdf 重名口径一致）
        if dst.exists() && src != dst {
            warn!("移动跳过：目标已存在同名文件 {:?} (pdf_id={})", filename, pdf_id);
            report.skipped.push((pdf_id, format!("文件名已存在: {}", filename)));
            continue;
        }

        if src.exists() && src != dst {
            // 跨盘 rename 会失败，回退 copy + delete
            if let Err(e) = std::fs::rename(&src, &dst) {
                debug!("rename 失败，回退 copy+delete: {} -> {:?}, {}", storage_path, dst, e);
                match std::fs::copy(&src, &dst) {
                    Ok(_) => {
                        if let Err(e2) = std::fs::remove_file(&src) {
                            // 复制成功但原文件删不掉：目标已就位，保留目标，仅告警
                            warn!("删除源文件失败（目标已复制成功）: {:?}, {}", src, e2);
                        }
                    }
                    Err(e2) => {
                        warn!("移动跳过：复制失败 {:?}, {}", dst, e2);
                        report.skipped.push((pdf_id, format!("移动文件失败: {}", e2)));
                        continue;
                    }
                }
            }
        }

        // 更新 DB（失败则回滚文件位置，避免文件已迁移而记录未更新）
        {
            let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
            if let Err(e) = conn.execute(
                "UPDATE pdfs SET folder_id = ?1, storage_path = ?2, updated_at = datetime('now') WHERE id = ?3",
                params![target_folder_id, dst.to_string_lossy().to_string(), pdf_id],
            ) {
                warn!("更新PDF记录失败 (pdf_id={}): {}", pdf_id, e);
                drop(conn);
                // 回滚：把文件移回原位
                if dst.exists() && !src.exists() {
                    if let Err(re) = std::fs::rename(&dst, &src) {
                        let _ = std::fs::copy(&dst, &src);
                        let _ = std::fs::remove_file(&dst);
                        warn!("回滚移动失败: {:?} -> {:?}, {}", dst, src, re);
                    }
                }
                report.skipped.push((pdf_id, format!("更新记录失败: {}", e)));
                continue;
            }
        }

        // 重建该 PDF 的搜索索引（folder_id 变了；delete + 从 pdf_pages 回填）
        {
            let mut ss = search_service.lock().map_err(|e| format!("搜索服务锁定失败: {}", e))?;
            if let Err(e) = ss.delete_pdf(pdf_id as u64) {
                warn!("移动后清理旧索引失败 (pdf_id={}): {}", pdf_id, e);
            }
        }
        {
            let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
            let entries: Vec<PageIndexEntry> = match conn.prepare(
                "SELECT pp.id, pp.pdf_id, pdfs.folder_id, pp.page_number, pdfs.filename, pp.ocr_text
                 FROM pdf_pages pp JOIN pdfs ON pdfs.id = pp.pdf_id
                 WHERE pp.pdf_id = ?1 AND pp.ocr_text IS NOT NULL AND TRIM(pp.ocr_text) <> ''",
            ) {
                Ok(mut stmt) => stmt
                    .query_map(params![pdf_id], |row| {
                        Ok(PageIndexEntry {
                            page_id: row.get::<_, i64>(0)? as u64,
                            pdf_id: row.get::<_, i64>(1)? as u64,
                            folder_id: row.get::<_, Option<i64>>(2)?,
                            page_number: row.get::<_, i64>(3)? as u32,
                            filename: row.get::<_, String>(4)?,
                            content: row.get::<_, String>(5)?,
                        })
                    })
                    .map(|it| it.filter_map(|r| r.ok()).collect())
                    .unwrap_or_default(),
                Err(e) => {
                    warn!("准备索引回填查询失败 (pdf_id={}): {}", pdf_id, e);
                    Vec::new()
                }
            }; // stmt 在此释放

            // 先放 Db 锁，再取搜索锁（与 delete_pdf 同口径，防锁序交叉）
            drop(conn);

            if !entries.is_empty() {
                let mut ss = search_service.lock().map_err(|e| format!("搜索服务锁定失败: {}", e))?;
                if let Err(e) = ss.index_pages(&entries) {
                    warn!("移动后回填索引失败 (pdf_id={}): {}", pdf_id, e);
                }
            }
        }

        report.moved.push(pdf_id);
        info!("PDF移动成功: pdf_id={}, -> {:?}", pdf_id, target_dir);
    }

    info!("批量移动完成: moved={}, skipped={}", report.moved.len(), report.skipped.len());
    Ok(report)
}

/// 获取 PDF 详情
#[tauri::command]
pub fn get_pdf_detail(pdf_id: i64, db: State<'_, Db>) -> Result<Pdf, String> {
    debug!("开始获取PDF详情: pdf_id={}", pdf_id);

    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    let pdf = conn
        .query_row(
            "SELECT id, folder_id, filename, original_path, storage_path, file_size, page_count, pdf_type, status, error_message, created_at, updated_at FROM pdfs WHERE id = ?1",
            params![pdf_id],
            |row| {
                let status_str: String = row.get(8)?;
                let pdf_type_str: String = row.get(7)?;
                Ok(Pdf {
                    id: row.get(0)?,
                    folder_id: row.get(1)?,
                    filename: row.get(2)?,
                    original_path: row.get(3)?,
                    storage_path: row.get(4)?,
                    file_size: row.get(5)?,
                    page_count: row.get(6)?,
                    pdf_type: pdf_type_str.parse().unwrap_or(PdfType::Scanned),
                    status: status_str.parse().unwrap_or(PdfStatus::Pending),
                    error_message: row.get(9)?,
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            },
        )
        .map_err(|e| {
            error!("PDF不存在 (id={}): {}", pdf_id, e);
            format!("PDF不存在: {}", e)
        })?;

    debug!("PDF详情获取成功: filename={}, pages={}, status={:?}", pdf.filename, pdf.page_count, pdf.status);
    Ok(pdf)
}

/// 渲染 PDF 页面为图像（返回 base64 编码）
#[tauri::command]
pub fn render_pdf_page(
    pdf_id: i64,
    page_num: u32,
    db: State<'_, Db>,
    pdf_service: State<'_, Mutex<PdfService>>,
) -> Result<String, String> {
    debug!("渲染PDF页面: pdf_id={}, page={}", pdf_id, page_num);

    // 获取 PDF 存储路径
    let storage_path: String = {
        let conn = db.lock().map_err(|e| {
            error!("获取数据库锁失败: {}", e);
            format!("数据库锁定失败: {}", e)
        })?;

        conn.query_row(
            "SELECT storage_path FROM pdfs WHERE id = ?1",
            params![pdf_id],
            |row| row.get(0),
        )
        .map_err(|e| {
            error!("PDF不存在 (id={}): {}", pdf_id, e);
            format!("PDF不存在: {}", e)
        })?
    };

    // 获取 PDF 服务锁
    let pdf_svc = pdf_service.lock().map_err(|e| {
        error!("获取PDF服务锁失败: {}", e);
        format!("PDF服务锁定失败: {}", e)
    })?;

    // 渲染页面
    let image = pdf_svc
        .render_page(std::path::Path::new(&storage_path), page_num)
        .map_err(|e| {
            error!("渲染PDF页面失败: pdf_id={}, page={}, 错误: {}", pdf_id, page_num, e);
            format!("渲染页面失败: {}", e)
        })?;

    drop(pdf_svc);

    // 转换为 PNG 格式并编码为 base64
    let mut buffer = Vec::new();
    image
        .write_to(&mut std::io::Cursor::new(&mut buffer), image::ImageFormat::Png)
        .map_err(|e| {
            error!("编码图像失败: {}", e);
            format!("编码图像失败: {}", e)
        })?;

    let base64_str = STANDARD.encode(&buffer);
    info!("PDF页面渲染成功: pdf_id={}, page={}, size={} bytes", pdf_id, page_num, base64_str.len());

    Ok(format!("data:image/png;base64,{}", base64_str))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        conn
    }

    fn insert_pdf(conn: &rusqlite::Connection, id: i64, folder_id: Option<i64>, filename: &str) {
        conn.execute(
            "INSERT INTO pdfs (id, folder_id, filename, storage_path, status) VALUES (?1, ?2, ?3, ?4, 'pending')",
            params![id, folder_id, filename, format!("/x/{}", filename)],
        ).unwrap();
    }

    /// 重名判定口径：同文件夹同名 → 命中；不同文件夹同名 → 不命中；根级(NULL)互为同级
    #[test]
    fn test_count_filename_duplicates_scope() {
        let conn = mem_conn();
        conn.execute("INSERT INTO folders (id, name) VALUES (1, 'A')", []).unwrap();
        conn.execute("INSERT INTO folders (id, name) VALUES (2, 'B')", []).unwrap();

        insert_pdf(&conn, 1, Some(1), "合同.pdf");
        insert_pdf(&conn, 2, Some(1), "合同.pdf"); // 同文件夹同名
        insert_pdf(&conn, 3, Some(2), "合同.pdf"); // 不同文件夹同名
        insert_pdf(&conn, 4, None, "报告.pdf");    // 根级
        insert_pdf(&conn, 5, None, "报告.pdf");    // 根级同名

        // 文件夹 1 内应命中 2 条（含自身，语义为「已存在 N 条同名」）
        assert_eq!(count_filename_duplicates(&conn, "合同.pdf", Some(1)), 2);
        // 文件夹 2 内只有 1 条
        assert_eq!(count_filename_duplicates(&conn, "合同.pdf", Some(2)), 1);
        // 文件夹 1 内不存在「报告.pdf」（根级不同级）
        assert_eq!(count_filename_duplicates(&conn, "报告.pdf", Some(1)), 0);
        // 根级互相判定为同级
        assert_eq!(count_filename_duplicates(&conn, "报告.pdf", None), 2);
        // 不存在的文件名
        assert_eq!(count_filename_duplicates(&conn, "不存在.pdf", None), 0);
    }
}