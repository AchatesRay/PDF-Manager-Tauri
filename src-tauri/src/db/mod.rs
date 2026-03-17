mod schema;
#[cfg(test)]
mod tests;

use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub use schema::SCHEMA;

pub type Db = Mutex<Connection>;

/// 初始化数据库
pub fn init_database(app_handle: &tauri::AppHandle) -> Result<Connection, Box<dyn std::error::Error>> {
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    std::fs::create_dir_all(&app_dir)?;

    let db_path = app_dir.join("pdf-manager.db");
    let conn = Connection::open(&db_path)?;
    conn.execute_batch(SCHEMA)?;

    tracing::info!("Database initialized at {:?}", db_path);

    Ok(conn)
}

/// 获取数据存储目录
pub fn get_data_dir(app_handle: &tauri::AppHandle) -> PathBuf {
    app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory")
        .join("data")
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