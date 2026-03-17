# 阶段2：布局重构 + 核心功能 实施计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 重构为三栏布局，添加PDF预览器，实现多级子文件夹和统一搜索框

**Architecture:** 左栏文件夹树（可折叠）、中栏搜索+PDF列表、右栏PDF预览器。使用 pdf.js 渲染PDF，递归组件显示子文件夹，统一搜索框支持内容/文件名切换。

**Tech Stack:** Svelte 4, TypeScript, pdf.js, Tauri 2.0

---

## 文件结构

| 文件 | 操作 | 职责 |
|------|------|------|
| `src/App.svelte` | 修改 | 三栏布局 |
| `src/lib/components/PdfViewer.svelte` | 新建 | PDF预览组件 |
| `src/lib/components/FolderTree.svelte` | 修改 | 多级子文件夹、递归渲染 |
| `src/lib/components/FolderNode.svelte` | 新建 | 文件夹节点组件（递归） |
| `src/lib/components/SearchBar.svelte` | 修改 | 统一搜索框 |
| `src/lib/components/PdfList.svelte` | 修改 | 选中状态样式 |
| `src/lib/stores/index.ts` | 修改 | 添加预览相关状态 |
| `src/lib/api/index.ts` | 修改 | 添加文件名搜索API |
| `src-tauri/src/commands/search.rs` | 修改 | 添加文件名搜索命令 |
| `src-tauri/src/services/search_service.rs` | 修改 | 添加文件名搜索逻辑 |
| `package.json` | 修改 | 添加 pdfjs-dist 依赖 |

---

## Chunk 1: PDF预览器基础

### Task 1: 添加 pdf.js 依赖

**Files:**
- Modify: `package.json`

- [ ] **Step 1: 添加 pdfjs-dist 依赖**

Run: `cd /root/PdfOCR && npm install pdfjs-dist@3.11.174`
Expected: 安装成功

- [ ] **Step 2: Commit**

```bash
git add package.json package-lock.json
git commit -m "chore: add pdfjs-dist dependency for PDF viewer"
```

### Task 2: 创建 PDF 预览组件

**Files:**
- Create: `src/lib/components/PdfViewer.svelte`

- [ ] **Step 1: 创建基础组件结构**

```svelte
<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as pdfjsLib from 'pdfjs-dist';

  // 设置 worker
  pdfjsLib.GlobalWorkerOptions.workerSrc = 'https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.worker.min.js';

  export let pdfPath: string | null = null;

  let canvas: HTMLCanvasElement;
  let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null;
  let currentPage = 1;
  let totalPages = 0;
  let scale = 1.2;
  let isLoading = false;
  let error: string | null = null;

  $: if (pdfPath) {
    loadPdf(pdfPath);
  }

  async function loadPdf(path: string) {
    isLoading = true;
    error = null;

    try {
      const loadingTask = pdfjsLib.getDocument(path);
      pdfDoc = await loadingTask.promise;
      totalPages = pdfDoc.numPages;
      currentPage = 1;
      await renderPage(currentPage);
    } catch (e) {
      error = '加载PDF失败: ' + e;
      console.error('Failed to load PDF:', e);
    } finally {
      isLoading = false;
    }
  }

  async function renderPage(pageNum: number) {
    if (!pdfDoc || !canvas) return;

    const page = await pdfDoc.getPage(pageNum);
    const viewport = page.getViewport({ scale });

    canvas.height = viewport.height;
    canvas.width = viewport.width;

    const context = canvas.getContext('2d');
    if (!context) return;

    const renderContext = {
      canvasContext: context,
      viewport: viewport
    };

    await page.render(renderContext).promise;
  }

  function prevPage() {
    if (currentPage > 1) {
      currentPage--;
      renderPage(currentPage);
    }
  }

  function nextPage() {
    if (currentPage < totalPages) {
      currentPage++;
      renderPage(currentPage);
    }
  }

  function zoomIn() {
    scale = Math.min(scale + 0.2, 3);
    renderPage(currentPage);
  }

  function zoomOut() {
    scale = Math.max(scale - 0.2, 0.5);
    renderPage(currentPage);
  }

  onDestroy(() => {
    pdfDoc?.destroy();
  });
</script>

<div class="pdf-viewer">
  {#if !pdfPath}
    <div class="placeholder">
      <p>选择PDF文件预览</p>
    </div>
  {:else if isLoading}
    <div class="loading">
      <p>加载中...</p>
    </div>
  {:else if error}
    <div class="error">
      <p>{error}</p>
    </div>
  {:else}
    <div class="toolbar">
      <button on:click={prevPage} disabled={currentPage <= 1}>上一页</button>
      <span>{currentPage} / {totalPages}</span>
      <button on:click={nextPage} disabled={currentPage >= totalPages}>下一页</button>
      <div class="spacer"></div>
      <button on:click={zoomOut} disabled={scale <= 0.5}>-</button>
      <span>{Math.round(scale * 100)}%</span>
      <button on:click={zoomIn} disabled={scale >= 3}>+</button>
    </div>
    <div class="canvas-container">
      <canvas bind:this={canvas}></canvas>
    </div>
  {/if}
</div>

<style>
  .pdf-viewer {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: #f0f0f0;
  }

  .placeholder, .loading, .error {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
  }

  .error {
    color: #f44336;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    background: #fff;
    border-bottom: 1px solid #ddd;
  }

  .toolbar button {
    padding: 5px 10px;
    border: 1px solid #ddd;
    background: #fff;
    cursor: pointer;
    border-radius: 4px;
  }

  .toolbar button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .spacer {
    flex: 1;
  }

  .canvas-container {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    padding: 10px;
  }

  canvas {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }
</style>
```

