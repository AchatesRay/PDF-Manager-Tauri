use crate::db::{Db, get_setting, set_setting, SETTING_DATA_DIR, default_data_dir};
use rusqlite::Connection;
use tauri::{Manager, State};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub data_dir: String,
    pub log_dir: String,
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

    debug!("应用设置: data_dir={}, log_dir={}", data_dir, log_dir);
    Ok(AppSettings { data_dir, log_dir })
}

/// 设置数据目录
#[tauri::command]
pub fn set_data_dir(db: State<'_, Db>, app_handle: tauri::AppHandle, path: String) -> Result<(), String> {
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