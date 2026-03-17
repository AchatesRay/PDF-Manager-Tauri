use crate::db::Db;
use crate::services::ocr_service::OcrService;
use crate::services::pdf_service::PdfService;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Manager, State};
use tracing::{error, info};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OcrStatus {
    pub available: bool,
    pub languages: Vec<String>,
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
    let svc = ocr_service.lock().map_err(|e| e.to_string())?;

    Ok(OcrStatus {
        available: svc.is_available(),
        languages: svc.available_languages(),
    })
}

/// 开始 OCR 处理
#[tauri::command]
pub async fn start_ocr(
    pdf_id: i64,
    db: State<'_, Db>,
    ocr_service: State<'_, Mutex<OcrService>>,
    pdf_service: State<'_, Mutex<PdfService>>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Starting OCR for PDF: {}", pdf_id);

    // 获取 PDF 信息
    let (storage_path, page_count): (String, i32) = {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.query_row(
            "SELECT storage_path, page_count FROM pdfs WHERE id = ?1",
            rusqlite::params![pdf_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?
    };

    // 更新状态为 processing
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE pdfs SET status = 'processing', updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![pdf_id],
        )
        .map_err(|e| e.to_string())?;
    }

    // 发送进度事件
    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: 0,
        total: page_count as u32,
        status: "processing".to_string(),
    });

    let ocr_svc = ocr_service.lock().map_err(|e| e.to_string())?;
    let pdf_svc = pdf_service.lock().map_err(|e| e.to_string())?;

    // 处理每一页
    for page_num in 1..=page_count {
        match process_page(&pdf_id, page_num, &storage_path, &ocr_svc, &pdf_svc, &db) {
            Ok(_) => {
                info!("Page {} processed successfully", page_num);
            }
            Err(e) => {
                error!("Failed to process page {}: {}", page_num, e);
            }
        }

        // 发送进度
        let _ = app_handle.emit("ocr-progress", OcrProgress {
            pdf_id,
            current: page_num as u32,
            total: page_count as u32,
            status: "processing".to_string(),
        });
    }

    drop(pdf_svc);
    drop(ocr_svc);

    // 更新状态为 done
    {
        let conn = db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE pdfs SET status = 'done', updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![pdf_id],
        )
        .map_err(|e| e.to_string())?;
    }

    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: page_count as u32,
        total: page_count as u32,
        status: "done".to_string(),
    });

    info!("OCR completed for PDF: {}", pdf_id);
    Ok(())
}

fn process_page(
    pdf_id: &i64,
    page_num: i32,
    storage_path: &str,
    ocr_service: &OcrService,
    pdf_service: &PdfService,
    db: &State<'_, Db>,
) -> Result<(), String> {
    // 渲染 PDF 页面为图像
    let image = pdf_service
        .render_page(std::path::Path::new(storage_path), page_num as u32)
        .map_err(|e| e.to_string())?;

    // OCR 识别
    let text = ocr_service
        .recognize(&image)
        .map_err(|e| e.to_string())?;

    // 保存到数据库
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status)
         VALUES (?1, ?2, ?3, 'done')",
        rusqlite::params![pdf_id, page_num, text],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}