# 阶段1：日志系统 + Bug修复 实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 添加日志系统，修复文件夹创建交互，排查PDF添加失败问题

**Architecture:** 使用 Rust tracing 框架实现日志系统，日志文件按日期滚动存储。前端添加回车键支持，后端添加详细日志便于排查问题。

**Tech Stack:** Rust (tracing, tracing-subscriber, tracing-appender), Svelte, TypeScript

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/Cargo.toml` | 修改 | 添加日志依赖 |
| `src-tauri/src/lib.rs` | 修改 | 初始化日志系统 |
| `src-tauri/src/commands/pdf.rs` | 修改 | 添加详细日志 |
| `src-tauri/src/commands/folder.rs` | 修改 | 添加详细日志 |
| `src-tauri/src/services/pdf_service.rs` | 修改 | 添加详细日志 |
| `src/lib/components/FolderTree.svelte` | 修改 | 支持回车创建 |
| `src/lib/components/PdfList.svelte` | 修改 | 改进错误处理 |

---

## Chunk 1: 日志系统

### Task 1: 添加日志依赖

**Files:**
- Modify: `src-tauri/Cargo.toml`

- [ ] **Step 1: 添加 tracing 相关依赖到 Cargo.toml**

在 `[dependencies]` 部分添加：

```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
```

- [ ] **Step 2: 验证依赖添加成功**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功，无错误

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "chore: add tracing dependencies for logging"
```

### Task 2: 初始化日志系统

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 添加日志初始化函数**

在 `lib.rs` 顶部添加导入：

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
```

在 `run()` 函数的 `setup` 闭包开始处添加日志初始化：

```rust
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // 初始化日志系统
            init_logging(app.handle());

            // 初始化数据库
            let db = db::init_database(app.handle())?;
            // ... 其余代码保持不变
        })
        // ... 其余代码保持不变
}

fn init_logging(app_handle: &tauri::AppHandle) {
    let log_dir = app_handle.path()
        .app_data_dir()
        .expect("Failed to get app data dir")
        .join("logs");

    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log directory: {}", e);
        return;
    }

    let file_appender = tracing_appender::rolling::daily(&log_dir, "pdf-ocr.log");

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(file_appender)
                .with_ansi(false)
        )
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
        )
        .init();

    tracing::info!("Logging initialized, log directory: {:?}", log_dir);
}
```

- [ ] **Step 2: 验证编译成功**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: initialize logging system with daily rolling files"
```

### Task 3: 为 PDF 命令添加日志

**Files:**
- Modify: `src-tauri/src/commands/pdf.rs`

- [ ] **Step 1: 在 add_pdf 函数中添加日志**

在文件顶部添加：

```rust
use tracing::{info, debug, error, warn};
```

修改 `add_pdf` 函数，添加日志：

```rust
#[tauri::command]
pub fn add_pdf(
    path: String,
    folder_id: Option<i64>,
    db: State<'_, Db>,
    pdf_service: State<'_, Mutex<PdfService>>,
    app_handle: tauri::AppHandle,
) -> Result<PdfInfo, String> {
    info!("Adding PDF: path={}, folder_id={:?}", path, folder_id);

    let src_path = PathBuf::from(&path);
    if !src_path.exists() {
        error!("File does not exist: {}", path);
        return Err("File does not exist".to_string());
    }
    debug!("Source file exists: {:?}", src_path);

    let filename = src_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown.pdf".to_string());
    debug!("Filename extracted: {}", filename);

    let storage_name = format!("{}.pdf", Uuid::new_v4());
    let storage_path = get_pdfs_dir(&app_handle).join(&storage_name);
    debug!("Storage path: {:?}", storage_path);

    std::fs::copy(&src_path, &storage_path)
        .map_err(|e| {
            error!("Failed to copy file: {}", e);
            format!("Failed to copy file: {}", e)
        })?;
    info!("File copied successfully");

    let pdf_svc = pdf_service.lock().map_err(|e| {
        error!("Failed to lock PDF service: {}", e);
        e.to_string()
    })?;

    let metadata = pdf_svc
        .get_metadata(&storage_path)
        .map_err(|e| {
            error!("Failed to get PDF metadata: {}", e);
            e.to_string()
        })?;
    debug!("PDF metadata: pages={}, size={}", metadata.page_count, metadata.file_size);

    let pdf_type = pdf_svc
        .detect_type(&storage_path)
        .map_err(|e| {
            error!("Failed to detect PDF type: {}", e);
            e.to_string()
        })?;
    debug!("PDF type detected: {:?}", pdf_type);
    drop(pdf_svc);

    let conn = db.lock().map_err(|e| {
        error!("Failed to lock database: {}", e);
        e.to_string()
    })?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO pdfs (folder_id, filename, original_path, storage_path, file_size, page_count, pdf_type, status, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            folder_id,
            filename,
            path,
            storage_path.to_string_lossy().to_string(),
            metadata.file_size,
            metadata.page_count,
            pdf_type.to_string(),
            "pending",
            now,
            now
        ],
    )
    .map_err(|e| {
        error!("Failed to insert PDF record: {}", e);
        e.to_string()
    })?;

    let id = conn.last_insert_rowid();
    drop(conn);

    info!("PDF added successfully: id={}, filename={}", id, filename);

    Ok(PdfInfo {
        id,
        folder_id,
        filename,
        page_count: metadata.page_count,
        pdf_type,
        status: PdfStatus::Pending,
        progress: Some(0.0),
    })
}
```

