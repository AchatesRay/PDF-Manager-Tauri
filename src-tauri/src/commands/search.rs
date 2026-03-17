use crate::db::Db;
use crate::models::{PdfInfo, PdfStatus, PdfType};
use crate::services::search_service::{SearchResult, SearchService};
use rusqlite::params;
use std::sync::Mutex;
use tauri::State;
use tracing::info;

/// 搜索PDF内容
#[tauri::command]
pub fn search(
    query: String,
    folder_id: Option<i64>,
    search_service: State<'_, Mutex<SearchService>>,
) -> Result<Vec<SearchResult>, String> {
    info!("Searching content: query={}, folder_id={:?}", query, folder_id);
    let svc = search_service.lock().map_err(|e| e.to_string())?;
    svc.search(&query, folder_id, 50).map_err(|e| e.to_string())
}

/// 搜索PDF文件名
#[tauri::command]
pub fn search_filename(
    query: String,
    folder_id: Option<i64>,
    db: State<'_, Db>,
) -> Result<Vec<PdfInfo>, String> {
    info!("Searching filename: query={}, folder_id={:?}", query, folder_id);

    let conn = db.lock().map_err(|e| e.to_string())?;

    let search_pattern = format!("%{}%", query);

    let sql = match folder_id {
        Some(_) => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE filename LIKE ?1 AND folder_id = ?2 ORDER BY created_at DESC",
        None => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE filename LIKE ?1 ORDER BY created_at DESC",
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

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

    let pdfs = if let Some(fid) = folder_id {
        stmt.query_map(params![search_pattern, fid], row_to_pdf_info)
    } else {
        stmt.query_map(params![search_pattern], row_to_pdf_info)
    }
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    info!("Found {} PDFs matching filename", pdfs.len());
    Ok(pdfs)
}