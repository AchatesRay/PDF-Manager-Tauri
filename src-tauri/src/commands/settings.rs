use crate::db::{Db, get_setting, set_setting, SETTING_DATA_DIR, SETTING_PDF_READER, SETTING_OCR_MAX_IMAGE_DIMENSION, default_data_dir};
use tauri::State;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

/// 默认最大图像尺寸
pub const DEFAULT_OCR_MAX_IMAGE_DIMENSION: u32 = 2000;
/// 最小允许值
pub const MIN_OCR_MAX_IMAGE_DIMENSION: u32 = 500;
/// 最大允许值
pub const MAX_OCR_MAX_IMAGE_DIMENSION: u32 = 4000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub data_dir: String,
    pub log_dir: String,
    pub pdf_reader_path: Option<String>,
    pub ocr_max_image_dimension: u32,
}

/// 获取应用设置
#[tauri::command]
pub fn get_settings(db: State<'_, Db>, app_handle: tauri::AppHandle) -> Result<AppSettings, String> {
    debug!("开始获取应用设置");

    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    let data_dir = get_setting(&conn, SETTING_DATA_DIR)
        .unwrap_or_else(|| {
            let default = default_data_dir(&app_handle);
            debug!("使用默认数据目录: {:?}", default);
            default.to_string_lossy().to_string()
        });

    let log_dir = PathBuf::from(&data_dir).join("logs").to_string_lossy().to_string();

    let pdf_reader_path = get_setting(&conn, SETTING_PDF_READER);

    let ocr_max_image_dimension = get_setting(&conn, SETTING_OCR_MAX_IMAGE_DIMENSION)
        .and_then(|v| v.parse::<u32>().ok())
        .unwrap_or(DEFAULT_OCR_MAX_IMAGE_DIMENSION);

    debug!("应用设置: data_dir={}, log_dir={}, pdf_reader_path={:?}, ocr_max_image_dimension={}",
           data_dir, log_dir, pdf_reader_path, ocr_max_image_dimension);
    Ok(AppSettings { data_dir, log_dir, pdf_reader_path, ocr_max_image_dimension })
}

/// 设置数据目录
#[tauri::command]
pub fn set_data_dir(db: State<'_, Db>, _app_handle: tauri::AppHandle, path: String) -> Result<(), String> {
    info!("开始设置数据目录: {}", path);

    // 验证路径
    let path_buf = PathBuf::from(&path);

    // 检查路径是否有效
    if path.is_empty() {
        warn!("数据目录路径为空");
        return Err("数据目录路径不能为空".to_string());
    }

    // 检查路径是否是绝对路径
    if !path_buf.is_absolute() {
        warn!("数据目录不是绝对路径: {}", path);
        return Err("数据目录必须是绝对路径".to_string());
    }

    // 创建目录（如果不存在）
    if !path_buf.exists() {
        info!("数据目录不存在，尝试创建: {:?}", path_buf);
        match std::fs::create_dir_all(&path_buf) {
            Ok(_) => info!("成功创建数据目录: {:?}", path_buf),
            Err(e) => {
                error!("创建数据目录失败 '{}': {}", path, e);
                return Err(format!("无法创建目录 '{}': {}", path, e));
            }
        }
    }

    // 检查目录是否可写
    let test_file = path_buf.join(".write_test");
    match std::fs::write(&test_file, "test") {
        Ok(_) => {
            let _ = std::fs::remove_file(&test_file);
            debug!("数据目录可写: {:?}", path_buf);
        }
        Err(e) => {
            error!("数据目录不可写 '{}': {}", path, e);
            return Err(format!("目录不可写 '{}': {}", path, e));
        }
    }

    // 创建子目录
    let subdirs = ["pdfs", "thumbnails", "index", "logs"];
    for subdir in &subdirs {
        let subdir_path = path_buf.join(subdir);
        match std::fs::create_dir_all(&subdir_path) {
            Ok(_) => debug!("创建子目录: {:?}", subdir_path),
            Err(e) => {
                warn!("创建子目录失败 {:?}: {}", subdir_path, e);
            }
        }
    }

    // 保存设置
    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    match set_setting(&conn, SETTING_DATA_DIR, &path) {
        Ok(_) => info!("数据目录设置已保存: {}", path),
        Err(e) => {
            error!("保存设置失败: {}", e);
            return Err(format!("保存设置失败: {}", e));
        }
    }

    info!("数据目录设置成功: {}", path);
    Ok(())
}

