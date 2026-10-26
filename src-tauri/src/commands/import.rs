use crate::commands::pdf::add_pdf_impl;
use crate::db::{get_configured_data_dir, Db};
use crate::services::pdf_service::PdfService;
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};
use tracing::{info, warn};
use walkdir::WalkDir;

/// 文件夹导入进度（通过 Tauri 事件推送到前端）
#[derive(Debug, Clone, Serialize)]
pub struct FolderImportProgress {
    pub current: usize,
    pub total: usize,
    pub filename: String,
    /// 相对源文件夹的子目录（根目录为空字符串），统一用 `/` 分隔
    pub relative_dir: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderImportFailure {
    pub path: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FolderImportReport {
    /// 导入后创建的顶级文件夹 ID（父目录按 `parent_id` 参数）
    pub folder_id: i64,
    /// 顶级文件夹名 = 源文件夹名
    pub folder_name: String,
    pub total_pdfs: usize,
    pub imported: usize,
    /// 已存在而跳过的 PDF（相对路径）
    pub skipped: Vec<String>,
    pub failed: Vec<FolderImportFailure>,
    pub created_folders: usize,
    /// 同名文件夹已存在时复用（再次导入同一来源）
    pub reused_folder: bool,
}

/// 从源目录名推导导入后的顶级文件夹名。
fn derive_root_name(src: &Path) -> String {
    src.file_name()
        .map(|n| n.to_string_lossy().trim().to_string())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "导入的文件夹".to_string())
}

/// 递归收集 PDF 文件，按路径不区分大小写排序，保证导入顺序稳定。
fn collect_pdf_files(src: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = WalkDir::new(src)
        .follow_links(false)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_file())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("pdf"))
                .unwrap_or(false)
        })
        .map(|entry| entry.into_path())
        .collect();

    files.sort_by(|a, b| {
        a.to_string_lossy()
            .to_lowercase()
            .cmp(&b.to_string_lossy().to_lowercase())
    });
    files
}

/// 计算文件夹的物理路径：向上取第一个 storage_path 作为根，再拼接各级名称。
/// 与 `commands/pdf.rs::get_folder_storage_path` 的口径保持一致。
fn folder_fs_path(conn: &rusqlite::Connection, folder_id: i64) -> Option<PathBuf> {
    let mut parts: Vec<String> = Vec::new();
    let mut current_id = Some(folder_id);
    let mut root_storage_path: Option<String> = None;

    while let Some(id) = current_id {
        let result: Result<(Option<i64>, String, Option<String>), _> = conn.query_row(
            "SELECT parent_id, name, storage_path FROM folders WHERE id = ?1",
            params![id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        );

        match result {
            Ok((parent_id, name, storage_path)) => {
                parts.push(name);
                if root_storage_path.is_none() {
                    root_storage_path = storage_path;
                }
                current_id = parent_id;
            }
            Err(_) => break,
        }
    }

    parts.reverse();

    let mut path = PathBuf::from(root_storage_path?);
    for part in parts {
        path.push(part);
    }
    Some(path)
}

/// 确保指定名称的文件夹记录存在（不存在则新建并创建物理目录）。
///
/// 返回 `(folder_id, created)`。同名同父文件夹已存在时直接复用，
/// 这样重复导入同一来源不会产生 `xxx (2)` 之类的重复目录。
fn ensure_folder(
    db: &Db,
    name: &str,
    parent_id: Option<i64>,
    base_storage: &Path,
) -> Result<(i64, bool), String> {
    let conn = db
        .lock()
        .map_err(|e| format!("数据库锁定失败: {}", e))?;

    let existing: Option<i64> = conn
        .query_row(
            "SELECT id FROM folders WHERE name = ?1 AND COALESCE(parent_id, -1) = COALESCE(?2, -1)",
            params![name, parent_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("查询文件夹失败: {}", e))?;

    if let Some(id) = existing {
        let path = folder_fs_path(&conn, id);
        drop(conn);
        if let Some(path) = path {
            let _ = std::fs::create_dir_all(&path);
        }
        return Ok((id, false));
    }

    // 与 create_folder 一致：根目录使用数据目录下的 pdfs 作为基准；
    // 子目录继承父目录的 storage_path。
    let parent_storage: Option<String> = match parent_id {
        Some(pid) => conn
            .query_row(
                "SELECT storage_path FROM folders WHERE id = ?1",
                params![pid],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| format!("查询父文件夹失败: {}", e))?
            .flatten(),
        None => Some(base_storage.to_string_lossy().to_string()),
    };

    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO folders (name, parent_id, storage_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![name, parent_id, parent_storage, now, now],
    )
    .map_err(|e| format!("创建文件夹失败 '{}': {}", name, e))?;

    let id = conn.last_insert_rowid();
    let path = folder_fs_path(&conn, id);
    drop(conn);

    if let Some(path) = path {
        std::fs::create_dir_all(&path)
            .map_err(|e| format!("无法创建目录 '{}': {}", path.display(), e))?;
    }

    Ok((id, true))
}

/// 按相对路径逐级确保子文件夹存在。
fn ensure_relative_dir(
    db: &Db,
    rel_dir: &Path,
    root_id: i64,
    base_storage: &Path,
    cache: &mut HashMap<PathBuf, i64>,
    created_folders: &mut usize,
) -> Result<i64, String> {
    if let Some(id) = cache.get(rel_dir) {
        return Ok(*id);
    }

    let (parent_id, name) = match rel_dir.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            let parent_id = ensure_relative_dir(db, parent, root_id, base_storage, cache, created_folders)?;
            (parent_id, rel_dir.file_name())
        }
        _ => (root_id, rel_dir.file_name()),
    };

    let name = name
        .map(|n| n.to_string_lossy().to_string())
        .ok_or_else(|| "无效的子文件夹路径".to_string())?;

    let (id, created) = ensure_folder(db, &name, Some(parent_id), base_storage)?;
    if created {
        *created_folders += 1;
    }
    cache.insert(rel_dir.to_path_buf(), id);
    Ok(id)
}

