# PDF OCR 应用优化设计文档

## 概述

本文档描述 PDF OCR 管理工具的优化设计方案，解决现有问题并添加新功能。

## 需求清单

| # | 需求 | 类型 | 优先级 |
|---|------|------|--------|
| 1 | 日志系统 | 基础设施 | 高 |
| 2 | 文件夹创建交互修复 | Bug修复 | 高 |
| 3 | PDF添加失败排查 | Bug修复 | 高 |
| 4 | 三栏布局 + PDF预览 | 功能新增 | 中 |
| 5 | 多级子文件夹 | 功能新增 | 中 |
| 6 | 统一搜索框 | 功能新增 | 中 |
| 7 | 文件夹关联物理目录 | 功能新增 | 低 |
| 8 | OCR手动触发 | 功能新增 | 低 |

## 实现方案：分阶段实施

### 阶段1：基础设施 + Bug修复

#### 1.1 日志系统

**技术选型**：
- Rust后端使用 `tracing` + `tracing-appender` 框架
- 日志文件存放在应用数据目录下的 `logs/` 文件夹
- 日志文件按日期滚动

**依赖添加**（Cargo.toml）：
```toml
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
tracing-appender = "0.2"
```

**初始化代码**（lib.rs）：
```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

fn init_logging(app_handle: &tauri::AppHandle) {
    let log_dir = app_handle.path().app_data_dir().unwrap().join("logs");
    std::fs::create_dir_all(&log_dir).ok();

    let file_appender = tracing_appender::rolling::daily(&log_dir, "pdf-ocr.log");
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .init();
}
```

**日志级别**：
- 生产环境：INFO
- 开发环境：DEBUG

#### 1.2 文件夹创建交互修复

**修改文件**：`src/lib/components/FolderTree.svelte`

**改动内容**：
```svelte
<div class="new-folder">
  <input
    type="text"
    bind:value={newFolderName}
    placeholder="文件夹名称"
    on:keydown={(e) => e.key === 'Enter' && handleCreate()}
  />
  <button on:click={handleCreate}>确定</button>
</div>
```

#### 1.3 PDF添加问题排查

**修改文件**：
- `src-tauri/src/commands/pdf.rs` - 添加详细日志
- `src/lib/components/PdfList.svelte` - 改进错误处理

**后端日志示例**：
```rust
#[tauri::command]
pub fn add_pdf(...) -> Result<PdfInfo, String> {
    tracing::info!("Adding PDF: path={}, folder_id={:?}", path, folder_id);

    let src_path = PathBuf::from(&path);
    if !src_path.exists() {
        tracing::error!("File does not exist: {}", path);
        return Err("File does not exist".to_string());
    }

    tracing::debug!("Copying file to storage...");
    // ... rest of the function
}
```

**前端错误处理**：
```svelte
async function handleFileSelect(e: Event) {
  const input = e.target as HTMLInputElement;
  if (input.files) {
    for (const file of Array.from(input.files)) {
      try {
        console.log('Adding PDF:', file.path);
        const result = await addPdf(file.path, $selectedFolderId ?? undefined);
        console.log('PDF added successfully:', result);
      } catch (err) {
        console.error('Failed to add PDF:', err);
        alert('添加失败: ' + err);
      }
    }
    await loadPdfs();
  }
}
```

---

### 阶段2：布局重构 + 核心功能

#### 2.1 三栏布局

**布局结构**：
```
┌──────────────┬──────────────────────────┬───────────────────────┐
│   文件夹树   │       中间内容区域       │      PDF预览器        │
│   200px      │        flex: 1           │       350px           │
└──────────────┴──────────────────────────┴───────────────────────┘
```

**修改文件**：`src/App.svelte`

**改动内容**：
```svelte
<script lang="ts">
  import FolderTree from './lib/components/FolderTree.svelte';
  import PdfList from './lib/components/PdfList.svelte';
  import SearchBar from './lib/components/SearchBar.svelte';
  import SearchResults from './lib/components/SearchResults.svelte';
  import PdfViewer from './lib/components/PdfViewer.svelte';
  import { selectedPdfId } from './lib/stores';
</script>

<main>
  <FolderTree />
  <div class="content">
    <SearchBar />
    <PdfList />
    <SearchResults />
  </div>
  {#if $selectedPdfId}
    <PdfViewer pdfId={$selectedPdfId} />
  {:else}
    <div class="preview-placeholder">
      选择PDF文件预览
    </div>
  {/if}
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    width: 100vw;
  }

  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .preview-placeholder {
    width: 350px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: #f5f5f5;
    color: #999;
  }
</style>
```

#### 2.2 PDF预览器

**新建文件**：`src/lib/components/PdfViewer.svelte`

**技术方案**：
- 使用 `pdf.js` 库渲染PDF
- 支持翻页、缩放功能

**依赖添加**（package.json）：
```json
"pdfjs-dist": "^3.11.0"
```

