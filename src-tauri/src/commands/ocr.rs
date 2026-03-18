use crate::db::Db;
use crate::services::ocr_service::OcrService;
use crate::services::pdf_service::PdfService;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{Emitter, State};
use tracing::{debug, error, info, warn};

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
    debug!("开始获取OCR状态");

    let svc = match ocr_service.lock() {
        Ok(s) => s,
        Err(e) => {
            error!("获取OCR服务锁失败: {}", e);
            return Err(format!("OCR服务锁定失败: {}", e));
        }
    };

    let available = svc.is_available();
    let languages = svc.available_languages();

    debug!("OCR状态: available={}, languages={:?}", available, languages);

    Ok(OcrStatus {
        available,
        languages,
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
    info!("开始OCR处理: pdf_id={}", pdf_id);

    // 获取 PDF 信息
    let (storage_path, page_count, filename): (String, i32, String) = {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(e) => {
                error!("获取数据库锁失败: {}", e);
                return Err(format!("数据库锁定失败: {}", e));
            }
        };

        match conn.query_row(
            "SELECT storage_path, page_count, filename FROM pdfs WHERE id = ?1",
            rusqlite::params![pdf_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        ) {
            Ok(info) => info,
            Err(e) => {
                error!("PDF不存在 (pdf_id={}): {}", pdf_id, e);
                return Err(format!("PDF不存在: {}", e));
            }
        }
    };

    info!("PDF信息: filename={}, pages={}, storage={}", filename, page_count, storage_path);

    // 检查OCR服务是否可用
    {
        let ocr_svc = match ocr_service.lock() {
            Ok(s) => s,
            Err(e) => {
                error!("获取OCR服务锁失败: {}", e);
                return Err(format!("OCR服务锁定失败: {}", e));
            }
        };

        if !ocr_svc.is_available() {
            error!("OCR服务不可用");
            return Err("OCR服务不可用，请检查Tesseract是否正确安装".to_string());
        }
        drop(ocr_svc);
    }

    // 更新状态为 processing
    {
        let conn = match db.lock() {
            Ok(c) => c,
            Err(e) => {
                error!("获取数据库锁失败: {}", e);
                return Err(format!("数据库锁定失败: {}", e));
            }
        };

        match conn.execute(
            "UPDATE pdfs SET status = 'processing', updated_at = datetime('now') WHERE id = ?1",
            rusqlite::params![pdf_id],
        ) {
            Ok(_) => debug!("PDF状态更新为processing: pdf_id={}", pdf_id),
            Err(e) => {
                error!("更新PDF状态失败: {}", e);
                return Err(format!("更新状态失败: {}", e));
            }
        }
    }

    // 发送进度事件
    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: 0,
        total: page_count as u32,
        status: "processing".to_string(),
    });

    let ocr_svc = match ocr_service.lock() {
        Ok(s) => s,
        Err(e) => {
            error!("获取OCR服务锁失败: {}", e);
            // 恢复状态
            let _ = update_pdf_status(&db, pdf_id, "error", Some(&format!("OCR服务锁定失败: {}", e)));
            return Err(format!("OCR服务锁定失败: {}", e));
        }
    };

    let pdf_svc = match pdf_service.lock() {
        Ok(s) => s,
        Err(e) => {
            error!("获取PDF服务锁失败: {}", e);
            drop(ocr_svc);
            let _ = update_pdf_status(&db, pdf_id, "error", Some(&format!("PDF服务锁定失败: {}", e)));
            return Err(format!("PDF服务锁定失败: {}", e));
        }
    };

    let mut success_count = 0;
    let mut error_count = 0;

    // 处理每一页
    for page_num in 1..=page_count {
        info!("处理PDF页面: pdf_id={}, page={}/{}", pdf_id, page_num, page_count);

        match process_page(&pdf_id, page_num, &storage_path, &ocr_svc, &pdf_svc, &db) {
            Ok(_) => {
                success_count += 1;
                info!("页面处理成功: pdf_id={}, page={}", pdf_id, page_num);
            }
            Err(e) => {
                error_count += 1;
                error!("页面处理失败: pdf_id={}, page={}, 错误: {}", pdf_id, page_num, e);
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

    // 更新最终状态
    let final_status = if error_count == 0 {
        "done"
    } else if success_count == 0 {
        "error"
    } else {
        "done" // 部分成功也标记为完成
    };

    let error_message = if error_count > 0 {
        Some(format!("{}个页面处理失败", error_count))
    } else {
        None
    };

    if let Err(e) = update_pdf_status(&db, pdf_id, final_status, error_message.as_deref()) {
        error!("更新PDF最终状态失败: {}", e);
    }

    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: page_count as u32,
        total: page_count as u32,
        status: final_status.to_string(),
    });

    info!("OCR处理完成: pdf_id={}, filename={}, 成功={}, 失败={}", pdf_id, filename, success_count, error_count);
    Ok(())
}

/// 更新PDF状态
fn update_pdf_status(db: &State<'_, Db>, pdf_id: i64, status: &str, error_message: Option<&str>) -> Result<(), String> {
    let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;

    match error_message {
        Some(msg) => {
            conn.execute(
                "UPDATE pdfs SET status = ?1, error_message = ?2, updated_at = datetime('now') WHERE id = ?3",
                rusqlite::params![status, msg, pdf_id],
            ).map_err(|e| format!("更新状态失败: {}", e))?;
        }
        None => {
            conn.execute(
                "UPDATE pdfs SET status = ?1, error_message = NULL, updated_at = datetime('now') WHERE id = ?2",
                rusqlite::params![status, pdf_id],
            ).map_err(|e| format!("更新状态失败: {}", e))?;
        }
    }

    debug!("PDF状态已更新: pdf_id={}, status={}", pdf_id, status);
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
    debug!("渲染PDF页面: page={}, path={}", page_num, storage_path);

    // 渲染 PDF 页面为图像
    let image = pdf_service
        .render_page(std::path::Path::new(storage_path), page_num as u32)
        .map_err(|e| {
            error!("渲染PDF页面失败: page={}, 错误: {}", page_num, e);
            format!("渲染页面失败: {}", e)
        })?;

    debug!("PDF页面渲染成功: page={}", page_num);

    // OCR 识别
    let text = ocr_service
        .recognize(&image)
        .map_err(|e| {
            error!("OCR识别失败: page={}, 错误: {}", page_num, e);
            format!("OCR识别失败: {}", e)
        })?;

    debug!("OCR识别成功: page={}, 文本长度={}", page_num, text.len());

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

    debug!("OCR结果已保存: page={}", page_num);
    Ok(())
}