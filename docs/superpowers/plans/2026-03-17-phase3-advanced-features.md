# 阶段3：高级功能 实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现文件夹关联物理目录和OCR手动触发功能

**Architecture:** 文件夹表添加 storage_path 字段存储物理路径，创建文件夹时可选物理目录。PDF列表显示OCR状态和进度，点击按钮触发异步OCR处理。

**Tech Stack:** Rust, Svelte, Tauri 2.0, tauri-plugin-dialog

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src-tauri/src/db/schema.rs` | 修改 | 添加 storage_path 字段 |
| `src-tauri/src/commands/folder.rs` | 修改 | 支持设置存储路径 |
| `src-tauri/src/commands/pdf.rs` | 修改 | 使用文件夹存储路径 |
| `src-tauri/src/commands/ocr.rs` | 修改 | 添加OCR触发命令 |
| `src-tauri/src/services/ocr_service.rs` | 修改 | 添加OCR处理逻辑 |
| `src-tauri/src/lib.rs` | 修改 | 注册新命令 |
| `src/lib/components/FolderTree.svelte` | 修改 | 添加选择目录功能 |
| `src/lib/components/PdfList.svelte` | 修改 | 显示OCR状态和触发按钮 |
| `src/lib/api/index.ts` | 修改 | 添加新API |
| `src/lib/stores/index.ts` | 修改 | 添加OCR进度状态 |

---

## Chunk 1: 文件夹关联物理目录

### Task 1: 数据库添加 storage_path 字段

**Files:**
- Modify: `src-tauri/src/db/schema.rs`

- [ ] **Step 1: 修改数据库 schema**

```rust
pub const SCHEMA: &str = r#"
-- 文件夹表
CREATE TABLE IF NOT EXISTS folders (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    parent_id INTEGER,
    storage_path TEXT,  -- 新增：物理存储路径
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

-- 迁移：添加 storage_path 字段（如果不存在）
ALTER TABLE folders ADD COLUMN storage_path TEXT;
"#;
```

- [ ] **Step 2: 在 db/mod.rs 中添加迁移逻辑**

在数据库初始化时添加迁移：

```rust
// 在 init_database 函数中，创建表之后添加：

// 迁移：添加 storage_path 字段
conn.execute(
    "ALTER TABLE folders ADD COLUMN storage_path TEXT",
    [],
).ok(); // 忽略错误（字段已存在）
```

- [ ] **Step 3: 更新 Folder 模型**

修改 `src-tauri/src/models/folder.rs`：

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Folder {
    pub id: i64,
    pub name: String,
    pub parent_id: Option<i64>,
    pub storage_path: Option<String>,  // 新增
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFolder {
    pub name: String,
    pub parent_id: Option<i64>,
    pub storage_path: Option<String>,  // 新增
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateFolder {
    pub name: String,
    pub storage_path: Option<String>,  // 新增
}
```

- [ ] **Step 4: 验证编译**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/db/schema.rs src-tauri/src/models/folder.rs
git commit -m "feat: add storage_path field to folders for physical directory association"
```

### Task 2: 更新文件夹命令支持存储路径

**Files:**
- Modify: `src-tauri/src/commands/folder.rs`

- [ ] **Step 1: 更新 create_folder 命令**

```rust
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
    storage_path: Option<String>,  // 新增参数
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
    storage_path: Option<String>,  // 新增参数
) -> Result<(), String> {
    info!("Renaming folder: id={}, name={}, storage_path={:?}", id, name, storage_path);

    let conn = db.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "UPDATE folders SET name = ?1, storage_path = ?2, updated_at = ?3 WHERE id = ?4",
        params![name, storage_path, now, id],
    )
    .map_err(|e| e.to_string())?;

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
```

- [ ] **Step 2: 更新 lib.rs 中的命令注册**

确保 `create_folder` 和 `rename_folder` 命令参数正确。

- [ ] **Step 3: 验证编译**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/folder.rs src-tauri/src/lib.rs
git commit -m "feat: update folder commands to support storage_path"
```