**组件设计**：
```svelte
<script lang="ts">
  import * as pdfjsLib from 'pdfjs-dist';
  import { onMount, onDestroy } from 'svelte';

  export let pdfId: number;

  let canvas: HTMLCanvasElement;
  let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null;
  let currentPage = 1;
  let totalPages = 0;
  let scale = 1.0;

  async function loadPdf() {
    // 获取PDF路径并加载
    // ...
  }

  async function renderPage(pageNum: number) {
    // 渲染指定页面
    // ...
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
</script>

<div class="pdf-viewer">
  <div class="toolbar">
    <button on:click={prevPage} disabled={currentPage <= 1}>上一页</button>
    <span>{currentPage} / {totalPages}</span>
    <button on:click={nextPage} disabled={currentPage >= totalPages}>下一页</button>
  </div>
  <div class="canvas-container">
    <canvas bind:this={canvas}></canvas>
  </div>
</div>
```

#### 2.3 多级子文件夹

**修改文件**：`src/lib/components/FolderTree.svelte`

**数据结构**：数据库已支持 `parent_id` 字段，无需修改。

**UI改动**：
- 递归渲染文件夹树
- 添加展开/折叠功能
- 创建时选择父文件夹

**递归组件设计**：
```svelte
<script lang="ts">
  function buildTree(folders: Folder[]): TreeNode[] {
    // 构建树形结构
    const map = new Map<number, TreeNode>();
    const roots: TreeNode[] = [];

    folders.forEach(f => {
      map.set(f.id, { ...f, children: [] });
    });

    folders.forEach(f => {
      const node = map.get(f.id)!;
      if (f.parent_id) {
        map.get(f.parent_id)?.children.push(node);
      } else {
        roots.push(node);
      }
    });

    return roots;
  }
</script>

{#each treeNodes as node}
  <FolderNode {node} level={0} />
{/each}
```

#### 2.4 统一搜索框

**修改文件**：`src/lib/components/SearchBar.svelte`

**UI设计**：
```svelte
<script lang="ts">
  let searchMode: 'content' | 'filename' = 'content';

  async function handleSearch() {
    if (searchMode === 'content') {
      searchResults.set(await searchContent($searchQuery.trim()));
    } else {
      searchResults.set(await searchFilename($searchQuery.trim()));
    }
  }
</script>

<div class="search-bar">
  <select bind:value={searchMode}>
    <option value="content">搜索内容</option>
    <option value="filename">搜索文件名</option>
  </select>
  <input type="text" bind:value={$searchQuery} placeholder="输入关键词..." />
  <button on:click={handleSearch}>搜索</button>
</div>
```

**后端支持**：
- 新增 `search_filename` 命令
- 使用 SQL LIKE 或全文索引搜索文件名

---

### 阶段3：高级功能

#### 3.1 文件夹关联物理目录

**数据库修改**：
```sql
ALTER TABLE folders ADD COLUMN storage_path TEXT;
```

**创建文件夹流程**：
1. 用户点击创建文件夹
2. 弹出目录选择对话框（使用 tauri-plugin-dialog）
3. 用户选择物理目录
4. 保存路径到 storage_path 字段

**添加PDF流程**：
1. 检查文件夹是否有关联存储路径
2. 如果有，复制到该路径
3. 如果没有，使用默认存储位置

#### 3.2 OCR手动触发

**UI改动**：
- PDF列表项添加"开始处理"按钮（状态为pending时显示）
- 显示处理进度

**新增命令**：
```rust
#[tauri::command]
pub async fn start_ocr(pdf_id: i64, app_handle: tauri::AppHandle) -> Result<(), String> {
    // 异步执行OCR
    // 发送进度事件
}
```

**进度事件**：
```rust
app_handle.emit("ocr-progress", ProgressPayload {
    pdf_id,
    current: 3,
    total: 10,
});
```

---

## 技术依赖

### Rust 依赖
- `tracing` - 日志框架
- `tracing-subscriber` - 日志订阅
- `tracing-appender` - 日志文件管理

### Node 依赖
- `pdfjs-dist` - PDF渲染

---

## 文件改动清单

### 新建文件
- `src/lib/components/PdfViewer.svelte` - PDF预览组件

### 修改文件
- `src-tauri/Cargo.toml` - 添加日志依赖
- `src-tauri/src/lib.rs` - 初始化日志系统
- `src-tauri/src/commands/pdf.rs` - 添加日志、OCR触发命令
- `src-tauri/src/commands/search.rs` - 添加文件名搜索
- `src-tauri/src/db/schema.rs` - 添加 storage_path 字段
- `src/App.svelte` - 三栏布局
- `src/lib/components/FolderTree.svelte` - 子文件夹、回车创建
- `src/lib/components/SearchBar.svelte` - 统一搜索框
- `src/lib/components/PdfList.svelte` - OCR按钮
- `package.json` - 添加 pdfjs-dist

---

## 验收标准

### 阶段1
- [ ] 日志文件正确生成并记录运行信息
- [ ] 回车键可创建文件夹
- [ ] PDF添加失败时显示具体错误信息

### 阶段2
- [ ] 三栏布局正确显示
- [ ] PDF预览器可显示PDF内容并翻页
- [ ] 子文件夹可创建并正确显示层级
- [ ] 文件名搜索功能正常工作

### 阶段3
- [ ] 文件夹可关联物理目录
- [ ] PDF可手动触发OCR处理
- [ ] OCR进度正确显示