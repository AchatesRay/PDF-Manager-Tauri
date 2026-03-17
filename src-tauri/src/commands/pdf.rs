use crate::db::{get_pdfs_dir, Db};
use crate::models::{Pdf, PdfInfo, PdfStatus, PdfType};
use crate::services::pdf_service::PdfService;
use chrono::Utc;
use rusqlite::params;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::{Manager, State};
use uuid::Uuid;

/// 添加 PDF 文件
#[tauri::command]
pub fn add_pdf(
    path: String,
    folder_id: Option<i64>,
    db: State<'_, Db>,
    pdf_service: State<'_, Mutex<PdfService>>,
    app_handle: tauri::AppHandle,
) -> Result<PdfInfo, String> {
    let src_path = PathBuf::from(&path);
    if !src_path.exists() {
        return Err("File does not exist".to_string());
    }

    let filename = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.pdf".to_string());

    let storage_name = format!("{}.pdf", Uuid::new_v4());
    let storage_path = get_pdfs_dir(&app_handle).join(&storage_name);

    std::fs::copy(&src_path, &storage_path)
        .map_err(|e| format!("Failed to copy file: {}", e))?;

    let pdf_svc = pdf_service.lock().map_err(|e| e.to_string())?;
    let metadata = pdf_svc
        .get_metadata(&storage_path)
        .map_err(|e| e.to_string())?;
    let pdf_type = pdf_svc
        .detect_type(&storage_path)
        .map_err(|e| e.to_string())?;
    drop(pdf_svc);

    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO pdfs (folder_id, filename, original_path, storage_path, file_size, page_count, pdf_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            folder_id,
            filename,
            path,
            storage_path.to_string_lossy().to_string(),
            metadata.file_size,
            metadata.page_count,
            pdf_type.to_string(),
            "pending",
            now,
            now
        ],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    drop(conn);

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
    let conn = db.lock().map_err(|e| e.to_string())?;

    let sql = match folder_id {
        Some(_) => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE folder_id = ?1 ORDER BY created_at DESC",
        None => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs ORDER BY created_at DESC",
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    let pdfs = if let Some(fid) = folder_id {
        stmt.query_map(params![fid], |row| {
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
        })
    } else {
        stmt.query_map([], |row| {
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
        })
    }
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    Ok(pdfs)
}

/// 删除 PDF
#[tauri::command]
pub fn delete_pdf(
    pdf_id: i64,
    db: State<'_, Db>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let storage_path: String = conn
        .query_row(
            "SELECT storage_path FROM pdfs WHERE id = ?1",
            params![pdf_id],
            |row| row.get(0),
        )
        .map_err(|e| e.to_string())?;

    conn.execute("DELETE FROM pdfs WHERE id = ?1", params![pdf_id])
        .map_err(|e| e.to_string())?;

    drop(conn);

    let path = PathBuf::from(&storage_path);
    if path.exists() {
        std::fs::remove_file(&path).map_err(|e| e.to_string())?;
    }

    let thumbnails_dir = get_pdfs_dir(&app_handle).parent().unwrap().join("thumbnails").join(pdf_id.to_string());
    if thumbnails_dir.exists() {
        std::fs::remove_dir_all(&thumbnails_dir).ok();
    }

    Ok(())
}

/// 获取 PDF 详情
#[tauri::command]
pub fn get_pdf_detail(pdf_id: i64, db: State<'_, Db>) -> Result<Pdf, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

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
        .map_err(|e| e.to_string())?;

    Ok(pdf)
}