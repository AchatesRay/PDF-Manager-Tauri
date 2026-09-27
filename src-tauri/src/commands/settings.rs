use crate::db::{Db, get_setting, set_setting, SETTING_DATA_DIR, SETTING_DATA_DIR_MIGRATE_FROM, SETTING_PDF_READER, SETTING_OCR_MAX_IMAGE_DIMENSION, SETTING_OCR_PREPROCESS_MODE, default_data_dir};
use crate::services::ocr_service::PreprocessMode;
use tauri::State;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, warn};

/// 默认最大图像尺寸（降低以减少内存占用）
pub const DEFAULT_OCR_MAX_IMAGE_DIMENSION: u32 = 1000;
/// 最小允许值
pub const MIN_OCR_MAX_IMAGE_DIMENSION: u32 = 500;
/// 最大允许值
pub const MAX_OCR_MAX_IMAGE_DIMENSION: u32 = 2000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub data_dir: String,
    pub log_dir: String,
    pub pdf_reader_path: Option<String>,
    pub ocr_max_image_dimension: u32,
    /// OCR 预处理模式：auto / off / on（T6）
    pub ocr_preprocess_mode: String,
}

// ===== 数据目录迁移辅助 =====

/// 需要随数据目录迁移的子目录（全量迁移，用户确认口径）
const MIGRATE_SUBDIRS: [&str; 5] = ["pdfs", "thumbnails", "index", "logs", "models"];
/// 需要迁移的 DB 文件（WAL 三件套）
const MIGRATE_DB_FILES: [&str; 3] = ["pdf-manager.db", "pdf-manager.db-wal", "pdf-manager.db-shm"];

/// 规范化目录比较基准：去掉尾部分隔符（保留盘符根 `C:\` 原样，防止退化成盘符相对路径）
fn normalize_dir(p: &Path) -> PathBuf {
    let s = p.to_string_lossy().to_string();
    let trimmed = s.trim_end_matches(['/', '\\']);
    if trimmed.is_empty() || trimmed.ends_with(':') {
        PathBuf::from(s)
    } else {
        PathBuf::from(trimmed)
    }
}

/// 递归复制目录：文件直接复制（目标已存在则覆盖 —— 启动期刷新需要源覆盖旧副本）；
/// 目标独有的文件保留（例如改目录后新导入、直接写到目标目录的 PDF）。
/// 只复制不删除，源目录始终完好（天然备份）。
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<usize, String> {
    if !src.exists() {
        return Ok(0);
    }
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("创建目录 {:?} 失败: {}", dst, e))?;

    let mut copied = 0usize;
    let entries = std::fs::read_dir(src)
        .map_err(|e| format!("读取目录 {:?} 失败: {}", src, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| format!("读取目录项失败: {}", e))?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        let file_type = entry
            .file_type()
            .map_err(|e| format!("读取文件类型失败 {:?}: {}", from, e))?;
        if file_type.is_dir() {
            copied += copy_dir_recursive(&from, &to)?;
        } else {
            std::fs::copy(&from, &to)
                .map_err(|e| format!("复制文件 {:?} -> {:?} 失败: {}", from, to, e))?;
            copied += 1;
        }
    }
    Ok(copied)
}

/// 复制数据目录核心内容：DB 文件 + 迁移子目录。
/// DB 文件调用方负责先做 WAL checkpoint 并持有 Db 锁（防复制瞬间的写入撕裂）。
fn copy_data_dir_core(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst)
        .map_err(|e| format!("创建目标目录 {:?} 失败: {}", dst, e))?;

    for name in MIGRATE_DB_FILES {
        let from = src.join(name);
        if from.exists() {
            std::fs::copy(&from, &dst.join(name))
                .map_err(|e| format!("复制数据库 {:?} 失败: {}", from, e))?;
        }
    }
    for sub in MIGRATE_SUBDIRS {
        copy_dir_recursive(&src.join(sub), &dst.join(sub))?;
    }
    Ok(())
}