- [ ] **Step 2: 为其他函数添加日志**

在 `get_pdf_list`、`delete_pdf`、`get_pdf_detail` 函数开头添加 info 日志：

```rust
#[tauri::command]
pub fn get_pdf_list(
    folder_id: Option<i64>,
    db: State<'_, Db>,
) -> Result<Vec<PdfInfo>, String> {
    info!("Getting PDF list, folder_id={:?}", folder_id);
    // ... 现有代码
}

#[tauri::command]
pub fn delete_pdf(
    pdf_id: i64,
    db: State<'_, Db>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Deleting PDF: pdf_id={}", pdf_id);
    // ... 现有代码
}
```

- [ ] **Step 3: 验证编译成功**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/pdf.rs
git commit -m "feat: add detailed logging to PDF commands"
```

### Task 4: 为文件夹命令添加日志

**Files:**
- Modify: `src-tauri/src/commands/folder.rs`

- [ ] **Step 1: 添加日志导入和日志语句**

```rust
use tracing::{info, debug, error};

#[tauri::command]
pub fn get_folders(db: State<'_, Db>) -> Result<Vec<Folder>, String> {
    info!("Getting all folders");
    // ... 现有代码
    info!("Found {} folders", folders.len());
    Ok(folders)
}

#[tauri::command]
pub fn create_folder(db: State<'_, Db>, name: String, parent_id: Option<i64>) -> Result<Folder, String> {
    info!("Creating folder: name={}, parent_id={:?}", name, parent_id);
    // ... 现有代码
    info!("Folder created: id={}", id);
    Ok(Folder { ... })
}

#[tauri::command]
pub fn delete_folder(db: State<'_, Db>, id: i64) -> Result<(), String> {
    info!("Deleting folder: id={}", id);
    // ... 现有代码
    info!("Folder deleted: id={}", id);
    Ok(())
}
```

- [ ] **Step 2: 验证编译成功**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands/folder.rs
git commit -m "feat: add logging to folder commands"
```

---

## Chunk 2: 文件夹创建交互修复

### Task 5: 支持回车键创建文件夹

**Files:**
- Modify: `src/lib/components/FolderTree.svelte`

- [ ] **Step 1: 添加回车键事件处理**

找到 `<input>` 元素（约第59行），修改为：

```svelte
{#if showNewFolder}
  <div class="new-folder">
    <input
      type="text"
      bind:value={newFolderName}
      placeholder="文件夹名称"
      on:keydown={(e) => {
        if (e.key === 'Enter') {
          handleCreate();
        }
      }}
    />
    <button on:click={handleCreate}>确定</button>
  </div>
{/if}
```

