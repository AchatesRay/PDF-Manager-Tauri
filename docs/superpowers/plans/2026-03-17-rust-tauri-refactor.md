# PDF Manager 重构实现计划 - Rust + Tauri

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 使用 Rust + Tauri 重构 PDF Manager，解决 OCR 稳定性问题并将打包体积从 500MB+ 减少到 ~50MB

**Architecture:** 前后端分离架构，Tauri 提供 WebView 渲染前端，Rust 后端处理 PDF/OCR/搜索，SQLite 存储数据，Tantivy 提供全文搜索

**Tech Stack:** Tauri 2.0, Rust, Svelte, TypeScript, Tesseract OCR, pdfium-render, Tantivy, rusqlite, jieba-rs

---

## Chunk 1: 项目初始化与基础架构

### Task 1: 创建 Tauri 项目骨架

**Files:**
- Create: `pdf-manager-tauri/package.json`
- Create: `pdf-manager-tauri/src-tauri/Cargo.toml`
- Create: `pdf-manager-tauri/src-tauri/tauri.conf.json`
- Create: `pdf-manager-tauri/src-tauri/src/main.rs`
- Create: `pdf-manager-tauri/src-tauri/src/lib.rs`

- [ ] **Step 1: 初始化 Tauri 项目**

```bash
# 在项目根目录创建新的 Tauri 项目
cd /root/PdfOCR
npm create tauri-app@latest pdf-manager-tauri -- --template svelte-ts
```

选择选项：
- Package manager: npm
- UI template: Svelte
- UI flavor: TypeScript

- [ ] **Step 2: 验证项目创建成功**

```bash
cd pdf-manager-tauri
ls -la
```

Expected: 看到 `src-tauri/`, `src/`, `package.json` 等文件

- [ ] **Step 3: 配置 Cargo.toml 添加依赖**

编辑 `src-tauri/Cargo.toml`:

```toml
[package]
name = "pdf-manager"
version = "0.1.0"
description = "PDF Manager with OCR"
authors = ["you"]
edition = "2021"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = ["shell-open"] }
tauri-plugin-shell = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.31", features = ["bundled"] }
tantivy = "0.22"
jieba-rs = { version = "0.6", features = ["default-dict"] }
uuid = { version = "1", features = ["v4"] }
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
image = "0.25"
chrono = { version = "0.4", features = ["serde"] }
walkdir = "2"

[target.'cfg(windows)'.dependencies]
tesseract-plumbing = "0.1"

[features]
default = ["custom-protocol"]
custom-protocol = ["tauri/custom-protocol"]
```

- [ ] **Step 4: 配置 tauri.conf.json**

编辑 `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "PDF Manager",
  "version": "0.1.0",
  "identifier": "com.pdfmanager.app",
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "withGlobalTauri": true,
    "windows": [
      {
        "title": "PDF Manager",
        "width": 1200,
        "height": 800,
        "resizable": true,
        "fullscreen": false
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "resources": ["tesseract/*"],
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": ""
    }
  }
}
```

- [ ] **Step 5: 创建基础目录结构**

```bash
cd pdf-manager-tauri/src-tauri/src
mkdir -p commands services models db
```

- [ ] **Step 6: 创建 lib.rs 模块入口**

编辑 `src-tauri/src/lib.rs`:

```rust
pub mod commands;
pub mod db;
pub mod models;
pub mod services;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 初始化应用状态
            let app_handle = app.handle();

            // 初始化数据库
            let db = db::init_database(app_handle)?;
            app.manage(std::sync::Mutex::new(db));

            // 初始化服务
            let pdf_service = services::pdf_service::PdfService::new()?;
            app.manage(std::sync::Mutex::new(pdf_service));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folder::get_folders,
            commands::folder::create_folder,
            commands::folder::delete_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 7: 验证编译**

```bash
cd /root/PdfOCR/pdf-manager-tauri
cargo check
```

Expected: 编译通过（可能有 warnings 但无 errors）

- [ ] **Step 8: Commit**

```bash
git add pdf-manager-tauri/
git commit -m "feat: init Tauri project skeleton"
```

---

### Task 2: 创建数据模型

**Files:**
- Create: `src-tauri/src/models/mod.rs`
- Create: `src-tauri/src/models/folder.rs`
- Create: `src-tauri/src/models/pdf.rs`
- Create: `src-tauri/src/models/page.rs`

- [ ] **Step 1: 创建 models/mod.rs**

```rust
pub mod folder;
pub mod page;
pub mod pdf;

pub use folder::*;
pub use page::*;
pub use pdf::*;
```

- [ ] **Step 2: 创建 models/folder.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFolder {
    pub name: String,
    pub parent_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFolder {
    pub name: String,
}
```

