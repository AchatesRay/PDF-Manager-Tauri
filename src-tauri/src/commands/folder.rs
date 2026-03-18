use crate::db::{Db, get_setting, SETTING_DATA_DIR};
use crate::models::Folder;
use chrono::Utc;
use rusqlite::params;
use tauri::{Manager, State};
use tracing::{debug, error, info, warn};
use std::path::PathBuf;

/// 获取文件夹的完整存储路径
fn get_folder_storage_path(
    conn: &rusqlite::Connection,
    folder_id: Option<i64>,
    base_storage_path: Option<&str>,
) -> Option<PathBuf> {
    let mut path_parts: Vec<String> = Vec::new();
    let mut current_id = folder_id;

    // 向上遍历获取所有父文件夹名称
    while let Some(id) = current_id {
        let result: Result<(Option<i64>, String), _> = conn
            .query_row(
                "SELECT parent_id, name FROM folders WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            );

        if let Ok((parent_id, name)) = result {
            path_parts.push(name);
            current_id = parent_id;
        } else {
            break;
        }
    }

    // 反转得到从根到当前的路径
    path_parts.reverse();

    // 组合完整路径
    if let Some(base) = base_storage_path {
        let mut full_path = PathBuf::from(base);
        for part in path_parts {
            full_path.push(&part);
        }
        debug!("Calculated folder storage path: {:?}", full_path);
        Some(full_path)
    } else {
        debug!("No base storage path provided, returning None");
        None
    }
}

/// 获取所有文件夹
#[tauri::command]
pub fn get_folders(db: State<'_, Db>) -> Result<Vec<Folder>, String> {
    info!("开始获取所有文件夹列表");
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    let mut stmt = conn
        .prepare(
            "SELECT id, name, parent_id, storage_path, created_at, updated_at FROM folders ORDER BY name",
        )
        .map_err(|e| {
            error!("准备SQL语句失败: {}", e);
            format!("数据库查询准备失败: {}", e)
        })?;

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
        .map_err(|e| {
            error!("执行查询失败: {}", e);
            format!("数据库查询执行失败: {}", e)
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| {
            error!("解析查询结果失败: {}", e);
            format!("数据解析失败: {}", e)
        })?;

    info!("成功获取 {} 个文件夹", folders.len());
    Ok(folders)
}

/// 创建文件夹
#[tauri::command]
pub fn create_folder(
    db: State<'_, Db>,
    name: String,
    parent_id: Option<i64>,
    storage_path: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<Folder, String> {
    info!("开始创建文件夹: name={}, parent_id={:?}, storage_path={:?}", name, parent_id, storage_path);

    if name.trim().is_empty() {
        warn!("文件夹名称为空");
        return Err("文件夹名称不能为空".to_string());
    }

    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;
    let now = Utc::now().to_rfc3339();

    // 检查同名文件夹是否存在
    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM folders WHERE name = ?1 AND COALESCE(parent_id, 0) = COALESCE(?2, 0)",
        params![name, parent_id],
        |row| row.get(0),
    ).unwrap_or(0);

    if existing > 0 {
        warn!("同名文件夹已存在: name={}, parent_id={:?}", name, parent_id);
        return Err("同名文件夹已存在".to_string());
    }

    // 获取数据目录作为默认存储路径
    let default_data_dir = get_setting(&conn, SETTING_DATA_DIR)
        .unwrap_or_else(|| {
            app_handle
                .path()
                .app_data_dir()
                .expect("Failed to get app data directory")
                .join("data")
                .to_string_lossy()
                .to_string()
        });
    let default_pdfs_dir = PathBuf::from(&default_data_dir).join("pdfs");

    // 获取父文件夹的 storage_path
    let parent_storage_path: Option<String> = if let Some(pid) = parent_id {
        conn.query_row(
            "SELECT storage_path FROM folders WHERE id = ?1",
            params![pid],
            |row| row.get(0),
        ).ok().flatten()
    } else {
        // 对于根目录，使用用户指定的路径或默认的 pdfs 目录
        storage_path.clone().or_else(|| Some(default_pdfs_dir.to_string_lossy().to_string()))
    };

    debug!("父文件夹存储路径: {:?}", parent_storage_path);

    // 计算新文件夹的物理路径
    let physical_path = get_folder_storage_path(&conn, parent_id, parent_storage_path.as_deref());

    // 在物理存储中创建目录
    if let Some(ref path) = physical_path {
        debug!("尝试创建物理目录: {:?}", path);
        match std::fs::create_dir_all(path) {
            Ok(_) => info!("成功创建物理目录: {:?}", path),
            Err(e) => {
                error!("创建物理目录失败 '{}': {}", path.display(), e);
                return Err(format!("无法创建目录 '{}': {}", path.display(), e));
            }
        }
    } else {
        warn!("未指定存储路径，跳过物理目录创建");
    }

    // 保存计算后的存储路径到数据库
    let final_storage_path = parent_storage_path.clone();

    conn.execute(
        "INSERT INTO folders (name, parent_id, storage_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, parent_id, final_storage_path, now, now],
    )
    .map_err(|e| {
        error!("插入文件夹记录失败: {}", e);
        format!("数据库插入失败: {}", e)
    })?;

    let id = conn.last_insert_rowid();
    info!("文件夹创建成功: id={}, name={}", id, name);

    Ok(Folder {
        id,
        name,
        parent_id,
        storage_path: final_storage_path,
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
    info!("开始重命名文件夹: id={}, name={}, storage_path={:?}", id, name, storage_path);

    if name.trim().is_empty() {
        warn!("文件夹名称为空");
        return Err("文件夹名称不能为空".to_string());
    }

    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;
    let now = Utc::now().to_rfc3339();

    // 获取旧名称
    let old_name: String = conn
        .query_row("SELECT name FROM folders WHERE id = ?1", params![id], |row| row.get(0))
        .map_err(|e| {
            error!("文件夹不存在 (id={}): {}", id, e);
            format!("文件夹不存在: {}", e)
        })?;

    debug!("旧文件夹名称: {}", old_name);

    // 检查新名称是否与其他兄弟文件夹冲突
    let parent_id: Option<i64> = conn
        .query_row("SELECT parent_id FROM folders WHERE id = ?1", params![id], |row| row.get(0))
        .ok()
        .flatten();

    let existing: i64 = conn.query_row(
        "SELECT COUNT(*) FROM folders WHERE name = ?1 AND COALESCE(parent_id, 0) = COALESCE(?2, 0) AND id != ?3",
        params![name, parent_id, id],
        |row| row.get(0),
    ).unwrap_or(0);

    if existing > 0 {
        warn!("同名文件夹已存在: name={}", name);
        return Err("同名文件夹已存在".to_string());
    }

    // 获取文件夹的物理路径
    let parent_storage_path: Option<String> = conn.query_row(
        "SELECT f.storage_path FROM folders f WHERE f.id = ?1",
        params![id],
        |row| row.get(0),
    ).ok().flatten();

    // 如果有物理路径，重命名物理目录
    if let Some(ref base_path) = parent_storage_path {
        let old_path = PathBuf::from(base_path).join(&old_name);
        let new_path = PathBuf::from(base_path).join(&name);

        if old_path.exists() {
            debug!("尝试重命名物理目录: {:?} -> {:?}", old_path, new_path);
            match std::fs::rename(&old_path, &new_path) {
                Ok(_) => info!("成功重命名物理目录: {:?} -> {:?}", old_path, new_path),
                Err(e) => {
                    error!("重命名物理目录失败: {}", e);
                    return Err(format!("无法重命名目录: {}", e));
                }
            }
        } else {
            debug!("物理目录不存在，跳过重命名: {:?}", old_path);
        }
    }

    conn.execute(
        "UPDATE folders SET name = ?1, storage_path = ?2, updated_at = ?3 WHERE id = ?4",
        params![name, storage_path, now, id],
    )
    .map_err(|e| {
        error!("更新文件夹记录失败: {}", e);
        format!("数据库更新失败: {}", e)
    })?;

    info!("文件夹重命名成功: id={}, old_name={}, new_name={}", id, old_name, name);
    Ok(())
}

/// 删除文件夹
#[tauri::command]
pub fn delete_folder(db: State<'_, Db>, id: i64) -> Result<(), String> {
    info!("开始删除文件夹: id={}", id);
    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    // 检查是否有子文件夹
    let child_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE parent_id = ?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|e| {
            error!("查询子文件夹数量失败: {}", e);
            format!("数据库查询失败: {}", e)
        })?;

    if child_count > 0 {
        warn!("文件夹包含 {} 个子文件夹，无法删除", child_count);
        return Err(format!("无法删除包含 {} 个子文件夹的目录", child_count));
    }

    // 检查是否有PDF文件
    let pdf_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM pdfs WHERE folder_id = ?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|e| {
            error!("查询PDF文件数量失败: {}", e);
            format!("数据库查询失败: {}", e)
        })?;

    if pdf_count > 0 {
        warn!("文件夹包含 {} 个PDF文件，无法删除", pdf_count);
        return Err(format!("无法删除包含 {} 个PDF文件的目录", pdf_count));
    }

    // 获取文件夹信息用于删除物理目录
    let folder_info: Result<(String, Option<i64>), _> = conn
        .query_row(
            "SELECT name, parent_id FROM folders WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        );

    if let Ok((folder_name, parent_id)) = folder_info {
        // 获取父文件夹的 storage_path
        let parent_storage_path: Option<String> = if let Some(pid) = parent_id {
            conn.query_row(
                "SELECT storage_path FROM folders WHERE id = ?1",
                params![pid],
                |row| row.get(0),
            ).ok().flatten()
        } else {
            None
        };

        // 删除物理目录
        if let Some(ref base_path) = parent_storage_path {
            let physical_path = PathBuf::from(base_path).join(&folder_name);
            if physical_path.exists() {
                debug!("尝试删除物理目录: {:?}", physical_path);
                match std::fs::remove_dir(&physical_path) {
                    Ok(_) => info!("成功删除物理目录: {:?}", physical_path),
                    Err(e) => {
                        error!("删除物理目录失败: {}", e);
                        return Err(format!("无法删除目录: {}", e));
                    }
                }
            } else {
                debug!("物理目录不存在，跳过删除: {:?}", physical_path);
            }
        }
    }

    conn.execute("DELETE FROM folders WHERE id = ?1", params![id])
        .map_err(|e| {
            error!("删除文件夹记录失败: {}", e);
            format!("数据库删除失败: {}", e)
        })?;

    info!("文件夹删除成功: id={}", id);
    Ok(())
}