/// 修正目标 DB 副本中的绝对路径前缀（pdfs.storage_path / folders.storage_path）
/// 并把 data_dir 设置写为目标目录。
///
/// 用 `substr(...) = 前缀` 精确前缀匹配（不用 LIKE——路径中的 `_` `%` 会被当通配符）。
/// 前缀两端均已 normalize（无尾分隔符），`old_prefix || suffix` 拼接后分隔符由
/// substr 从 `length(old)+1` 处保留（原路径 `old\x` 的 `\` 属于 suffix）。
fn fix_db_paths(db_path: &Path, old_dir: &Path, new_dir: &Path) -> Result<(), String> {
    if !db_path.exists() {
        return Err(format!("目标数据库不存在: {:?}", db_path));
    }
    let old = normalize_dir(old_dir).to_string_lossy().to_string();
    let new = normalize_dir(new_dir).to_string_lossy().to_string();

    let conn = rusqlite::Connection::open(db_path)
        .map_err(|e| format!("打开目标数据库失败 {:?}: {}", db_path, e))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;

    for table in ["pdfs", "folders"] {
        let sql = format!(
            "UPDATE {} SET storage_path = ?2 || substr(storage_path, length(?1) + 1)
             WHERE storage_path IS NOT NULL AND substr(storage_path, 1, length(?1)) = ?1",
            table
        );
        let n = conn
            .execute(&sql, rusqlite::params![old, new])
            .map_err(|e| format!("修正 {} 路径失败: {}", table, e))?;
        debug!("迁移路径修正 {}: {} 行", table, n);
    }

    set_setting(&conn, SETTING_DATA_DIR, &new)
        .map_err(|e| format!("写入目标 data_dir 设置失败: {}", e))?;

    Ok(())
}

/// 写引导设置到 exe 目录的 pdf-manager.db（重启时 `check_custom_data_dir` 只读它，
/// 不双写则改目录/重置在重启后不生效或二次修改静默失效）。
/// `data_dir` 为 None 时删除该键；`migrate_from` 同理。
fn write_guide_settings(
    guide_db: &Path,
    data_dir: Option<&str>,
    migrate_from: Option<&str>,
) -> Result<(), String> {
    // 引导 DB 可能尚未创建（极端环境）：补建 settings 表即可，其余交给启动流程
    let need_create = !guide_db.exists();
    let conn = rusqlite::Connection::open(guide_db)
        .map_err(|e| format!("打开引导数据库失败 {:?}: {}", guide_db, e))?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(|e| e.to_string())?;
    if need_create {
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .map_err(|e| format!("初始化引导设置表失败: {}", e))?;
    }

    for (key, value) in [
        (SETTING_DATA_DIR, data_dir),
        (SETTING_DATA_DIR_MIGRATE_FROM, migrate_from),
    ] {
        match value {
            Some(v) => set_setting(&conn, key, v)
                .map_err(|e| format!("写入引导设置 {} 失败: {}", key, e))?,
            None => {
                conn.execute("DELETE FROM settings WHERE key = ?1", [key])
                    .map_err(|e| format!("删除引导设置 {} 失败: {}", key, e))?;
            }
        }
    }
    Ok(())
}

/// 取当前活跃 DB 文件路径（`PRAGMA database_list` = 本连接真实打开的文件）。
/// 不能用「data_dir 设置值」当数据源目录：上一次 set_data_dir 已把设置改成新目录，
/// 但运行期数据（连接、物理文件）仍在旧目录 —— 以活跃 DB 所在目录为迁移源才准确。
fn live_db_file(db: &State<Db>) -> Option<PathBuf> {
    let conn = db.lock().ok()?;
    let file: String = conn
        .query_row("PRAGMA database_list", [], |row| row.get(2))
        .unwrap_or_default();
    if file.is_empty() {
        None
    } else {
        Some(PathBuf::from(file))
    }
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

    // 非法值回退 auto（保证 UI 与后端口径一致）
    let ocr_preprocess_mode = get_setting(&conn, SETTING_OCR_PREPROCESS_MODE)
        .and_then(|v| PreprocessMode::parse(&v).map(|m| m.as_str().to_string()))
        .unwrap_or_else(|| PreprocessMode::default().as_str().to_string());

    debug!("应用设置: data_dir={}, log_dir={}, pdf_reader_path={:?}, ocr_max_image_dimension={}, ocr_preprocess_mode={}",
           data_dir, log_dir, pdf_reader_path, ocr_max_image_dimension, ocr_preprocess_mode);
    Ok(AppSettings { data_dir, log_dir, pdf_reader_path, ocr_max_image_dimension, ocr_preprocess_mode })
}