- [ ] **Step 2: 验证前端编译成功**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功，无错误

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/FolderTree.svelte
git commit -m "fix: support Enter key to create folder"
```

---

## Chunk 3: PDF添加错误处理改进

### Task 6: 改进前端错误处理

**Files:**
- Modify: `src/lib/components/PdfList.svelte`

- [ ] **Step 1: 添加详细错误日志和控制台输出**

修改 `handleFileSelect` 函数：

```svelte
async function handleFileSelect(e: Event) {
  const input = e.target as HTMLInputElement;
  if (input.files) {
    console.log('Selected files:', input.files.length);
    for (const file of Array.from(input.files)) {
      try {
        console.log('Adding PDF:', file.path, 'to folder:', $selectedFolderId);
        const result = await addPdf(file.path, $selectedFolderId ?? undefined);
        console.log('PDF added successfully:', result);
      } catch (err) {
        console.error('Failed to add PDF:', err);
        alert('添加PDF失败: ' + err);
      }
    }
    await loadPdfs();
    // 重置 input 以允许再次选择相同文件
    input.value = '';
  }
}
```

- [ ] **Step 2: 添加空文件检查**

在函数开头添加检查：

```svelte
async function handleFileSelect(e: Event) {
  const input = e.target as HTMLInputElement;
  if (!input.files || input.files.length === 0) {
    console.log('No files selected');
    return;
  }
  // ... 其余代码
}
```

- [ ] **Step 3: 验证前端编译成功**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src/lib/components/PdfList.svelte
git commit -m "fix: improve PDF addition error handling with detailed logging"
```

### Task 7: 为 pdf_service 添加日志

**Files:**
- Modify: `src-tauri/src/services/pdf_service.rs`

- [ ] **Step 1: 添加日志导入**

在文件顶部添加：

```rust
use tracing::{info, debug, error, warn};
```

- [ ] **Step 2: 在关键函数中添加日志**

找到 `get_metadata` 和 `detect_type` 函数，添加日志：

```rust
pub fn get_metadata(&self, path: &Path) -> Result<PdfMetadata, PdfError> {
    debug!("Getting PDF metadata for: {:?}", path);

    // ... 现有代码

    debug!("PDF metadata retrieved: pages={}, size={}", metadata.page_count, metadata.file_size);
    Ok(metadata)
}

pub fn detect_type(&self, path: &Path) -> Result<PdfType, PdfError> {
    debug!("Detecting PDF type for: {:?}", path);

    // ... 现有代码

    debug!("PDF type detected: {:?}", pdf_type);
    Ok(pdf_type)
}
```

- [ ] **Step 3: 验证编译成功**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/services/pdf_service.rs
git commit -m "feat: add logging to PDF service"
```

---

## 验证步骤

### Task 8: 验证日志系统工作正常

- [ ] **Step 1: 构建并运行应用**

Run: `cd /root/PdfOCR && npm run tauri dev`
Expected: 应用启动成功

- [ ] **Step 2: 检查日志文件创建**

在应用运行后，检查日志目录：
- Windows: `%APPDATA%/pdf-manager-tauri/logs/`
- macOS: `~/Library/Application Support/pdf-manager-tauri/logs/`
- Linux: `~/.local/share/pdf-manager-tauri/logs/`

Expected: 存在 `pdf-ocr.log` 文件

- [ ] **Step 3: 验证日志内容**

执行以下操作并检查日志：
1. 创建文件夹
2. 添加PDF文件
3. 查看日志文件内容

Expected: 日志文件包含 INFO 级别的操作记录

### Task 9: 验证回车键创建文件夹

- [ ] **Step 1: 测试回车键功能**

1. 启动应用
2. 点击 "+" 按钮
3. 输入文件夹名称
4. 按回车键

Expected: 文件夹创建成功，出现在列表中

### Task 10: 验证PDF添加错误提示

- [ ] **Step 1: 测试错误处理**

1. 选择一个不存在或无权限的文件
2. 观察错误提示

Expected: 显示具体的错误信息

---

## 完成标准

- [ ] 日志文件在应用数据目录的 `logs/` 文件夹中正确生成
- [ ] 日志文件按日期滚动命名
- [ ] 回车键可创建文件夹
- [ ] PDF添加失败时显示具体错误信息
- [ ] 所有代码编译通过
- [ ] 所有改动已提交到 git