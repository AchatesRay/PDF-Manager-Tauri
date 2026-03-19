use crate::db::{get_pdfs_dir, Db, get_setting, SETTING_DATA_DIR};
use crate::models::{Pdf, PdfInfo, PdfStatus, PdfType};
use crate::services::pdf_service::PdfService;
use crate::services::search_service::SearchService;
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
    let existing: i64 = conn_check.query_row(
        "SELECT COUNT(*) FROM pdfs WHERE original_path = ?1",
        params![&path],
        |row| row.get(0),
    ).unwrap_or(0);
    drop(conn_check);

    if existing > 0 {
        warn!("PDF文件已存在: {}", path);
        return Err("该PDF文件已添加过".to_string());
    }

    // 获取存储路径：优先使用文件夹的 storage_path，否则使用用户配置的数据目录
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    let storage_dir = if let Some(folder_path) = get_folder_storage_path(&conn, folder_id) {
        debug!("使用文件夹存储路径: {:?}", folder_path);
        folder_path
    } else {
        // 获取用户配置的数据目录
        let data_dir = get_setting(&conn, SETTING_DATA_DIR)
            .map(|p| PathBuf::from(p).join("pdfs"))
            .unwrap_or_else(|| get_pdfs_dir(&app_handle));
        debug!("使用数据目录存储路径: {:?}", data_dir);
        data_dir
    };
    drop(conn);

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

    // 删除数据库记录
    match conn.execute("DELETE FROM pdfs WHERE id = ?1", params![pdf_id]) {
        Ok(rows) => debug!("删除了 {} 条PDF记录", rows),
        Err(e) => {
            error!("删除PDF记录失败 (id={}): {}", pdf_id, e);
            return Err(format!("数据库删除失败: {}", e));
        }
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