/// 阻塞式导入主体（在 spawn_blocking 中执行，避免卡住 UI）。
fn import_folder_blocking(
    app_handle: &AppHandle,
    path: &str,
    parent_id: Option<i64>,
) -> Result<FolderImportReport, String> {
    let src = PathBuf::from(path)
        .canonicalize()
        .map_err(|e| format!("无法访问文件夹 '{}': {}", path, e))?;

    if !src.is_dir() {
        return Err(format!("所选路径不是文件夹: {}", src.display()));
    }

    let pdf_files = collect_pdf_files(&src);
    if pdf_files.is_empty() {
        return Err("所选文件夹及其子文件夹中没有找到 PDF 文件".to_string());
    }

    let root_name = derive_root_name(&src);
    let db = app_handle.state::<Db>();
    let pdf_service = app_handle.state::<Mutex<PdfService>>();

    // 指定了父分类时先校验其存在，避免产生悬挂 parent_id
    if let Some(pid) = parent_id {
        let conn = db
            .lock()
            .map_err(|e| format!("数据库锁定失败: {}", e))?;
        let exists: Option<i64> = conn
            .query_row("SELECT id FROM folders WHERE id = ?1", params![pid], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| format!("查询父分类失败: {}", e))?;
        if exists.is_none() {
            return Err(format!("目标分类不存在: {}", pid));
        }
    }

    // 取基准存储目录。get_configured_data_dir 只需 conn，不回调 Db 锁，
    // 但仍显式释放锁后再进入逐文件导入。
    let base_storage = {
        let conn = db
            .lock()
            .map_err(|e| format!("数据库锁定失败: {}", e))?;
        get_configured_data_dir(&conn, app_handle).join("pdfs")
    };

    // 防止把应用自身的 PDF 存储目录当源目录导入（否则会自我嵌套复制）
    if let Ok(base_canonical) = base_storage.canonicalize() {
        if src.starts_with(&base_canonical) {
            return Err("不能导入应用自身的 PDF 存储目录".to_string());
        }
    }

    let (root_id, reused_root) = ensure_folder(db.inner(), &root_name, parent_id, &base_storage)?;
    let mut created_folders = if reused_root { 0 } else { 1 };

    let total = pdf_files.len();
    let mut imported = 0usize;
    let mut skipped: Vec<String> = Vec::new();
    let mut failed: Vec<FolderImportFailure> = Vec::new();
    let mut dir_cache: HashMap<PathBuf, i64> = HashMap::new();
    dir_cache.insert(PathBuf::new(), root_id);

    info!(
        "开始导入 PDF 文件夹: {:?} -> 文件夹 '{}' (id={}), 共 {} 个 PDF",
        src, root_name, root_id, total
    );

    for (idx, file) in pdf_files.iter().enumerate() {
        let rel = file.strip_prefix(&src).unwrap_or(file).to_path_buf();
        let rel_dir = rel.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        let relative_dir = rel_dir.to_string_lossy().replace('\\', "/");
        let filename = file
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let folder_id = match ensure_relative_dir(
            db.inner(),
            &rel_dir,
            root_id,
            &base_storage,
            &mut dir_cache,
            &mut created_folders,
        ) {
            Ok(id) => id,
            Err(e) => {
                warn!("创建子文件夹失败 {:?}: {}", rel_dir, e);
                failed.push(FolderImportFailure {
                    path: rel.display().to_string(),
                    reason: e,
                });
                continue;
            }
        };

        let _ = app_handle.emit(
            "folder-import-progress",
            FolderImportProgress {
                current: idx + 1,
                total,
                filename: filename.clone(),
                relative_dir: relative_dir.clone(),
            },
        );

        match add_pdf_impl(
            file.to_string_lossy().to_string(),
            Some(folder_id),
            db.inner(),
            pdf_service.inner(),
            app_handle,
        ) {
            Ok(_) => imported += 1,
            Err(e) => {
                if e.contains("已添加过") || e.contains("文件名已存在") {
                    let rel_path = if relative_dir.is_empty() {
                        filename.clone()
                    } else {
                        format!("{}/{}", relative_dir, filename)
                    };
                    skipped.push(rel_path);
                } else {
                    warn!("导入 PDF 失败 {:?}: {}", file, e);
                    failed.push(FolderImportFailure {
                        path: rel.display().to_string(),
                        reason: e,
                    });
                }
            }
        }
    }

    info!(
        "文件夹导入完成: {:?}, 成功 {} / 共 {}, 跳过 {}, 失败 {}, 新建目录 {}",
        src,
        imported,
        total,
        skipped.len(),
        failed.len(),
        created_folders
    );

    Ok(FolderImportReport {
        folder_id: root_id,
        folder_name: root_name,
        total_pdfs: total,
        imported,
        skipped,
        failed,
        created_folders,
        reused_folder: reused_root,
    })
}

