use crate::services::ocr_service::OcrService;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::State;

#[derive(Debug, Serialize, Deserialize)]
pub struct OcrStatus {
    pub available: bool,
    pub languages: Vec<String>,
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