/// 重置数据目录为默认值
#[tauri::command]
pub fn reset_data_dir(db: State<'_, Db>) -> Result<String, String> {
    info!("开始重置数据目录为默认值");

    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    match conn.execute("DELETE FROM settings WHERE key = ?1", [SETTING_DATA_DIR]) {
        Ok(rows) => debug!("删除了 {} 条设置记录", rows),
        Err(e) => {
            error!("重置数据目录失败: {}", e);
            return Err(format!("重置失败: {}", e));
        }
    }

    info!("数据目录已重置为默认值");
    Ok("数据目录已重置".to_string())
}

/// 设置PDF阅读器路径
#[tauri::command]
pub fn set_pdf_reader(db: State<'_, Db>, path: Option<String>) -> Result<(), String> {
    info!("开始设置PDF阅读器路径: {:?}", path);

    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    match path {
        Some(ref p) => {
            // 验证路径
            let path_buf = PathBuf::from(p);
            if !path_buf.exists() {
                warn!("PDF阅读器不存在: {}", p);
                return Err(format!("文件不存在: {}", p));
            }
            match set_setting(&conn, SETTING_PDF_READER, p) {
                Ok(_) => info!("PDF阅读器路径已保存: {}", p),
                Err(e) => {
                    error!("保存设置失败: {}", e);
                    return Err(format!("保存设置失败: {}", e));
                }
            }
        }
        None => {
            // 清除设置，使用系统默认
            match conn.execute("DELETE FROM settings WHERE key = ?1", [SETTING_PDF_READER]) {
                Ok(_) => info!("PDF阅读器路径已清除，将使用系统默认"),
                Err(e) => {
                    error!("清除PDF阅读器设置失败: {}", e);
                    return Err(format!("清除设置失败: {}", e));
                }
            }
        }
    }

    Ok(())
}

/// 设置 OCR 最大图像尺寸
#[tauri::command]
pub fn set_ocr_max_image_dimension(db: State<'_, Db>, dimension: u32) -> Result<(), String> {
    info!("开始设置 OCR 最大图像尺寸: {}", dimension);

    // 验证范围
    if dimension < MIN_OCR_MAX_IMAGE_DIMENSION || dimension > MAX_OCR_MAX_IMAGE_DIMENSION {
        warn!("OCR 最大图像尺寸超出范围: {}", dimension);
        return Err(format!("尺寸必须在 {}-{} 之间", MIN_OCR_MAX_IMAGE_DIMENSION, MAX_OCR_MAX_IMAGE_DIMENSION));
    }

    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    match set_setting(&conn, SETTING_OCR_MAX_IMAGE_DIMENSION, &dimension.to_string()) {
        Ok(_) => info!("OCR 最大图像尺寸已保存: {}", dimension),
        Err(e) => {
            error!("保存设置失败: {}", e);
            return Err(format!("保存设置失败: {}", e));
        }
    }

    Ok(())
}

/// 使用外部阅读器打开PDF
#[tauri::command]
pub fn open_pdf_externally(db: State<'_, Db>, pdf_path: String) -> Result<(), String> {
    info!("打开PDF文件: {}", pdf_path);

    // 检查文件是否存在
    let path = PathBuf::from(&pdf_path);
    if !path.exists() {
        error!("PDF文件不存在: {}", pdf_path);
        return Err(format!("文件不存在: {}", pdf_path));
    }

    // 获取配置的PDF阅读器路径
    let conn = match db.lock() {
        Ok(c) => c,
        Err(e) => {
            error!("获取数据库锁失败: {}", e);
            return Err(format!("数据库锁定失败: {}", e));
        }
    };

    let reader_path = get_setting(&conn, SETTING_PDF_READER);

    let result = if let Some(ref reader) = reader_path {
        // 使用指定的阅读器
        info!("使用指定阅读器打开PDF: {} {}", reader, pdf_path);
        std::process::Command::new(reader)
            .arg(&pdf_path)
            .spawn()
    } else {
        // 使用系统默认程序打开
        info!("使用系统默认程序打开PDF: {}", pdf_path);
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", &pdf_path])
                .spawn()
        }
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open")
                .arg(&pdf_path)
                .spawn()
        }
        #[cfg(target_os = "linux")]
        {
            std::process::Command::new("xdg-open")
                .arg(&pdf_path)
                .spawn()
        }
    };

    match result {
        Ok(_) => {
            info!("PDF文件打开成功: {}", pdf_path);
            Ok(())
        }
        Err(e) => {
            error!("打开PDF文件失败: {}", e);
            Err(format!("打开文件失败: {}", e))
        }
    }
}