- [ ] **Step 2: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/PdfViewer.svelte
git commit -m "feat: create PDF viewer component with navigation and zoom"
```

---

## Chunk 2: 三栏布局重构

### Task 3: 添加 PDF 路径状态

**Files:**
- Modify: `src/lib/stores/index.ts`

- [ ] **Step 1: 添加选中的 PDF 路径状态**

```typescript
import { writable } from 'svelte/store';

export interface Folder {
  id: number;
  name: string;
  parent_id: number | null;
  created_at: string;
  updated_at: string;
}

export interface PdfInfo {
  id: number;
  folder_id: number | null;
  filename: string;
  page_count: number;
  pdf_type: 'text' | 'scanned' | 'mixed';
  status: 'pending' | 'processing' | 'done' | 'error';
  progress?: number;
}

export interface SearchResult {
  page_id: number;
  pdf_id: number;
  folder_id: number | null;
  page_number: number;
  filename: string;
  score: number;
  snippet: string;
}

// 状态
export const folders = writable<Folder[]>([]);
export const selectedFolderId = writable<number | null>(null);
export const pdfList = writable<PdfInfo[]>([]);
export const selectedPdfId = writable<number | null>(null);
export const selectedPdfPath = writable<string | null>(null); // 新增：选中的PDF路径
export const searchQuery = writable('');
export const searchResults = writable<SearchResult[]>([]);
export const isLoading = writable(false);
export const showSearchResults = writable(false);
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/stores/index.ts
git commit -m "feat: add selectedPdfPath store for PDF preview"
```

### Task 4: 添加获取 PDF 详情 API

**Files:**
- Modify: `src/lib/api/index.ts`

- [ ] **Step 1: 添加 get_pdf_detail 函数**

```typescript
// 在文件末尾添加

export interface PdfDetail {
  id: number;
  folder_id: number | null;
  filename: string;
  original_path: string;
  storage_path: string;
  file_size: number;
  page_count: number;
  pdf_type: 'text' | 'scanned' | 'mixed';
  status: 'pending' | 'processing' | 'done' | 'error';
  error_message: string | null;
  created_at: string;
  updated_at: string;
}