### Task 3: 前端支持选择存储目录

**Files:**
- Modify: `src/lib/api/index.ts`
- Modify: `src/lib/components/FolderTree.svelte`

- [ ] **Step 1: 更新 API 类型定义**

```typescript
export interface Folder {
  id: number;
  name: string;
  parent_id: number | null;
  storage_path: string | null;  // 新增
  created_at: string;
  updated_at: string;
}

export async function createFolder(name: string, parentId?: number, storagePath?: string): Promise<Folder> {
  return invoke('create_folder', { name, parentId, storagePath });
}

export async function renameFolder(id: number, name: string, storagePath?: string): Promise<void> {
  return invoke('rename_folder', { id, name, storagePath });
}
```

- [ ] **Step 2: 更新 FolderTree 组件**

添加选择目录功能：

```svelte
<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog';
  import { folders, selectedFolderId, isLoading } from '../stores';
  import { getFolders, createFolder, deleteFolder, renameFolder } from '../api';
  import { onMount } from 'svelte';
  import FolderNode from './FolderNode.svelte';
  import type { Folder } from '../api';

  // ... 现有代码 ...

  let selectedStoragePath: string | null = null;

  async function selectDirectory() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择存储目录',
    });

    if (selected) {
      selectedStoragePath = selected as string;
    }
  }

  async function handleCreate() {
    if (newFolderName.trim()) {
      try {
        await createFolder(
          newFolderName.trim(),
          newFolderParentId ?? undefined,
          selectedStoragePath ?? undefined
        );
        folders.set(await getFolders());
        newFolderName = '';
        showNewFolder = false;
        newFolderParentId = null;
        selectedStoragePath = null;
      } catch (e) {
        alert('创建失败: ' + e);
      }
    }
  }
</script>

<!-- 在新建文件夹区域添加 -->
{#if showNewFolder}
  <div class="new-folder">
    <input
      type="text"
      bind:value={newFolderName}
      placeholder="文件夹名称"
      on:keydown={handleKeydown}
    />
    <button class="path-btn" on:click={selectDirectory} title="选择存储目录">
      📁
    </button>
    <button on:click={handleCreate}>确定</button>
    {#if selectedStoragePath}
      <div class="selected-path">{selectedStoragePath}</div>
    {/if}
  </div>
{/if}
```

- [ ] **Step 3: 添加样式**

```css
.path-btn {
  padding: 5px 8px;
  border: 1px solid #ddd;
  background: #fff;
  cursor: pointer;
  border-radius: 4px;
}

.selected-path {
  font-size: 11px;
  color: #666;
  margin-top: 4px;
  word-break: break-all;
}
```

- [ ] **Step 4: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 5: Commit**

```bash
git add src/lib/api/index.ts src/lib/components/FolderTree.svelte
git commit -m "feat: add directory selection for folder storage path"
```

---

## Chunk 2: OCR手动触发

### Task 4: 添加 OCR 处理命令

**Files:**
- Modify: `src-tauri/src/commands/ocr.rs`

- [ ] **Step 1: 添加 OCR 处理命令**

