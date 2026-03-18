use crate::db::Db;
use crate::models::{PdfInfo, PdfStatus, PdfType};
use crate::services::search_service::{SearchResult, SearchService};
use rusqlite::params;
use std::sync::Mutex;
use tauri::State;
use tracing::{debug, error, info, warn};

/// 搜索PDF内容
#[tauri::command]
pub fn search(
    query: String,
    folder_id: Option<i64>,
    search_service: State<'_, Mutex<SearchService>>,
) -> Result<Vec<SearchResult>, String> {
    info!("开始搜索PDF内容: query='{}', folder_id={:?}", query, folder_id);

    if query.trim().is_empty() {
        warn!("搜索查询为空");
        return Ok(Vec::new());
    }

    let svc = match search_service.lock() {
        Ok(s) => s,
        Err(e) => {
            error!("获取搜索服务锁失败: {}", e);
            return Err(format!("搜索服务锁定失败: {}", e));
        }
    };

    match svc.search(&query, folder_id, 50) {
        Ok(results) => {
            info!("搜索完成: 找到 {} 条结果", results.len());
            if !results.is_empty() {
                debug!("搜索结果示例: {:?}", &results[0..std::cmp::min(3, results.len())]);
            }
            Ok(results)
        }
        Err(e) => {
            error!("搜索失败: query='{}', 错误: {}", query, e);
            Err(format!("搜索失败: {}", e))
        }
    }
}

/// 搜索PDF文件名
#[tauri::command]
pub fn search_filename(
    query: String,
    folder_id: Option<i64>,
    db: State<'_, Db>,
) -> Result<Vec<PdfInfo>, String> {
    info!("开始搜索PDF文件名: query='{}', folder_id={:?}", query, folder_id);

    if query.trim().is_empty() {
        warn!("搜索查询为空");
        return Ok(Vec::new());
    }

    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    let search_pattern = format!("%{}%", query);
    debug!("搜索模式: {}", search_pattern);

    let sql = match folder_id {
        Some(_) => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE filename LIKE ?1 AND folder_id = ?2 ORDER BY created_at DESC",
        None => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE filename LIKE ?1 ORDER BY created_at DESC",
    };

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            error!("准备SQL语句失败: {}", e);
            return Err(format!("数据库查询准备失败: {}", e));
        }
    };

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
    .map_err(|e| {
        error!("执行查询失败: {}", e);
        format!("数据库查询执行失败: {}", e)
    })?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| {
        error!("解析查询结果失败: {}", e);
        format!("数据解析失败: {}", e)
    })?;

    info!("文件名搜索完成: 找到 {} 个PDF", pdfs.len());
    Ok(pdfs)
}