use crate::db::Db;
use crate::models::{PdfInfo, PdfStatus, PdfType};
use crate::services::search_service::{PageIndexEntry, SearchResult, SearchService};
use rusqlite::params;
use std::sync::Mutex;
use tauri::State;
use tracing::{debug, error, info, warn};

/// 索引为空时从数据库回填（T1 索引版本升级的安全网）。
///
/// 索引版本变更（如 v5→v6）会备份旧索引并重建空索引；若不回填，
/// 老用户升级后搜索会静默变空，只能靠 force 重识别恢复。
/// 门控：索引非空直接返回 0（正常运行期零开销）。
pub fn refill_index_from_db(
    conn: &rusqlite::Connection,
    search_service: &mut SearchService,
) -> Result<usize, String> {
    if search_service.doc_count() > 0 {
        return Ok(0);
    }

    let mut stmt = conn
        .prepare(
            "SELECT pp.id, pp.pdf_id, pdfs.folder_id, pp.page_number, pdfs.filename, pp.ocr_text
             FROM pdf_pages pp
             JOIN pdfs ON pdfs.id = pp.pdf_id
             WHERE pp.ocr_text IS NOT NULL AND TRIM(pp.ocr_text) <> ''",
        )
        .map_err(|e| format!("准备回填查询失败: {}", e))?;

    let entries: Vec<PageIndexEntry> = stmt
        .query_map([], |row| {
            Ok(PageIndexEntry {
                page_id: row.get::<_, i64>(0)? as u64,
                pdf_id: row.get::<_, i64>(1)? as u64,
                folder_id: row.get::<_, Option<i64>>(2)?,
                page_number: row.get::<_, i64>(3)? as u32,
                filename: row.get::<_, String>(4)?,
                content: row.get::<_, String>(5)?,
            })
        })
        .map_err(|e| format!("执行回填查询失败: {}", e))?
        .filter_map(|r| r.ok())
        .collect();

    if entries.is_empty() {
        return Ok(0);
    }

    let count = entries.len();
    search_service
        .index_pages(&entries)
        .map_err(|e| format!("回填索引失败: {}", e))?;
    info!("索引已从数据库回填: {} 页", count);
    Ok(count)
}

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

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_conn() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        conn
    }

    /// 索引版本升级重建后必须能从数据库回填（否则老用户升级后搜索静默变空）
    #[test]
    fn test_refill_index_from_db() {
        let conn = mem_conn();
        conn.execute("INSERT INTO folders (id, name) VALUES (7, '回填测试组')", []).unwrap();
        conn.execute(
            "INSERT INTO pdfs (id, folder_id, filename, storage_path, status) VALUES (100, 7, 'doc.pdf', '/x/doc.pdf', 'done')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status) VALUES (100, 1, '配置单北斗七星内容', 'done')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status) VALUES (100, 2, '   ', 'done')",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status) VALUES (100, 3, NULL, 'pending')",
            [],
        ).unwrap();

        let dir = tempfile::tempdir().unwrap();
        let mut ss = SearchService::open(&dir.path().join("idx")).unwrap();

        // 空索引 → 回填（空文本 / NULL 页不计）
        let n = refill_index_from_db(&conn, &mut ss).unwrap();
        assert_eq!(n, 1, "仅非空文本页应回填");
        let hits = ss.search("北斗七星", Some(7), 10).unwrap();
        assert!(!hits.is_empty(), "回填后应可搜索");
        let wrong_folder = ss.search("北斗七星", Some(8), 10).unwrap();
        assert!(wrong_folder.is_empty(), "folder 下推在回填后仍生效");

        // 非空索引 → 门控直接返回 0（不重复回填）
        let n2 = refill_index_from_db(&conn, &mut ss).unwrap();
        assert_eq!(n2, 0);
    }
}