```rust
use crate::db::Db;
use crate::services::ocr_service::OcrService;
use crate::services::pdf_service::PdfService;
use std::sync::Mutex;
use tauri::{Manager, State};
use tracing::{info, error};

#[derive(Clone, serde::Serialize)]
struct OcrProgress {
    pdf_id: i64,
    current: u32,
    total: u32,
    status: String,
}

/// 获取 OCR 状态
#[tauri::command]
pub fn get_ocr_status() -> Result<OcrStatusInfo, String> {
    // 检查 Tesseract 是否可用
    let available = which::which("tesseract").is_ok();

    let languages = if available {
        // 尝试获取可用语言
        std::process::Command::new("tesseract")
            .arg("--list-langs")
            .output()
            .ok()
            .and_then(|output| {
                String::from_utf8(output.stdout).ok()
            })
            .map(|s| {
                s.lines()
                    .skip(1)
                    .filter_map(|l| l.trim().split_whitespace().next())
                    .map(String::from)
                    .collect()
            })
            .unwrap_or_default()
    } else {
        vec![]
    };

    Ok(OcrStatusInfo { available, languages })
}

#[derive(serde::Serialize)]
pub struct OcrStatusInfo {
    available: bool,
    languages: Vec<String>,
}

/// 开始 OCR 处理
#[tauri::command]
pub async fn start_ocr(
    pdf_id: i64,
    db: State<'_, Db>,
    app_handle: tauri::AppHandle,
) -> Result<(), String> {
    info!("Starting OCR for PDF: {}", pdf_id);

    // 获取 PDF 信息
    let conn = db.lock().map_err(|e| e.to_string())?;
    let (storage_path, page_count): (String, i32) = conn
        .query_row(
            "SELECT storage_path, page_count FROM pdfs WHERE id = ?1",
            rusqlite::params![pdf_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(|e| e.to_string())?;

    // 更新状态为 processing
    conn.execute(
        "UPDATE pdfs SET status = 'processing', updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![pdf_id],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);

    // 发送进度事件
    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: 0,
        total: page_count as u32,
        status: "processing".to_string(),
    });

    // 初始化 OCR 服务
    let ocr_service = OcrService::new().map_err(|e| e.to_string())?;
    let pdf_service = PdfService::new().map_err(|e| e.to_string())?;

    // 处理每一页
    for page_num in 1..=page_count {
        match process_page(&pdf_id, &page_num, &storage_path, &ocr_service, &pdf_service, &db) {
            Ok(_) => {
                info!("Page {} processed successfully", page_num);
            }
            Err(e) => {
                error!("Failed to process page {}: {}", page_num, e);
            }
        }

        // 发送进度
        let _ = app_handle.emit("ocr-progress", OcrProgress {
            pdf_id,
            current: page_num as u32,
            total: page_count as u32,
            status: "processing".to_string(),
        });
    }

    // 更新状态为 done
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE pdfs SET status = 'done', updated_at = datetime('now') WHERE id = ?1",
        rusqlite::params![pdf_id],
    )
    .map_err(|e| e.to_string())?;
    drop(conn);

    let _ = app_handle.emit("ocr-progress", OcrProgress {
        pdf_id,
        current: page_count as u32,
        total: page_count as u32,
        status: "done".to_string(),
    });

    info!("OCR completed for PDF: {}", pdf_id);
    Ok(())
}

fn process_page(
    pdf_id: &i64,
    page_num: &i32,
    storage_path: &str,
    ocr_service: &OcrService,
    pdf_service: &PdfService,
    db: &State<'_, Db>,
) -> Result<(), String> {
    // 1. 渲染 PDF 页面为图像
    let image = pdf_service
        .render_page(storage_path, *page_num as u32)
        .map_err(|e| e.to_string())?;

    // 2. OCR 识别
    let text = ocr_service
        .recognize(&image)
        .map_err(|e| e.to_string())?;

    // 3. 保存到数据库
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO pdf_pages (pdf_id, page_number, ocr_text, ocr_status)
         VALUES (?1, ?2, ?3, 'done')",
        rusqlite::params![pdf_id, page_num, text],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}
```

- [ ] **Step 2: 在 lib.rs 中注册命令**

```rust
.invoke_handler(tauri::generate_handler![
    // ... 现有命令 ...
    commands::ocr::get_ocr_status,
    commands::ocr::start_ocr,
])
```

- [ ] **Step 3: 验证编译**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 可能需要实现一些缺失的方法

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/ocr.rs src-tauri/src/lib.rs
git commit -m "feat: add start_ocr command for manual OCR triggering"
```

### Task 5: 前端添加OCR触发按钮

**Files:**
- Modify: `src/lib/api/index.ts`
- Modify: `src/lib/stores/index.ts`
- Modify: `src/lib/components/PdfList.svelte`

- [ ] **Step 1: 添加 API**

```typescript
export async function startOcr(pdfId: number): Promise<void> {
  return invoke('start_ocr', { pdfId });
}
```

- [ ] **Step 2: 添加 OCR 进度状态**

```typescript
export interface OcrProgress {
  pdf_id: number;
  current: number;
  total: number;
  status: string;
}