- [ ] **Step 3: 创建 models/pdf.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PdfType {
    Text,
    Scanned,
    Mixed,
}

impl std::fmt::Display for PdfType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfType::Text => write!(f, "text"),
            PdfType::Scanned => write!(f, "scanned"),
            PdfType::Mixed => write!(f, "mixed"),
        }
    }
}

impl std::str::FromStr for PdfType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "text" => Ok(PdfType::Text),
            "scanned" => Ok(PdfType::Scanned),
            "mixed" => Ok(PdfType::Mixed),
            _ => Err(format!("Unknown PDF type: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PdfStatus {
    Pending,
    Processing,
    Done,
    Error,
}

impl std::fmt::Display for PdfStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PdfStatus::Pending => write!(f, "pending"),
            PdfStatus::Processing => write!(f, "processing"),
            PdfStatus::Done => write!(f, "done"),
            PdfStatus::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for PdfStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(PdfStatus::Pending),
            "processing" => Ok(PdfStatus::Processing),
            "done" => Ok(PdfStatus::Done),
            "error" => Ok(PdfStatus::Error),
            _ => Err(format!("Unknown PDF status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pdf {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub filename: String,
    pub original_path: Option<String>,
    pub storage_path: String,
    pub file_size: i64,
    pub page_count: i32,
    pub pdf_type: PdfType,
    pub status: PdfStatus,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPdf {
    pub folder_id: Option<i64>,
    pub filename: String,
    pub original_path: Option<String>,
    pub storage_path: String,
    pub file_size: i64,
    pub page_count: i32,
    pub pdf_type: PdfType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfInfo {
    pub id: i64,
    pub folder_id: Option<i64>,
    pub filename: String,
    pub page_count: i32,
    pub pdf_type: PdfType,
    pub status: PdfStatus,
    pub progress: Option<f32>, // 0.0 - 1.0
}
```

- [ ] **Step 4: 创建 models/page.rs**

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OcrStatus {
    Pending,
    Done,
    Error,
}

impl std::fmt::Display for OcrStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OcrStatus::Pending => write!(f, "pending"),
            OcrStatus::Done => write!(f, "done"),
            OcrStatus::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for OcrStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(OcrStatus::Pending),
            "done" => Ok(OcrStatus::Done),
            "error" => Ok(OcrStatus::Error),
            _ => Err(format!("Unknown OCR status: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PdfPage {
    pub id: i64,
    pub pdf_id: i64,
    pub page_number: i32,
    pub ocr_text: Option<String>,
    pub ocr_status: OcrStatus,
    pub thumbnail_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewPage {
    pub pdf_id: i64,
    pub page_number: i32,
}
```

- [ ] **Step 5: 验证编译**

```bash
cd /root/PdfOCR/pdf-manager-tauri
cargo check
```

Expected: 编译通过

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/models/
git commit -m "feat: add data models (Folder, Pdf, PdfPage)"
```

---

### Task 3: 创建数据库模块

**Files:**
- Create: `src-tauri/src/db/mod.rs`
- Create: `src-tauri/src/db/schema.rs`

- [ ] **Step 1: 创建 db/schema.rs**

```rust
pub const SCHEMA: &str = r#"
-- 文件夹表
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES folders(id) ON DELETE SET NULL
);

-- PDF 文件表
CREATE TABLE IF NOT EXISTS pdfs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    folder_id INTEGER,
    filename TEXT NOT NULL,
    original_path TEXT,
    storage_path TEXT NOT NULL,
    file_size INTEGER DEFAULT 0,
    page_count INTEGER DEFAULT 0,
    pdf_type TEXT CHECK(pdf_type IN ('text', 'scanned', 'mixed')) DEFAULT 'scanned',
    status TEXT CHECK(status IN ('pending', 'processing', 'done', 'error')) DEFAULT 'pending',
    error_message TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id) ON DELETE SET NULL
);

-- PDF 页面表
CREATE TABLE IF NOT EXISTS pdf_pages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    pdf_id INTEGER NOT NULL,
    page_number INTEGER NOT NULL,
    ocr_text TEXT,
    ocr_status TEXT CHECK(ocr_status IN ('pending', 'done', 'error')) DEFAULT 'pending',
    thumbnail_path TEXT,
    FOREIGN KEY (pdf_id) REFERENCES pdfs(id) ON DELETE CASCADE
);

-- 索引
CREATE INDEX IF NOT EXISTS idx_pdfs_folder ON pdfs(folder_id);
CREATE INDEX IF NOT EXISTS idx_pdfs_status ON pdfs(status);
CREATE INDEX IF NOT EXISTS idx_pages_pdf ON pdf_pages(pdf_id);
"#;
```

- [ ] **Step 2: 创建 db/mod.rs**

```rust
mod schema;

