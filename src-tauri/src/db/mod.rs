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
    // 获取可执行文件所在目录作为默认数据目录
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| {
            app_handle
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory")
        });

    // 检查是否存在自定义数据目录配置
    let data_dir = check_custom_data_dir(&exe_dir, app_handle);

    info!("初始化数据库, 数据目录={:?}", data_dir);

    if let Err(e) = std::fs::create_dir_all(&data_dir) {
        error!("创建数据目录失败: {:?}, 错误: {}", data_dir, e);
        return Err(Box::new(e));
    }
    debug!("数据目录已确认存在: {:?}", data_dir);

    // 数据库文件放入数据目录中
    let db_path = data_dir.join("pdf-manager.db");
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

/// 检查是否有自定义数据目录配置
fn check_custom_data_dir(default_dir: &PathBuf, _app_handle: &tauri::AppHandle) -> PathBuf {
    // 先尝试在默认目录下查找数据库文件，检查是否有配置
    let default_db_path = default_dir.join("pdf-manager.db");

    if default_db_path.exists() {
        // 尝试读取配置
        if let Ok(conn) = Connection::open(&default_db_path) {
            if let Ok(custom_dir) = conn.query_row(
                "SELECT value FROM settings WHERE key = 'data_dir'",
                [],
                |row| row.get::<_, String>(0),
            ) {
                debug!("找到自定义数据目录配置: {}", custom_dir);
                return PathBuf::from(custom_dir);
            }
        }
    }

    // 检查环境变量
    if let Ok(custom_dir) = std::env::var("PDF_MANAGER_DATA_DIR") {
        debug!("使用环境变量指定的数据目录: {}", custom_dir);
        return PathBuf::from(custom_dir);
    }

    // 使用默认目录
    default_dir.clone()
}

// ===== 设置管理 =====

/// 设置键名
pub const SETTING_DATA_DIR: &str = "data_dir";
pub const SETTING_PDF_READER: &str = "pdf_reader_path";
pub const SETTING_OCR_MAX_IMAGE_DIMENSION: &str = "ocr_max_image_dimension";

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

/// 获取默认数据目录 (可执行文件所在目录)
pub fn default_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    // 优先使用可执行文件所在目录
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            debug!("默认数据目录 (可执行文件目录): {:?}", exe_dir);
            return exe_dir.to_path_buf();
        }
    }

    // 回退到 app_data_dir
    let path = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    debug!("默认数据目录 (app_data_dir): {:?}", path);
    path
}

// ===== 路径获取函数 =====

/// 获取数据存储目录 (使用配置的目录)
pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    // 尝试从数据库获取配置的数据目录
    // 由于可能在 setup 早期调用，需要从 app state 获取数据库
    if let Some(db) = app_handle.try_state::<Db>() {
        if let Ok(conn) = db.lock() {
            if let Some(custom_dir) = get_setting(&conn, SETTING_DATA_DIR) {
                debug!("使用自定义数据目录: {}", custom_dir);
                return PathBuf::from(custom_dir);
            }
        }
    }
    // 回退到默认目录
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