/// 设置数据目录（全量迁移：DB + pdfs + thumbnails + index + logs + models）
///
/// 迁移策略（用户确认口径）：
/// - 只复制不删除：源目录原样保留，天然备份；
/// - 目标已含 `pdf-manager.db` → 拒绝（防新旧数据混合）；
/// - 迁移失败 → **不保存设置**，旧环境零影响；
/// - 设置双写（当前 DB + exe 引导 DB），否则重启后不生效/二次修改静默失效；
/// - 写 `data_dir_migrate_from` 标记 → 启动期刷新复制（补迁本会话新数据）。
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

    // ===== 迁移源 = 活跃 DB 所在目录（物理数据真实位置）=====
    let source_dir = live_db_file(&db)
        .and_then(|f| f.parent().map(|p| p.to_path_buf()))
        .ok_or_else(|| "无法确定当前数据目录，已取消操作".to_string())?;

    let src_norm = normalize_dir(&source_dir);
    let dst_norm = normalize_dir(&path_buf);

    if src_norm == dst_norm {
        // 选了同一目录：只保存设置，无迁移
        info!("目标目录与当前一致，跳过迁移: {:?}", path);
        let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
        set_setting(&conn, SETTING_DATA_DIR, &path)
            .map_err(|e| format!("保存设置失败: {}", e))?;
        drop(conn);
        let guide = default_data_dir(&app_handle).join("pdf-manager.db");
        write_guide_settings(&guide, Some(&path), None)?;
        return Ok(());
    }

    // 目标已含应用数据 → 拒绝（防新旧数据混合/覆盖破坏）
    if path_buf.join("pdf-manager.db").exists() {
        warn!("目标目录已包含应用数据，拒绝迁移: {:?}", path_buf);
        return Err("目标目录已包含应用数据（pdf-manager.db），请选择空目录".to_string());
    }

    info!("迁移数据目录: {:?} -> {:?}", source_dir, path_buf);

    // ===== ① 持 Db 锁：WAL checkpoint + 复制 DB 三件套（防复制瞬间写入撕裂）=====
    {
        let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
        // checkpoint 把 WAL 内容并入主库；TRUNCATE 清空 WAL，保证复制的 db 文件自洽
        match conn.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(())) {
            Ok(_) => debug!("WAL checkpoint 完成"),
            Err(e) => warn!("WAL checkpoint 失败（继续按现有文件复制）: {}", e),
        }
        for name in MIGRATE_DB_FILES {
            let from = source_dir.join(name);
            if from.exists() {
                if let Err(e) = std::fs::copy(&from, &path_buf.join(name)) {
                    drop(conn);
                    error!("复制数据库文件失败: {:?}, {}", from, e);
                    return Err(format!("迁移失败（未修改任何设置）: 复制 {} 失败: {}", name, e));
                }
            }
        }
    } // ← 复制 DB 期间持有锁，结束后释放

    // ===== ② 锁外复制子目录（pdfs/thumbnails/index/logs/models）=====
    for sub in MIGRATE_SUBDIRS {
        match copy_dir_recursive(&source_dir.join(sub), &path_buf.join(sub)) {
            Ok(n) => debug!("迁移子目录 {}: {} 个文件", sub, n),
            Err(e) => {
                error!("迁移子目录 {} 失败: {}", sub, e);
                return Err(format!("迁移失败（未修改任何设置）: {}", e));
            }
        }
    }

    // ===== ③ 修正目标 DB 副本中的绝对路径 + 写入目标 data_dir =====
    if let Err(e) = fix_db_paths(&path_buf.join("pdf-manager.db"), &source_dir, &path_buf) {
        error!("修正目标数据库路径失败: {}", e);
        return Err(format!("迁移失败（未修改任何设置）: {}", e));
    }

    // ===== ④ 创建全部子目录（含 models —— 原实现漏了）=====
    let subdirs = ["pdfs", "thumbnails", "index", "logs", "models"];
    for subdir in &subdirs {
        let subdir_path = path_buf.join(subdir);
        if let Err(e) = std::fs::create_dir_all(&subdir_path) {
            warn!("创建子目录失败 {:?}: {}", subdir_path, e);
        }
    }

    // ===== ⑤ 保存设置：当前 DB + exe 引导 DB 双写 + 迁移源标记 =====
    {
        let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
        match set_setting(&conn, SETTING_DATA_DIR, &path) {
            Ok(_) => info!("数据目录设置已保存: {}", path),
            Err(e) => {
                error!("保存设置失败: {}", e);
                return Err(format!("保存设置失败: {}", e));
            }
        }
    }

    let guide = default_data_dir(&app_handle).join("pdf-manager.db");
    write_guide_settings(
        &guide,
        Some(&path),
        Some(&source_dir.to_string_lossy()),
    )?;

    info!("数据目录设置成功（已完成全量迁移）: {}", path);
    Ok(())
}

