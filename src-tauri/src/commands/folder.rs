use crate::db::Db;
use crate::models::Folder;
use chrono::Utc;
use rusqlite::params;
use tauri::State;

/// 获取所有文件夹
#[tauri::command]
pub fn get_folders(db: State<'_, Db>) -> Result<Vec<Folder>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, parent_id, created_at, updated_at FROM folders ORDER BY name",
        )
        .map_err(|e| e.to_string())?;

    let folders = stmt
        .query_map([], |row| {
            Ok(Folder {
                id: row.get(0)?,
                name: row.get(1)?,
                parent_id: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;

    Ok(folders)
}

/// 创建文件夹
#[tauri::command]
pub fn create_folder(db: State<'_, Db>, name: String, parent_id: Option<i64>) -> Result<Folder, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO folders (name, parent_id, created_at, updated_at) VALUES (?1, ?2, ?3, ?4)",
        params![name, parent_id, now, now],
    )
    .map_err(|e| e.to_string())?;

    let id = conn.last_insert_rowid();

    Ok(Folder {
        id,
        name,
        parent_id,
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
pub fn rename_folder(db: State<'_, Db>, id: i64, name: String) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE folders SET name = ?1, updated_at = ?2 WHERE id = ?3",
        params![name, now, id],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

/// 删除文件夹
#[tauri::command]
pub fn delete_folder(db: State<'_, Db>, id: i64) -> Result<(), String> {
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

    Ok(())
}