use crate::db::Db;
use crate::models::Folder;
use chrono::Utc;
use rusqlite::params;
use tauri::State;
use tracing::info;

/// 获取所有文件夹
#[tauri::command]
pub fn get_folders(db: State<'_, Db>) -> Result<Vec<Folder>, String> {
    info!("Getting all folders");
    let conn = db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, parent_id, storage_path, created_at, updated_at FROM folders ORDER BY name",
        )
        .map_err(|e| e.to_string())?;

    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                storage_path: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    info!("Found {} folders", folders.len());
    Ok(folders)
}

/// 创建文件夹
#[tauri::command]
pub fn create_folder(
    db: State<'_, Db>,
    name: String,
    parent_id: Option<i64>,
    storage_path: Option<String>,
) -> Result<Folder, String> {
    info!("Creating folder: name={}, parent_id={:?}, storage_path={:?}", name, parent_id, storage_path);
    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO folders (name, parent_id, storage_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, parent_id, storage_path, now, now],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();
    info!("Folder created: id={}", id);

    Ok(Folder {
        id,
        name,
        parent_id,
        storage_path,
        created_at: chrono::DateTime::parse_from_rfc3339(&now)
            .unwrap()
            .with_timezone(&Utc),
        updated_at: chrono::DateTime::parse_from_rfc3339(&now)
            .unwrap()
            .with_timezone(&Utc),
    })
}

/// 重命名文件夹
#[tauri::command]
pub fn rename_folder(
    db: State<'_, Db>,
    id: i64,
    name: String,
    storage_path: Option<String>,
) -> Result<(), String> {
    info!("Renaming folder: id={}, name={}, storage_path={:?}", id, name, storage_path);
    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE folders SET name = ?1, storage_path = ?2, updated_at = ?3 WHERE id = ?4",
        params![name, storage_path, now, id],
    )
    .map_err(|e| e.to_string())?;

    info!("Folder renamed: id={}", id);
    Ok(())
}

/// 删除文件夹
#[tauri::command]
pub fn delete_folder(db: State<'_, Db>, id: i64) -> Result<(), String> {
    info!("Deleting folder: id={}", id);
    let conn = db.lock().map_err(|e| e.to_string())?;

    let child_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE parent_id = ?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    if child_count > 0 {
        return Err("Cannot delete folder with subfolders".to_string());
    }

    let pdf_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pdfs WHERE folder_id = ?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    if pdf_count > 0 {
        return Err("Cannot delete folder with PDFs".to_string());
    }

    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;

    info!("Folder deleted: id={}", id);
    Ok(())
}