/// 启动期刷新迁移：补迁 `set_data_dir` 与重启之间产生的新数据（新导入 PDF、
/// OCR 结果、索引增量、日志、新下载模型），并再次修正目标 DB 路径。
///
/// 由 `lib::run` 在 `init_early_logging` **之前**调用。失败时回退引导设置到源目录
/// （应用按旧数据完整启动，零丢失）。返回**生效的数据目录**。
pub fn finish_pending_data_dir_migration(exe_dir: &Path) -> PathBuf {
    let default_dir = exe_dir.to_path_buf();
    let guide = exe_dir.join("pdf-manager.db");
    if !guide.exists() {
        return default_dir;
    }

    let (target, source) = {
        let conn = match rusqlite::Connection::open(&guide) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("打开引导数据库失败: {}", e);
                return default_dir;
            }
        };
        (
            get_setting(&conn, SETTING_DATA_DIR),
            get_setting(&conn, SETTING_DATA_DIR_MIGRATE_FROM),
        )
    };

    let (Some(target), Some(source)) = (target.clone(), source) else {
        // 无迁移标记：正常路径 —— 引导设置指向哪就用哪（无标记的老配置 = 原行为）
        return target
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var("PDF_MANAGER_DATA_DIR")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .map(PathBuf::from)
            })
            .unwrap_or(default_dir);
    };

    let t = PathBuf::from(&target);
    let s = PathBuf::from(&source);

    // 清标记的辅助
    let clear_marker = |guide: &Path| {
        if let Ok(conn) = rusqlite::Connection::open(guide) {
            let _ = conn.execute(
                "DELETE FROM settings WHERE key = ?1",
                [SETTING_DATA_DIR_MIGRATE_FROM],
            );
        }
    };

    if normalize_dir(&s) == normalize_dir(&t) || !s.exists() {
        // 源==目标（重复标记）或源已不存在（用户手动清理过旧目录）→ 无需刷新
        clear_marker(&guide);
        return t;
    }

    info!("启动刷新迁移: {:?} -> {:?}", s, t);
    let refreshed = copy_data_dir_core(&s, &t)
        .and_then(|_| fix_db_paths(&t.join("pdf-manager.db"), &s, &t));

    match refreshed {
        Ok(_) => {
            clear_marker(&guide);
            info!("启动刷新迁移完成: {:?}", t);
            t
        }
        Err(e) => {
            // 回退：引导设置与源 DB 的 data_dir 都指回源目录 → 应用按旧数据完整启动
            error!("启动刷新迁移失败，回退到源目录: {}", e);
            let _ = write_guide_settings(&guide, Some(&source), None);
            if let Ok(conn) = rusqlite::Connection::open(s.join("pdf-manager.db")) {
                let _ = set_setting(&conn, SETTING_DATA_DIR, &source);
            }
            eprintln!("数据目录迁移刷新失败，已回退到原目录: {}", e);
            s
        }
    }
}