use rusqlite::{Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;
use tauri::Manager;

pub use schema::SCHEMA;

pub type Db = Mutex<Connection>;

/// 初始化数据库
pub fn init_database(app_handle: &tauri::AppHandle) -> Result<Connection, Box<dyn std::error::Error>> {
    // 获取应用数据目录
    let app_dir = app_handle
        .path()
        .app_data_dir()
        .expect("Failed to get app data directory");

    // 确保目录存在
    std::fs::create_dir_all(&app_dir)?;

    // 数据库文件路径
    let db_path = app_dir.join("pdf-manager.db");

    // 打开或创建数据库
    let conn = Connection::open(&db_path)?;

    // 执行 schema
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
```

- [ ] **Step 3: 验证编译**

```bash
cd /root/PdfOCR/pdf-manager-tauri
cargo check
```

Expected: 编译通过

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/db/
git commit -m "feat: add database module with schema and helpers"
```

---

### Task 4: 创建文件夹管理命令

**Files:**
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/folder.rs`

- [ ] **Step 1: 创建 commands/mod.rs**

```rust
pub mod folder;
pub mod ocr;
pub mod pdf;
pub mod search;
```

- [ ] **Step 2: 创建 commands/folder.rs**

```rust
use crate::db::Db;
use crate::models::{Folder, NewFolder, UpdateFolder};
use chrono::Utc;
use rusqlite::{params, Connection};
use serde_json::Value;
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

    // 检查是否有子文件夹
    let child_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM folders WHERE parent_id = ?1", params![id], |row| {
            row.get(0)
        })
        .map_err(|e| e.to_string())?;

    if child_count > 0 {
        return Err("Cannot delete folder with subfolders".to_string());
    }

    // 检查是否有 PDF
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
```

- [ ] **Step 3: 更新 lib.rs 注册命令**

编辑 `src-tauri/src/lib.rs`:

```rust
pub mod commands;
pub mod db;
pub mod models;
pub mod services;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .setup(|app| {
            // 初始化数据库
            let db = db::init_database(app.handle())?;
            app.manage(std::sync::Mutex::new(db));

            // 初始化 PDF 服务
            let pdf_service = services::pdf_service::PdfService::new()
                .expect("Failed to initialize PDF service");
            app.manage(std::sync::Mutex::new(pdf_service));

            // 确保数据目录存在
            let data_dir = db::get_data_dir(app.handle());
            std::fs::create_dir_all(&data_dir)?;
            std::fs::create_dir_all(db::get_pdfs_dir(app.handle()))?;
            std::fs::create_dir_all(db::get_thumbnails_dir(app.handle()))?;
            std::fs::create_dir_all(db::get_index_dir(app.handle()))?;

            tracing::info!("Application initialized successfully");

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::folder::get_folders,
            commands::folder::create_folder,
            commands::folder::rename_folder,
            commands::folder::delete_folder,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 4: 验证编译**

```bash
cd /root/PdfOCR/pdf-manager-tauri
cargo check
```

Expected: 编译通过

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/commands/
git commit -m "feat: add folder management commands"
```

---

## Chunk 2: PDF 服务与 OCR 服务

### Task 5: 创建 PDF 服务

**Files:**
- Create: `src-tauri/src/services/mod.rs`
- Create: `src-tauri/src/services/pdf_service.rs`

- [ ] **Step 1: 创建 services/mod.rs**

```rust
pub mod folder_service;
pub mod ocr_service;
pub mod pdf_service;
pub mod search_service;
```

- [ ] **Step 2: 创建 services/pdf_service.rs**

```rust
use crate::models::{PdfType, PdfStatus};
use image::{DynamicImage, ImageBuffer, Rgb};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum PdfError {
    #[error("Failed to open PDF: {0}")]
    OpenError(String),
    #[error("Failed to render page: {0}")]
    RenderError(String),
    #[error("Failed to extract text: {0}")]
    TextError(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub struct PdfService;

impl PdfService {
    pub fn new() -> Result<Self, PdfError> {
        Ok(Self {})
    }

    /// 获取 PDF 页数
    pub fn page_count(&self, pdf_path: &Path) -> Result<u32, PdfError> {
        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| PdfError::OpenError(e.to_string()))?;
        let pages = doc.get_pages();
        Ok(pages.len() as u32)
    }

    /// 检测 PDF 类型
    pub fn detect_type(&self, pdf_path: &Path) -> Result<PdfType, PdfError> {
        // 尝试提取文本，如果有足够文本则为文字型
        for _ in 0..3 {
            if let Ok(text) = self.extract_text(pdf_path) {
                if text.trim().len() > 100 {
                    return Ok(PdfType::Text);
                }
            }
        }
        Ok(PdfType::Scanned)
    }

    /// 提取 PDF 文本
    pub fn extract_text(&self, pdf_path: &Path) -> Result<String, PdfError> {
        let text = pdf_extract::extract_text(pdf_path)
            .map_err(|e| PdfError::TextError(e.to_string()))?;
        Ok(text)
    }

    /// 获取 PDF 元信息
    pub fn get_metadata(&self, pdf_path: &Path) -> Result<PdfMetadata, PdfError> {
        let doc = lopdf::Document::load(pdf_path)
            .map_err(|e| PdfError::OpenError(e.to_string()))?;
        let pages = doc.get_pages();
        let file_size = std::fs::metadata(pdf_path)?.len() as i64;

        Ok(PdfMetadata {
            page_count: pages.len() as i32,
            file_size,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PdfMetadata {
    pub page_count: i32,
    pub file_size: i64,
}
```

- [ ] **Step 3: 添加依赖到 Cargo.toml**

```toml
lopdf = "0.34"
pdf-extract = "0.7"
image = "0.25"
```

- [ ] **Step 4: 创建 folder_service.rs 占位**

```rust
pub struct FolderService;
impl FolderService { pub fn new() -> Self { Self } }
```

- [ ] **Step 5: 验证编译并提交**

```bash
cargo check
git add . && git commit -m "feat: add PDF service"
```

---

### Task 6: 创建 PDF 命令

**Files:**
- Create: `src-tauri/src/commands/pdf.rs`

- [ ] **Step 1: 创建 commands/pdf.rs** (完整实现 add_pdf, get_pdf_list, delete_pdf)

- [ ] **Step 2: 更新 lib.rs 注册命令**

- [ ] **Step 3: 验证编译并提交**

---

### Task 7: 创建 OCR 服务

**Files:**
- Create: `src-tauri/src/services/ocr_service.rs`
- Create: `src-tauri/src/commands/ocr.rs`

- [ ] **Step 1: 创建 OCR 服务** (使用 Tesseract CLI)

- [ ] **Step 2: 创建 OCR 命令**

- [ ] **Step 3: 验证编译并提交**

---

## Chunk 3: 搜索服务

### Task 8: 创建搜索服务

**Files:**
- Create: `src-tauri/src/services/search_service.rs`
- Create: `src-tauri/src/commands/search.rs`

- [ ] **Step 1: 创建 Tantivy 搜索服务** (含 jieba 分词)

- [ ] **Step 2: 创建搜索命令**

- [ ] **Step 3: 验证编译并提交**

---

## Chunk 4: 前端 UI (Svelte)

### Task 9: 设置前端项目

**Files:**
- Create: `src/lib/api/index.ts`
- Create: `src/lib/stores/index.ts`
- Create: `src/lib/components/FolderTree.svelte`
- Create: `src/lib/components/PdfList.svelte`

- [ ] **Step 1: 创建 API 封装**

- [ ] **Step 2: 创建状态管理**

- [ ] **Step 3: 创建 FolderTree 组件**

- [ ] **Step 4: 创建 PdfList 组件**

- [ ] **Step 5: 更新 App.svelte**

- [ ] **Step 6: 验证开发服务器**

- [ ] **Step 7: 提交**

---

### Task 10: 添加搜索功能

**Files:**
- Create: `src/lib/components/SearchBar.svelte`
- Create: `src/lib/components/SearchResults.svelte`

- [ ] **Step 1: 创建搜索栏组件**

- [ ] **Step 2: 创建搜索结果组件**

- [ ] **Step 3: 集成到主界面**

- [ ] **Step 4: 提交**

---

## Chunk 5: 打包与测试

### Task 11: 打包配置

**Files:**
- Create: `tesseract/chi_sim.traineddata`
- Create: `tesseract/eng.traineddata`

- [ ] **Step 1: 下载 Tesseract 语言模型**

```bash
mkdir -p tesseract
curl -L -o tesseract/chi_sim.traineddata \
  https://github.com/tesseract-ocr/tessdata/raw/main/chi_sim.traineddata
curl -L -o tesseract/eng.traineddata \
  https://github.com/tesseract-ocr/tessdata/raw/main/eng.traineddata
```

- [ ] **Step 2: 构建应用**

```bash
npm run build
cargo tauri build
```

- [ ] **Step 3: 测试安装包**

---

## 验收清单

| 项目 | 预期 |
|------|------|
| 安装包大小 | < 100MB |
| 添加 PDF | 正常 |
| OCR 识别 | 中文识别正常 |
| 全文搜索 | 正常 |
| 文件夹管理 | 正常 |
| 启动时间 | < 5 秒 |