/// 导入本地已有的 PDF 文件夹。
///
/// - 顶级文件夹名 = 所选文件夹的目录名；
/// - 递归导入子文件夹，并在应用中镜像相同的目录结构；
/// - PDF 会被复制到应用存储目录，重复文件自动跳过；
/// - 通过 `folder-import-progress` 事件推送进度。
#[tauri::command]
pub async fn import_pdf_folder(
    path: String,
    parent_id: Option<i64>,
    app_handle: AppHandle,
) -> Result<FolderImportReport, String> {
    info!("收到导入文件夹请求: path={}, parent_id={:?}", path, parent_id);

    tauri::async_runtime::spawn_blocking(move || {
        import_folder_blocking(&app_handle, &path, parent_id)
    })
    .await
    .map_err(|e| format!("导入任务执行失败: {}", e))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_derive_root_name() {
        assert_eq!(derive_root_name(Path::new(r"D:\docs\项目资料")), "项目资料");
        assert_eq!(derive_root_name(Path::new(r"C:\")), "导入的文件夹");
        assert_eq!(derive_root_name(Path::new("/")), "导入的文件夹");
    }

    #[test]
    fn test_collect_pdf_files_recursive_and_filter() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("sub")).unwrap();
        std::fs::write(dir.path().join("a.pdf"), b"%PDF-1.4").unwrap();
        std::fs::write(dir.path().join("b.PDF"), b"%PDF-1.4").unwrap();
        std::fs::write(dir.path().join("note.txt"), b"x").unwrap();
        std::fs::write(dir.path().join("sub").join("c.pdf"), b"%PDF-1.4").unwrap();

        let files = collect_pdf_files(dir.path());
        assert_eq!(files.len(), 3);
        assert!(files
            .iter()
            .all(|p| p.extension().unwrap().to_string_lossy().eq_ignore_ascii_case("pdf")));
    }
}