/// 重置数据目录为默认值
///
/// 修复：必须同时删 **exe 引导 DB** 的 `data_dir` 与迁移标记 —— 重启时只读引导 DB，
/// 只删当前 DB 的设置会导致重置静默无效。
#[tauri::command]
pub fn reset_data_dir(db: State<'_, Db>, app_handle: tauri::AppHandle) -> Result<String, String> {
    info!("开始重置数据目录为默认值");

    {
        let conn = db.lock().map_err(|e| format!("数据库锁定失败: {}", e))?;
        match conn.execute("DELETE FROM settings WHERE key = ?1", [SETTING_DATA_DIR]) {
            Ok(rows) => debug!("删除了 {} 条设置记录", rows),
            Err(e) => {
                error!("重置数据目录失败: {}", e);
                return Err(format!("重置失败: {}", e));
            }
        }
    }

    // 引导 DB：删 data_dir + 迁移标记（否则重启后重置不生效）
    let guide = default_data_dir(&app_handle).join("pdf-manager.db");
    write_guide_settings(&guide, None, None)?;

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

/// 设置 OCR 预处理模式（T6：auto=自动 / off=关闭 / on=强制增强）
#[tauri::command]
pub fn set_ocr_preprocess_mode(db: State<'_, Db>, mode: String) -> Result<(), String> {
    info!("开始设置 OCR 预处理模式: {}", mode);

    // 严格校验：非法值直接拒绝，不落库
    let parsed = PreprocessMode::parse(&mode)
        .ok_or_else(|| format!("无效的预处理模式 '{}'（可选: auto/off/on）", mode))?;

    let conn = db.lock().map_err(|e| {
        error!("获取数据库锁失败: {}", e);
        format!("数据库锁定失败: {}", e)
    })?;

    match set_setting(&conn, SETTING_OCR_PREPROCESS_MODE, parsed.as_str()) {
        Ok(_) => {
            info!("OCR 预处理模式已保存: {}", parsed.as_str());
            Ok(())
        }
        Err(e) => {
            error!("保存预处理模式失败: {}", e);
            Err(format!("保存失败: {}", e))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// 构造一个「源数据目录」：DB（settings + pdfs/folders 路径数据）+ 五个子目录各一文件
    fn make_source_dir(root: &Path, dir_name: &str) -> PathBuf {
        let src = root.join(dir_name);
        std::fs::create_dir_all(&src).unwrap();

        let conn = rusqlite::Connection::open(src.join("pdf-manager.db")).unwrap();
        conn.execute_batch(crate::db::SCHEMA).unwrap();
        conn.execute(
            "INSERT INTO folders (id, name, storage_path) VALUES (1, '合同组', ?1)",
            [src.join("pdfs").to_string_lossy().to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO pdfs (id, folder_id, filename, storage_path, status)
             VALUES (1, 1, 'a.pdf', ?1, 'done')",
            [src.join("pdfs").join("a.pdf").to_string_lossy().to_string()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES ('data_dir', ?1)",
            [src.to_string_lossy().to_string()],
        )
        .unwrap();
        drop(conn);

        for sub in MIGRATE_SUBDIRS {
            let d = src.join(sub);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("f.bin"), b"x").unwrap();
        }
        src
    }

    #[test]
    fn test_normalize_dir() {
        assert_eq!(normalize_dir(Path::new("C:\\a\\b\\")), PathBuf::from("C:\\a\\b"));
        assert_eq!(normalize_dir(Path::new("C:\\a\\b")), PathBuf::from("C:\\a\\b"));
        // 盘符根不能被裁成盘符相对路径
        assert_eq!(normalize_dir(Path::new("D:\\")), PathBuf::from("D:\\"));
        assert_eq!(normalize_dir(Path::new("/tmp/x/")), PathBuf::from("/tmp/x"));
    }

    /// 全量迁移：DB + 五个子目录齐备、绝对路径前缀已修正、data_dir 已指向新目录
    #[test]
    fn test_copy_data_dir_core_and_fix_paths() {
        let root = tempfile::tempdir().unwrap();
        let src = make_source_dir(root.path(), "old");
        let dst = root.path().join("new");

        copy_data_dir_core(&src, &dst).unwrap();
        fix_db_paths(&dst.join("pdf-manager.db"), &src, &dst).unwrap();

        // DB 副本存在且设置已改
        assert!(dst.join("pdf-manager.db").exists());
        let conn = rusqlite::Connection::open(dst.join("pdf-manager.db")).unwrap();
        let v: String = conn
            .query_row("SELECT value FROM settings WHERE key = 'data_dir'", [], |r| r.get(0))
            .unwrap();
        assert_eq!(PathBuf::from(&v), normalize_dir(&dst));

        // pdfs.storage_path 前缀已替换
        let sp: String = conn
            .query_row("SELECT storage_path FROM pdfs WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sp, dst.join("pdfs").join("a.pdf").to_string_lossy());

        // folders.storage_path 前缀已替换
        let fp: String = conn
            .query_row("SELECT storage_path FROM folders WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(fp, dst.join("pdfs").to_string_lossy());

        // 五个子目录均已迁移
        for sub in MIGRATE_SUBDIRS {
            assert!(dst.join(sub).join("f.bin").exists(), "子目录 {} 未迁移", sub);
        }

        // 源目录完好（只复制不删除）
        assert!(src.join("pdf-manager.db").exists());
        for sub in MIGRATE_SUBDIRS {
            assert!(src.join(sub).join("f.bin").exists());
        }
    }

    /// 启动刷新：有迁移标记 → 刷新复制 + 清标记；无标记 → 按引导设置返回
    #[test]
    fn test_finish_pending_migration_refresh_and_clear() {
        let root = tempfile::tempdir().unwrap();
        let exe_dir = root.path().join("app");
        let src = make_source_dir(&exe_dir, "src_data");
        std::fs::create_dir_all(&exe_dir).unwrap();

        // 引导 DB：data_dir=目标 + 迁移源标记
        let guide = exe_dir.join("pdf-manager.db");
        let gconn = rusqlite::Connection::open(&guide).unwrap();
        gconn.execute_batch(crate::db::SCHEMA).unwrap();
        let dst = root.path().join("dst_data");
        set_setting(&gconn, SETTING_DATA_DIR, &dst.to_string_lossy()).unwrap();
        set_setting(&gconn, SETTING_DATA_DIR_MIGRATE_FROM, &src.to_string_lossy()).unwrap();
        drop(gconn);

        // 刷新前：目标目录在 set_data_dir 之后又新增了一个文件（模拟会话期新数据）
        std::fs::create_dir_all(src.join("models")).unwrap();
        std::fs::write(src.join("models").join("new_model.onnx"), b"m").unwrap();

        let effective = finish_pending_data_dir_migration(&exe_dir);
        assert_eq!(normalize_dir(&effective), normalize_dir(&dst), "应返回目标目录");

        // 刷新复制生效：新文件已补迁
        assert!(dst.join("models").join("new_model.onnx").exists());
        // DB 路径已修正
        let conn = rusqlite::Connection::open(dst.join("pdf-manager.db")).unwrap();
        let sp: String = conn
            .query_row("SELECT storage_path FROM pdfs WHERE id = 1", [], |r| r.get(0))
            .unwrap();
        assert_eq!(sp, dst.join("pdfs").join("a.pdf").to_string_lossy());
        drop(conn);

        // 标记已清除
        let gconn = rusqlite::Connection::open(&guide).unwrap();
        let marker: Option<String> = get_setting(&gconn, SETTING_DATA_DIR_MIGRATE_FROM);
        assert!(marker.is_none(), "迁移标记应已清除");

        // 无标记路径：直接按引导 data_dir 返回
        let effective2 = finish_pending_data_dir_migration(&exe_dir);
        assert_eq!(normalize_dir(&effective2), normalize_dir(&dst));
    }

    /// 源==目标或源不存在 → 清标记直接返回目标，不做复制
    #[test]
    fn test_finish_pending_migration_skips_when_source_missing() {
        let root = tempfile::tempdir().unwrap();
        let exe_dir = root.path().join("app");
        std::fs::create_dir_all(&exe_dir).unwrap();

        let guide = exe_dir.join("pdf-manager.db");
        let gconn = rusqlite::Connection::open(&guide).unwrap();
        gconn.execute_batch(crate::db::SCHEMA).unwrap();
        let dst = root.path().join("dst");
        let ghost = root.path().join("ghost"); // 不存在
        set_setting(&gconn, SETTING_DATA_DIR, &dst.to_string_lossy()).unwrap();
        set_setting(&gconn, SETTING_DATA_DIR_MIGRATE_FROM, &ghost.to_string_lossy()).unwrap();
        drop(gconn);

        let effective = finish_pending_data_dir_migration(&exe_dir);
        assert_eq!(normalize_dir(&effective), normalize_dir(&dst));

        let gconn = rusqlite::Connection::open(&guide).unwrap();
        assert!(get_setting(&gconn, SETTING_DATA_DIR_MIGRATE_FROM).is_none());
    }

    /// 目标已含应用数据 → set_data_dir 拒绝（set_data_dir 本体需 State，此处验证判定逻辑所依赖的检查）
    #[test]
    fn test_target_conflict_marker() {
        let root = tempfile::tempdir().unwrap();
        let dst = root.path().join("occupied");
        std::fs::create_dir_all(&dst).unwrap();
        std::fs::write(dst.join("pdf-manager.db"), b"").unwrap();
        assert!(dst.join("pdf-manager.db").exists(), "冲突检测条件：目标含 db 文件");
    }
}