export async function getPdfDetail(pdfId: number): Promise<PdfDetail> {
  return invoke('get_pdf_detail', { pdfId });
}
```

- [ ] **Step 2: Commit**

```bash
git add src/lib/api/index.ts
git commit -m "feat: add getPdfDetail API for fetching PDF storage path"
```

### Task 5: 重构主应用为三栏布局

**Files:**
- Modify: `src/App.svelte`

- [ ] **Step 1: 更新主应用布局**

```svelte
<script lang="ts">
  import FolderTree from './lib/components/FolderTree.svelte';
  import PdfList from './lib/components/PdfList.svelte';
  import SearchBar from './lib/components/SearchBar.svelte';
  import SearchResults from './lib/components/SearchResults.svelte';
  import PdfViewer from './lib/components/PdfViewer.svelte';
  import { selectedPdfPath } from './lib/stores';
</script>

<main>
  <div class="left-panel">
    <FolderTree />
  </div>
  <div class="center-panel">
    <SearchBar />
    <PdfList />
    <SearchResults />
  </div>
  <div class="right-panel">
    <PdfViewer pdfPath={$selectedPdfPath} />
  </div>
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .left-panel {
    width: 220px;
    min-width: 180px;
    border-right: 1px solid #ddd;
    overflow: hidden;
  }

  .center-panel {
    flex: 1;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .right-panel {
    width: 400px;
    min-width: 300px;
    border-left: 1px solid #ddd;
    overflow: hidden;
  }
</style>
```

- [ ] **Step 2: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src/App.svelte
git commit -m "feat: refactor to three-column layout"
```

### Task 6: 更新 PdfList 组件支持选中预览

**Files:**
- Modify: `src/lib/components/PdfList.svelte`

- [ ] **Step 1: 导入状态和 API**

在 script 标签内添加：

```typescript
import { selectedPdfId, selectedPdfPath } from '../stores';
import { getPdfDetail } from '../api';
```

- [ ] **Step 2: 修改 selectPdf 函数**

```typescript
async function selectPdf(id: number) {
  selectedPdfId.set(id);
  try {
    const detail = await getPdfDetail(id);
    selectedPdfPath.set(detail.storage_path);
  } catch (e) {
    console.error('Failed to get PDF detail:', e);
    selectedPdfPath.set(null);
  }
}
```

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/PdfList.svelte
git commit -m "feat: load PDF path on selection for preview"
```

---

## Chunk 3: 多级子文件夹

### Task 7: 创建文件夹节点组件

**Files:**
- Create: `src/lib/components/FolderNode.svelte`

- [ ] **Step 1: 创建递归文件夹节点组件**

```svelte
<script lang="ts">
  import { selectedFolderId } from '../stores';
  import type { Folder } from '../api';

  export let folder: Folder & { children?: (Folder & { children?: Folder[] })[] };
  export let level = 0;
  export let onDelete: (id: number) => void;

  let isExpanded = false;
  let hasChildren = folder.children && folder.children.length > 0;

  function toggleExpand(e: MouseEvent) {
    e.stopPropagation();
    isExpanded = !isExpanded;
  }

  function selectFolder() {
    selectedFolderId.set(folder.id);
  }
</script>

<li
  class:active={$selectedFolderId === folder.id}
  style="padding-left: {12 + level * 16}px"
  on:click={selectFolder}
>
  {#if hasChildren}
    <button class="expand-btn" on:click={toggleExpand}>
      {isExpanded ? '▼' : '▶'}
    </button>
  {:else}
    <span class="expand-placeholder"></span>
  {/if}
  <span class="folder-icon">📁</span>
  <span class="name">{folder.name}</span>
  <button class="delete-btn" on:click|stopPropagation={() => onDelete(folder.id)}>×</button>
</li>

{#if hasChildren && isExpanded}
  {#each folder.children || [] as child}
    <svelte:self
      folder={child}
      level={level + 1}
      {onDelete}
    />
  {/each}
{/if}

<style>
  li {
    display: flex;
    align-items: center;
    padding: 8px 12px;
    cursor: pointer;
    border-radius: 4px;
    margin-bottom: 2px;
    gap: 4px;
  }

  li:hover {
    background: #e0e0e0;
  }

  li.active {
    background: #bbdefb;
  }

  .expand-btn {
    width: 16px;
    height: 16px;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 10px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .expand-placeholder {
    width: 16px;
  }

  .folder-icon {
    font-size: 14px;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .delete-btn {
    opacity: 0;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 16px;
    color: #f44336;
  }

  li:hover .delete-btn {
    opacity: 1;
  }
</style>
```

- [ ] **Step 2: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/FolderNode.svelte
git commit -m "feat: create recursive FolderNode component for nested folders"
```

### Task 8: 更新 FolderTree 使用新组件

**Files:**
- Modify: `src/lib/components/FolderTree.svelte`

- [ ] **Step 1: 重构 FolderTree 组件**

```svelte
<script lang="ts">
  import { folders, selectedFolderId, isLoading } from '../stores';
  import { getFolders, createFolder, deleteFolder } from '../api';
  import { onMount } from 'svelte';
  import FolderNode from './FolderNode.svelte';
  import type { Folder } from '../api';

  interface TreeNode extends Folder {
    children: TreeNode[];
  }

  let newFolderName = '';
  let showNewFolder = false;
  let newFolderParentId: number | null = null;

  $: treeNodes = buildTree($folders);

  onMount(async () => {
    try {
      folders.set(await getFolders());
    } catch (e) {
      console.error('Failed to load folders:', e);
    }
  });

  function buildTree(folderList: Folder[]): TreeNode[] {
    const map = new Map<number, TreeNode>();
    const roots: TreeNode[] = [];

    // 初始化所有节点
    folderList.forEach(f => {
      map.set(f.id, { ...f, children: [] });
    });

    // 构建树形结构
    folderList.forEach(f => {
      const node = map.get(f.id)!;
      if (f.parent_id && map.has(f.parent_id)) {
        map.get(f.parent_id)!.children.push(node);
      } else {
        roots.push(node);
      }
    });

    // 按名称排序
    const sortChildren = (nodes: TreeNode[]) => {
      nodes.sort((a, b) => a.name.localeCompare(b.name, 'zh'));
      nodes.forEach(n => sortChildren(n.children));
    };
    sortChildren(roots);

    return roots;
  }

  async function handleCreate() {
    if (newFolderName.trim()) {
      try {
        await createFolder(newFolderName.trim(), newFolderParentId ?? undefined);
        folders.set(await getFolders());
        newFolderName = '';
        showNewFolder = false;
        newFolderParentId = null;
      } catch (e) {
        alert('创建失败: ' + e);
      }
    }
  }

  async function handleDelete(id: number) {
    if (confirm('确定删除此文件夹？')) {
      try {
        await deleteFolder(id);
        folders.set(await getFolders());
        if ($selectedFolderId === id) {
          selectedFolderId.set(null);
        }
      } catch (e) {
        alert('删除失败: ' + e);
      }
    }
  }

  function selectFolder(id: number | null) {
    selectedFolderId.set(id);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleCreate();
    }
  }
</script>

<div class="folder-tree">
  <div class="header">
    <h3>文件夹</h3>
    <button on:click={() => showNewFolder = !showNewFolder}>+</button>
  </div>

  {#if showNewFolder}
    <div class="new-folder">
      <input
        type="text"
        bind:value={newFolderName}
        placeholder="文件夹名称"
        on:keydown={handleKeydown}
      />
      <button on:click={handleCreate}>确定</button>
    </div>
  {/if}

  <ul class="folder-list">
    <li
      class:active={$selectedFolderId === null}
      on:click={() => selectFolder(null)}
    >
      <span class="folder-icon">📚</span>
      <span class="name">全部文件</span>
    </li>
    {#each treeNodes as node}
      <FolderNode {node} level={0} onDelete={handleDelete} />
    {/each}
  </ul>
</div>

<style>
  .folder-tree {
    width: 100%;
    height: 100%;
    padding: 10px;
    overflow-y: auto;
    background: #fafafa;
    display: flex;
    flex-direction: column;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    flex-shrink: 0;
  }

  .header h3 {
    font-size: 14px;
    color: #333;
    margin: 0;
  }

  .header button {
    width: 24px;
    height: 24px;
    border: none;
    background: #2196f3;
    color: white;
    border-radius: 4px;
    cursor: pointer;
  }

  .folder-list {
    list-style: none;
    padding: 0;
    margin: 0;
    flex: 1;
    overflow-y: auto;
  }

  .folder-list > li {
    padding: 8px 12px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 2px;
  }

  .folder-list > li:hover {
    background: #e0e0e0;
  }

  .folder-list > li.active {
    background: #bbdefb;
  }

  .folder-icon {
    font-size: 14px;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .new-folder {
    display: flex;
    gap: 5px;
    margin-bottom: 10px;
    flex-shrink: 0;
  }

  .new-folder input {
    flex: 1;
    padding: 5px;
    border: 1px solid #ddd;
    border-radius: 4px;
  }

  .new-folder button {
    padding: 5px 10px;
    background: #4caf50;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }
</style>
```

- [ ] **Step 2: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 3: Commit**

```bash
git add src/lib/components/FolderTree.svelte
git commit -m "feat: update FolderTree to use recursive FolderNode component"
```

---

## Chunk 4: 统一搜索框

### Task 9: 后端添加文件名搜索命令

**Files:**
- Modify: `src-tauri/src/commands/search.rs`

- [ ] **Step 1: 添加文件名搜索命令**

```rust
use crate::db::Db;
use crate::models::PdfInfo;
use crate::services::search_service::{SearchResult, SearchService};
use rusqlite::params;
use std::sync::Mutex;
use tauri::State;
use tracing::info;

/// 搜索PDF内容
#[tauri::command]
pub fn search(
    query: String,
    folder_id: Option<i64>,
    search_service: State<'_, Mutex<SearchService>>,
) -> Result<Vec<SearchResult>, String> {
    info!("Searching content: query={}, folder_id={:?}", query, folder_id);
    let svc = search_service.lock().map_err(|e| e.to_string())?;
    svc.search(&query, folder_id, 50).map_err(|e| e.to_string())
}

/// 搜索PDF文件名
#[tauri::command]
pub fn search_filename(
    query: String,
    folder_id: Option<i64>,
    db: State<'_, Db>,
) -> Result<Vec<PdfInfo>, String> {
    info!("Searching filename: query={}, folder_id={:?}", query, folder_id);

    let conn = db.lock().map_err(|e| e.to_string())?;

    let search_pattern = format!("%{}%", query);

    let sql = match folder_id {
        Some(_) => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE filename LIKE ?1 AND folder_id = ?2 ORDER BY created_at DESC",
        None => "SELECT id, folder_id, filename, page_count, pdf_type, status FROM pdfs WHERE filename LIKE ?1 ORDER BY created_at DESC",
    };

    let mut stmt = conn.prepare(sql).map_err(|e| e.to_string())?;

    fn row_to_pdf_info(row: &rusqlite::Row) -> rusqlite::Result<PdfInfo> {
        use crate::models::{PdfStatus, PdfType};
        let status_str: String = row.get(5)?;
        let pdf_type_str: String = row.get(4)?;
        Ok(PdfInfo {
            id: row.get(0)?,
            folder_id: row.get(1)?,
            filename: row.get(2)?,
            page_count: row.get(3)?,
            pdf_type: pdf_type_str.parse().unwrap_or(PdfType::Scanned),
            status: status_str.parse().unwrap_or(PdfStatus::Pending),
            progress: None,
        })
    }

    let pdfs = if let Some(fid) = folder_id {
        stmt.query_map(params![search_pattern, fid], row_to_pdf_info)
    } else {
        stmt.query_map(params![search_pattern], row_to_pdf_info)
    }
    .map_err(|e| e.to_string())?
    .collect::<Result<Vec<_>, _>>()
    .map_err(|e| e.to_string())?;

    info!("Found {} PDFs matching filename", pdfs.len());
    Ok(pdfs)
}
```

- [ ] **Step 2: 在 lib.rs 中注册新命令**

修改 `src-tauri/src/lib.rs`，在 `invoke_handler` 中添加：

```rust
.invoke_handler(tauri::generate_handler![
    commands::folder::get_folders,
    commands::folder::create_folder,
    commands::folder::rename_folder,
    commands::folder::delete_folder,
    commands::pdf::add_pdf,
    commands::pdf::get_pdf_list,
    commands::pdf::delete_pdf,
    commands::pdf::get_pdf_detail,
    commands::search::search,
    commands::search::search_filename,  // 新增
    commands::ocr::get_ocr_status,
])
```

- [ ] **Step 3: 验证编译**

Run: `cd /root/PdfOCR/src-tauri && cargo check`
Expected: 编译成功

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/commands/search.rs src-tauri/src/lib.rs
git commit -m "feat: add search_filename command for filename search"
```

### Task 10: 前端添加统一搜索框

**Files:**
- Modify: `src/lib/api/index.ts`
- Modify: `src/lib/stores/index.ts`
- Modify: `src/lib/components/SearchBar.svelte`

- [ ] **Step 1: 添加 API 函数**

在 `src/lib/api/index.ts` 中添加：

```typescript
// 文件名搜索
export async function searchFilename(query: string, folderId?: number): Promise<PdfInfo[]> {
  return invoke('search_filename', { query, folderId });
}
```

- [ ] **Step 2: 添加搜索模式状态**

在 `src/lib/stores/index.ts` 中添加：

```typescript
export type SearchMode = 'content' | 'filename';
export const searchMode = writable<SearchMode>('content');
export const filenameSearchResults = writable<PdfInfo[]>([]);
```

- [ ] **Step 3: 重构 SearchBar 组件**

```svelte
<script lang="ts">
  import { searchQuery, searchResults, isLoading, showSearchResults, searchMode, filenameSearchResults, selectedPdfId, selectedPdfPath } from '../stores';
  import { search, searchFilename, getPdfDetail } from '../api';

  async function handleSearch() {
    if ($searchQuery.trim()) {
      isLoading.set(true);
      try {
        if ($searchMode === 'content') {
          searchResults.set(await search($searchQuery.trim()));
          filenameSearchResults.set([]);
        } else {
          filenameSearchResults.set(await searchFilename($searchQuery.trim()));
          searchResults.set([]);
        }
        showSearchResults.set(true);
      } catch (e) {
        console.error('Search failed:', e);
        alert('搜索失败: ' + e);
      } finally {
        isLoading.set(false);
      }
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleSearch();
    }
  }

  function clearSearch() {
    searchQuery.set('');
    searchResults.set([]);
    filenameSearchResults.set([]);
    showSearchResults.set(false);
  }
</script>

<div class="search-bar">
  <select bind:value={$searchMode} class="mode-select">
    <option value="content">内容</option>
    <option value="filename">文件名</option>
  </select>
  <input
    type="text"
    bind:value={$searchQuery}
    placeholder="搜索..."
    on:keydown={handleKeydown}
  />
  <button on:click={handleSearch} disabled={$isLoading}>
    {$isLoading ? '搜索中...' : '搜索'}
  </button>
  {#if $showSearchResults}
    <button class="clear-btn" on:click={clearSearch}>清除</button>
  {/if}
</div>

<style>
  .search-bar {
    display: flex;
    gap: 8px;
    padding: 10px;
    background: #f5f5f5;
    border-bottom: 1px solid #ddd;
    flex-shrink: 0;
  }

  .mode-select {
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 4px;
    background: #fff;
    cursor: pointer;
  }

  input {
    flex: 1;
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 14px;
  }

  input:focus {
    outline: none;
    border-color: #2196f3;
  }

  button {
    padding: 10px 20px;
    background: #2196f3;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  button:disabled {
    background: #ccc;
    cursor: not-allowed;
  }

  .clear-btn {
    background: #757575;
  }
</style>
```

- [ ] **Step 4: 更新 SearchResults 组件支持文件名搜索结果**

修改 `src/lib/components/SearchResults.svelte`：

```svelte
<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  async function handleContentResultClick(result: SearchResult) {
    selectedPdfId.set(result.pdf_id);
    showSearchResults.set(false);
    try {
      const detail = await getPdfDetail(result.pdf_id);
      selectedPdfPath.set(detail.storage_path);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
    }
  }

  async function handleFilenameResultClick(pdf: PdfInfo) {
    selectedPdfId.set(pdf.id);
    showSearchResults.set(false);
    try {
      const detail = await getPdfDetail(pdf.id);
      selectedPdfPath.set(detail.storage_path);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
    }
  }
</script>

{#if $showSearchResults}
  {#if $searchMode === 'content'}
    {#if $searchResults.length > 0}
      <div class="search-results">
        <h4>内容搜索结果 ({$searchResults.length})</h4>
        <ul>
          {#each $searchResults as result}
            <li on:click={() => handleContentResultClick(result)}>
              <div class="filename">{result.filename}</div>
              <div class="page">第 {result.page_number} 页</div>
              <div class="snippet">{result.snippet}</div>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-results">
        未找到匹配的内容
      </div>
    {/if}
  {:else}
    {#if $filenameSearchResults.length > 0}
      <div class="search-results">
        <h4>文件名搜索结果 ({$filenameSearchResults.length})</h4>
        <ul>
          {#each $filenameSearchResults as pdf}
            <li on:click={() => handleFilenameResultClick(pdf)}>
              <div class="filename">{pdf.filename}</div>
              <div class="meta">{pdf.page_count} 页</div>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-results">
        未找到匹配的文件名
      </div>
    {/if}
  {/if}
{/if}

<style>
  .search-results {
    padding: 10px;
    background: #fff;
    border-top: 1px solid #ddd;
    max-height: 300px;
    overflow-y: auto;
    flex-shrink: 0;
  }

  h4 {
    margin: 0 0 10px 0;
    font-size: 14px;
    color: #333;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  li {
    padding: 10px;
    cursor: pointer;
    border-radius: 4px;
    margin-bottom: 5px;
    background: #f9f9f9;
  }

  li:hover {
    background: #f0f0f0;
  }

  .filename {
    font-weight: 500;
    margin-bottom: 4px;
  }

  .page, .meta {
    font-size: 12px;
    color: #2196f3;
    margin-bottom: 4px;
  }

  .snippet {
    font-size: 13px;
    color: #666;
    line-height: 1.4;
  }

  .no-results {
    padding: 20px;
    text-align: center;
    color: #666;
    background: #fff;
    border-top: 1px solid #ddd;
    flex-shrink: 0;
  }
</style>
```

- [ ] **Step 5: 验证编译**

Run: `cd /root/PdfOCR && npm run build`
Expected: 编译成功

- [ ] **Step 6: Commit**

```bash
git add src/lib/api/index.ts src/lib/stores/index.ts src/lib/components/SearchBar.svelte src/lib/components/SearchResults.svelte
git commit -m "feat: add unified search with content/filename mode toggle"
```

---

## 验证步骤

### Task 11: 验证三栏布局

- [ ] **Step 1: 启动应用**

Run: `cd /root/PdfOCR && npm run tauri dev`
Expected: 应用启动成功，显示三栏布局

- [ ] **Step 2: 验证布局**

Expected:
- 左栏显示文件夹树
- 中栏显示搜索框和PDF列表
- 右栏显示PDF预览占位符

### Task 12: 验证PDF预览

- [ ] **Step 1: 添加并选择PDF**

1. 点击"添加PDF"添加一个PDF文件
2. 在列表中点击该PDF

Expected: 右栏显示PDF预览，可以翻页和缩放

### Task 13: 验证多级子文件夹

- [ ] **Step 1: 创建子文件夹**

目前前端支持创建子文件夹，但UI需要添加选择父文件夹的功能。这将在阶段3完善。

### Task 14: 验证统一搜索框

- [ ] **Step 1: 测试文件名搜索**

1. 在搜索框选择"文件名"
2. 输入关键词搜索

Expected: 显示匹配的PDF文件列表

---

## 完成标准

- [ ] 三栏布局正确显示
- [ ] PDF预览器可显示PDF内容并翻页
- [ ] 子文件夹UI组件可递归显示
- [ ] 统一搜索框支持内容/文件名切换
- [ ] 所有代码编译通过
- [ ] 所有改动已提交到 git