export const ocrProgress = writable<Map<number, OcrProgress>>(new Map());
```

- [ ] **Step 3: 更新 PdfList 组件**

```svelte
<script lang="ts">
  import { pdfList, selectedPdfId, isLoading, selectedPdfPath, ocrProgress } from '../stores';
  import { getPdfList, addPdf, deletePdf, startOcr, getPdfDetail } from '../api';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';

  // ... 现有代码 ...

  onMount(async () => {
    await loadPdfs();

    // 监听 OCR 进度事件
    const unlisten = await listen<OcrProgress>('ocr-progress', (event) => {
      const progress = event.payload;
      ocrProgress.update(map => {
        map.set(progress.pdf_id, progress);
        return map;
      });

      if (progress.status === 'done') {
        loadPdfs();
      }
    });

    return unlisten;
  });

  async function handleStartOcr(id: number) {
    try {
      await startOcr(id);
    } catch (e) {
      alert('启动OCR失败: ' + e);
    }
  }

  // ... 其余代码 ...
</script>

<!-- 更新列表项 -->
{#each filteredPdfs as pdf}
  <li class:active={$selectedPdfId === pdf.id} on:click={() => selectPdf(pdf.id)}>
    <div class="info">
      <span class="filename">{pdf.filename}</span>
      <span class="meta">
        {pdf.page_count} 页 · {getTypeText(pdf.pdf_type)} · {getStatusText(pdf.status)}
        {#if $ocrProgress.has(pdf.id) && $ocrProgress.get(pdf.id)?.status === 'processing'}
          <span class="progress">
            ({$ocrProgress.get(pdf.id)?.current}/{$ocrProgress.get(pdf.id)?.total})
          </span>
        {/if}
      </span>
    </div>
    <div class="actions">
      {#if pdf.status === 'pending'}
        <button
          class="ocr-btn"
          on:click|stopPropagation={() => handleStartOcr(pdf.id)}
        >
          开始处理
        </button>
      {:else if pdf.status === 'processing'}
        <span class="processing">处理中...</span>
      {/if}
      <button class="delete-btn" on:click|stopPropagation={() => handleDelete(pdf.id)}>删除</button>
    </div>
  </li>
{/each}
```

- [ ] **Step 4: 添加样式**

```css
.actions {
  display: flex;
  gap: 8px;
  align-items: center;
}

.ocr-btn {
  padding: 5px 10px;
  background: #4caf50;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}

.processing {
  font-size: 12px;
  color: #ff9800;
}

.progress {
  color: #2196f3;
}
```

- [ ] **Step 5: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 6: Commit**

```bash
git add src/lib/api/index.ts src/lib/stores/index.ts src/lib/components/PdfList.svelte
git commit -m "feat: add OCR trigger button and progress display to PDF list"
```

---

## 验证步骤

### Task 6: 验证文件夹存储路径

- [ ] **Step 1: 测试创建带存储路径的文件夹**

1. 点击 "+" 创建文件夹
2. 点击文件夹图标选择物理目录
3. 输入名称并确定

Expected: 文件夹创建成功，存储路径被保存

### Task 7: 验证OCR手动触发

- [ ] **Step 1: 测试OCR触发**

1. 添加一个扫描版PDF
2. 在列表中找到该PDF
3. 点击"开始处理"按钮

Expected: 显示处理中状态和进度，完成后状态变为"已完成"

---

## 完成标准

- [ ] 文件夹可关联物理存储目录
- [ ] 创建文件夹时可选择存储目录
- [ ] PDF列表显示OCR状态
- [ ] 可手动触发OCR处理
- [ ] OCR进度实时显示
- [ ] 所有代码编译通过
- [ ] 所有改动已提交到 git