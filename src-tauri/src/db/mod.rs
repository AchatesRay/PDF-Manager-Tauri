mod schema;
#[cfg(test)]
mod tests;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;
use tracing::{debug, error, info, warn};

pub use schema::SCHEMA;
pub use schema::MIGRATIONS;

pub type Db = Mutex<Connection>;

/// 初始化数据库
pub fn init_database(app_handle: &tauri::AppHandle) -> Result<Connection, Box<dyn std::error::Error>> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    info!("初始化数据库, app_dir={:?}", app_dir);

    if let Err(e) = std::fs::create_dir_all(&app_dir) {
        error!("创建应用目录失败: {:?}, 错误: {}", app_dir, e);
        return Err(Box::new(e));
    }
    debug!("应用目录已确认存在: {:?}", app_dir);

    let db_path = app_dir.join("pdf-manager.db");
    info!("数据库路径: {:?}", db_path);

    let conn = match Connection::open(&db_path) {
        Ok(c) => {
            debug!("数据库连接成功");
            c
        }
        Err(e) => {
            error!("打开数据库失败: {:?}, 错误: {}", db_path, e);
            return Err(Box::new(e));
        }
    };

    match conn.execute_batch(SCHEMA) {
        Ok(_) => debug!("数据库Schema创建成功"),
        Err(e) => {
            error!("执行Schema失败: {}", e);
            return Err(Box::new(e));
        }
    }

    // 运行迁移
    for (i, migration) in MIGRATIONS.iter().enumerate() {
        match conn.execute(migration, []) {
            Ok(_) => debug!("迁移 {} 执行成功", i + 1),
            Err(e) => {
                // 迁移可能已经执行过，忽略错误
                warn!("迁移 {} 执行跳过 (可能已存在): {}", i + 1, e);
            }
        }
    }

    info!("数据库初始化完成: {:?}", db_path);
    Ok(conn)
}

// ===== 设置管理 =====

/// 设置键名
pub const SETTING_DATA_DIR: &str = "data_dir";

/// 获取设置值
pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    match conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        [key],
        |row| row.get(0),
    ) {
        Ok(value) => {
            debug!("获取设置成功: key={}, value={}", key, value);
            Some(value)
        }
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            debug!("设置不存在: key={}", key);
            None
        }
        Err(e) => {
            warn!("查询设置失败: key={}, 错误: {}", key, e);
            None
        }
    }
}

/// 设置值
pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), rusqlite::Error> {
    debug!("保存设置: key={}, value={}", key, value);

    match conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        [key, value],
    ) {
        Ok(_) => {
            debug!("设置保存成功: key={}", key);
            Ok(())
        }
        Err(e) => {
            error!("保存设置失败: key={}, 错误: {}", key, e);
            Err(e)
        }
    }
}

/// 获取用户配置的数据目录，如果未配置则返回默认目录
pub fn get_configured_data_dir(conn: &Connection, app_handle: &tauri::AppHandle) -> PathBuf {
    match get_setting(conn, SETTING_DATA_DIR) {
        Some(custom_dir) => {
            debug!("使用自定义数据目录: {}", custom_dir);
            PathBuf::from(custom_dir)
        }
        None => {
            let default = default_data_dir(app_handle);
            debug!("使用默认数据目录: {:?}", default);
            default
        }
    }
}

/// 获取默认数据目录 (安装目录下)
pub fn default_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    let path = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory")
        .join("data");

    debug!("默认数据目录: {:?}", path);
    path
}

// ===== 路径获取函数 =====

/// 获取数据存储目录 (使用配置的目录)
pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    // 从数据库获取配置的数据目录
    // 这里需要从 managed state 获取数据库连接
    // 由于 setup 时数据库可能还未完全初始化，使用默认目录
    default_data_dir(app_handle)
}

/// 获取 PDF 存储目录
pub fn get_pdfs_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    get_data_dir(app_handle).join("pdfs")
}

/// 获取缩略图目录
pub fn get_thumbnails_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    get_data_dir(app_handle).join("thumbnails")
}

/// 获取索引目录
pub fn get_index_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    get_data_dir(app_handle).join("index")
}

/// 获取日志目录
pub fn get_log_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    get_data_dir(app_handle).join("logs")
}