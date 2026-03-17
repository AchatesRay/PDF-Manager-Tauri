# PDF Manager 重构设计文档 - Rust + Tauri 方案

## 1. 概述

### 1.1 背景
当前 Python + PaddleOCR 方案存在以下问题：
- OCR 功能不稳定，打包后存在兼容性问题
- 打包体积过大（>500MB）
- 运行时依赖复杂

### 1.2 重构目标
- 解决 OCR 稳定性问题
- 大幅减小打包体积（目标 ~50MB）
- 保持所有现有功能
- 提升性能和用户体验

### 1.3 核心功能保留
- 本地 PDF 文件管理
- 扫描版 PDF 的 OCR 识别
- 全文搜索功能
- 文件夹分类管理
- 完全离线运行

---

## 2. 技术选型

| 组件 | 技术选择 | 说明 |
|------|---------|------|
| **GUI框架** | Tauri 2.0 | 轻量级 Web UI 框架，比 Electron 小 10 倍 |
| **前端** | Svelte + TypeScript | 编译时框架，包体小，开发体验好 |
| **后端语言** | Rust | 内存安全，性能优秀，生态成熟 |
| **OCR引擎** | Tesseract 5.x | 开源成熟，中英文支持好，模型小巧 |
| **PDF处理** | pdfium-render | 高性能 PDF 渲染，基于 Chrome PDF 引擎 |
| **全文搜索** | Tantivy | Rust 版 Lucene，性能优秀 |
| **数据库** | SQLite (rusqlite) | 轻量级本地存储 |
| **中文分词** | jieba-rs | Rust 版 jieba |

### 2.1 依赖体积估算

| 组件 | 大小 |
|------|------|
| Tauri 运行时 | ~10MB |
| Tesseract 核心 | ~5MB |
| 中文语言包 (chi_sim) | ~20MB |
| 英文语言包 (eng) | ~5MB |
| 应用程序 | ~10MB |
| **总计** | **~50MB** |

对比当前 Python 方案（>500MB），减少约 **90%**

---

## 3. 系统架构

```
┌──────────────────────────────────────────────────────────────┐
│                    Svelte Frontend (Web)                     │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────────┐    │
│  │FolderTree│ │ PdfList  │ │PdfViewer │ │ SearchBar    │    │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └──────┬───────┘    │
│       │            │            │              │            │
│       └────────────┴────────────┴──────────────┘            │
│                          │                                   │
│                    Tauri Invoke API                          │
│                          │                                   │
├──────────────────────────┼───────────────────────────────────┤
│                    Rust Backend                              │
│                          │                                   │
│  ┌───────────────────────┴───────────────────────┐           │
│  │              Commands Layer                    │           │
│  │  folder_cmd │ pdf_cmd │ search_cmd │ ocr_cmd  │           │
│  └───────┬──────────┬──────────┬───────────┬─────┘           │
│          │          │          │           │                  │
│  ┌───────┴────┐ ┌───┴────┐ ┌───┴────┐ ┌────┴─────┐           │
│  │FolderSvc   │ │PdfSvc  │ │SearchSvc│ │ OcrSvc   │           │
│  └────────────┘ └────────┘ └────────┘ └──────────┘           │
│          │          │          │           │                  │
│  ┌───────┴──────────┴──────────┴───────────┴─────┐           │
│  │              Data Layer                        │           │
│  │  SQLite (rusqlite) │ Tantivy Index │ Tesseract│           │
│  └───────────────────────────────────────────────┘           │
└──────────────────────────────────────────────────────────────┘
```

### 3.1 设计原则
- **前后端分离**：前端负责 UI，后端负责业务逻辑
- **异步处理**：OCR 等耗时操作异步执行，不阻塞 UI
- **增量处理**：OCR 结果缓存，避免重复识别
- **事件驱动**：通过 Tauri 事件系统通知前端进度

---

## 4. 项目结构

```
pdf-manager/
├── src-tauri/                    # Rust 后端
│   ├── src/
│   │   ├── main.rs               # 入口
│   │   ├── lib.rs                # 库定义
│   │   ├── commands/             # Tauri 命令 (API)
│   │   │   ├── mod.rs
│   │   │   ├── folder.rs         # 文件夹操作
│   │   │   ├── pdf.rs            # PDF 操作
│   │   │   ├── search.rs         # 搜索操作
│   │   │   └── ocr.rs            # OCR 操作
│   │   ├── services/             # 业务服务
│   │   │   ├── mod.rs
│   │   │   ├── pdf_service.rs    # PDF 处理
│   │   │   ├── ocr_service.rs    # OCR 引擎封装
│   │   │   ├── search_service.rs # 全文搜索
│   │   │   └── folder_service.rs # 文件夹管理
│   │   ├── models/               # 数据模型
│   │   │   ├── mod.rs
│   │   │   ├── folder.rs
│   │   │   ├── pdf.rs
│   │   │   └── page.rs
│   │   └── db/                   # 数据库
│   │       ├── mod.rs
│   │       └── schema.rs
│   ├── Cargo.toml
│   └── tauri.conf.json
│
├── src/                          # Svelte 前端
│   ├── main.ts
│   ├── App.svelte
│   ├── lib/
│   │   ├── api/                  # Tauri API 封装
│   │   ├── stores/               # Svelte stores (状态管理)
│   │   └── components/           # UI 组件
│   │       ├── FolderTree.svelte
│   │       ├── PdfList.svelte
│   │       ├── PdfViewer.svelte
│   │       └── SearchBar.svelte
│   └── routes/                   # 页面路由 (如需要)
│
├── static/                       # 静态资源
│   └── icons/
│
├── tesseract/                    # OCR 模型 (打包时嵌入)
│   ├── chi_sim.traineddata       # 简体中文 (~20MB)
│   └── eng.traineddata           # 英文 (~5MB)
│
├── package.json
└── README.md
```

---

## 5. 核心模块设计

### 5.1 PDF 处理模块

```rust
// src-tauri/src/services/pdf_service.rs

pub struct PdfService {
    pdfium: Pdfium,
}

impl PdfService {
    /// 渲染 PDF 页面为图像
    pub fn render_page(&self, pdf_path: &Path, page: u32, dpi: u32) -> Result<RgbImage>;

    /// 提取 PDF 文本（文字型 PDF）
    pub fn extract_text(&self, pdf_path: &Path, page: u32) -> Result<String>;

    /// 检测 PDF 类型
    pub fn detect_type(&self, pdf_path: &Path) -> Result<PdfType>;

    /// 获取 PDF 元信息
    pub fn get_metadata(&self, pdf_path: &Path) -> Result<PdfMetadata>;

    /// 获取总页数
    pub fn page_count(&self, pdf_path: &Path) -> Result<u32>;
}

pub enum PdfType {
    Text,      // 文字型，可直接提取文本
    Scanned,   // 扫描型，需要 OCR
    Mixed,     // 混合型
}
```

### 5.2 OCR 服务模块

```rust
// src-tauri/src/services/ocr_service.rs

use tesseract::Tesseract;

pub struct OcrService {
    tesseract: Tesseract,
    language: String,
}

impl OcrService {
    /// 初始化 OCR 引擎（启动时调用）
    pub fn new(model_path: &Path, language: &str) -> Result<Self>;

    /// 识别图像中的文字
    pub fn recognize(&self, image: &RgbImage) -> Result<String>;

    /// 识别 PDF 页面
    pub fn recognize_page(
        &self,
        pdf_service: &PdfService,
        pdf_path: &Path,
        page: u32
    ) -> Result<String>;

    /// 检查模型是否已安装
    pub fn is_model_available(lang: &str) -> bool;

    /// 获取可用语言列表
    pub fn available_languages() -> Vec<String>;
}
```

### 5.3 搜索服务模块

```rust
// src-tauri/src/services/search_service.rs

use tantivy::{Index, Schema, collector::TopDocs, query::QueryParser};

pub struct SearchService {
    index: Index,
    schema: Schema,
    tokenizer: JiebaTokenizer,
}

impl SearchService {
    /// 创建或打开索引
    pub fn open(index_path: &Path) -> Result<Self>;

    /// 索引一个 PDF 页面
    pub fn index_page(
        &mut self,
        page_id: u64,
        pdf_id: u64,
        page_number: u32,
        filename: &str,
        content: &str
    ) -> Result<()>;

    /// 搜索
    pub fn search(
        &self,
        query: &str,
        folder_id: Option<i64>,
        limit: usize
    ) -> Result<Vec<SearchResult>>;

    /// 删除 PDF 相关索引
    pub fn delete_pdf(&mut self, pdf_id: u64) -> Result<()>;

    /// 优化索引
    pub fn optimize(&mut self) -> Result<()>;
}

pub struct SearchResult {
    pub page_id: u64,
    pub pdf_id: u64,
    pub page_number: u32,
    pub filename: String,
    pub score: f32,
    pub highlight: String,
}
```

### 5.4 Tauri 命令层（API）

```rust
// src-tauri/src/commands/pdf.rs

#[tauri::command]
pub async fn add_pdf(
    path: String,
    folder_id: Option<i64>,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<PdfInfo, String> {
    // 添加 PDF，自动检测类型并处理
    // 通过 app.emit() 发送进度事件
}

#[tauri::command]
pub async fn get_pdf_list(
    folder_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<PdfInfo>, String>;

#[tauri::command]
pub async fn delete_pdf(
    pdf_id: i64,
    state: State<'_, AppState>,
) -> Result<(), String>;

#[tauri::command]
pub async fn get_pdf_status(
    pdf_id: i64,
    state: State<'_, AppState>,
) -> Result<PdfStatus, String>;
```

```rust
// src-tauri/src/commands/search.rs

#[tauri::command]
pub async fn search(
    query: String,
    folder_id: Option<i64>,
    state: State<'_, AppState>,
) -> Result<Vec<SearchResult>, String>;
```

```rust
// src-tauri/src/commands/ocr.rs

#[tauri::command]
pub async fn get_ocr_status(
    state: State<'_, AppState>,
) -> Result<OcrStatus, String>;

#[tauri::command]
pub async fn reprocess_pdf(
    pdf_id: i64,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String>;
```

---

## 6. 数据模型

### 6.1 数据库表结构

```sql
-- 文件夹表
CREATE TABLE folders (
    id INTEGER PRIMARY KEY,
    name TEXT NOT NULL,
    parent_id INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (parent_id) REFERENCES folders(id)
);

-- PDF 文件表
CREATE TABLE pdfs (
    id INTEGER PRIMARY KEY,
    folder_id INTEGER,
    filename TEXT NOT NULL,
    original_path TEXT,
    storage_path TEXT NOT NULL,
    file_size INTEGER,
    page_count INTEGER,
    pdf_type TEXT CHECK(pdf_type IN ('text', 'scanned', 'mixed')),
    status TEXT CHECK(status IN ('pending', 'processing', 'done', 'error')),
    error_message TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (folder_id) REFERENCES folders(id)
);

-- PDF 页面表
CREATE TABLE pdf_pages (
    id INTEGER PRIMARY KEY,
    pdf_id INTEGER NOT NULL,
    page_number INTEGER NOT NULL,
    ocr_text TEXT,
    ocr_status TEXT CHECK(ocr_status IN ('pending', 'done', 'error')),
    thumbnail_path TEXT,
    FOREIGN KEY (pdf_id) REFERENCES pdfs(id) ON DELETE CASCADE
);

-- 索引
CREATE INDEX idx_pdfs_folder ON pdfs(folder_id);
CREATE INDEX idx_pdfs_status ON pdfs(status);
CREATE INDEX idx_pages_pdf ON pdf_pages(pdf_id);
```

### 6.2 Tantivy 索引 Schema

```rust
fn create_schema() -> Schema {
    let mut schema_builder = Schema::builder();

    schema_builder.add_u64_field("page_id", INDEXED | STORED);
    schema_builder.add_u64_field("pdf_id", STORED);
    schema_builder.add_u64_field("folder_id", STORED);
    schema_builder.add_u64_field("page_number", STORED);
    schema_builder.add_text_field("filename", TEXT | STORED);
    schema_builder.add_text_field("content", TEXT | STORED);

    schema_builder.build()
}
```

---

## 7. 数据流设计

### 7.1 PDF 导入流程

```
用户添加 PDF
     │
     ▼
┌────────────────┐
│  前端调用      │  invoke('add_pdf', { path, folderId })
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  复制到存储    │  data/pdfs/{uuid}.pdf
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  检测 PDF 类型 │  PdfService::detect_type()
└───────┬────────┘
        │
   ┌────┴────┐
   ▼         ▼
文字型     扫描型/混合型
   │         │
   ▼         ▼
┌────────┐ ┌────────────┐
│提取文本│ │逐页OCR识别 │ ← emit('ocr-progress', {...})
└───┬────┘ └─────┬──────┘
    │            │
    └─────┬──────┘
          ▼
┌────────────────┐
│  写入数据库    │  pdfs, pdf_pages 表
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  建立搜索索引  │  Tantivy index
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  发送完成事件  │  emit('pdf-processed', { pdfId })
└────────────────┘
```

### 7.2 搜索流程

```
用户输入关键词
     │
     ▼
┌────────────────┐
│  前端调用      │  invoke('search', { query, folderId })
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  jieba 分词    │  中文分词处理
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  Tantivy 检索  │  全文搜索
└───────┬────────┘
        │
        ▼
┌────────────────┐
│  生成高亮片段  │  显示匹配上下文
└───────┬────────┘
        │
        ▼
   返回结果列表
```

---

## 8. 数据存储

### 8.1 存储路径

```
%APPDATA%/pdf-manager/
├── data/
│   ├── pdfs/                    # PDF 文件存储
│   │   ├── {uuid1}.pdf
│   │   └── {uuid2}.pdf
│   ├── thumbnails/              # 缩略图缓存
│   │   ├── {pdf_id}/
│   │   │   ├── page_1.png
│   │   │   └── page_2.png
│   └── index/                   # Tantivy 搜索索引
│       └── ...
├── pdf-manager.db               # SQLite 数据库
└── config.json                  # 配置文件
```

---

## 9. 打包配置

### 9.1 Tauri 配置

```json
{
  "build": {
    "beforeBuildCommand": "npm run build",
    "beforeDevCommand": "npm run dev"
  },
  "bundle": {
    "active": true,
    "targets": ["msi", "nsis"],
    "identifier": "com.pdfmanager.app",
    "resources": [
      "tesseract/*"
    ],
    "externalBin": [
      "binaries/tesseract"
    ]
  }
}
```

### 9.2 Cargo 依赖

```toml
[dependencies]
tauri = { version = "2", features = ["shell-open"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tokio = { version = "1", features = ["full"] }
rusqlite = { version = "0.31", features = ["bundled"] }
tantivy = "0.22"
jieba-rs = "0.6"
tesseract-plumbing = "0.1"
pdfium-render = "0.8"
uuid = { version = "1", features = ["v4"] }
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
```

### 9.3 打包后结构

```
PDF-Manager-Setup.exe (~50MB)
├── pdf-manager.exe              # 主程序 (~10MB)
├── WebView2Loader.dll           # WebView 运行时
├── tesseract.dll                # OCR 引擎 (~5MB)
├── tesseract/                   # 语言模型 (~25MB)
│   ├── chi_sim.traineddata
│   └── eng.traineddata
└── resources/                   # 前端资源
```

---

## 10. 迁移策略

### 10.1 数据迁移

由于技术栈完全改变，需要提供数据迁移工具：

1. **导出功能**：在旧版本中导出 PDF 列表和 OCR 结果
2. **导入功能**：在新版本中导入数据，重新建立索引

### 10.2 开发阶段

1. **阶段一**：搭建 Tauri 项目骨架，实现基础 UI
2. **阶段二**：实现 PDF 处理和 OCR 功能
3. **阶段三**：实现搜索功能
4. **阶段四**：完善 UI 和用户体验
5. **阶段五**：打包测试和优化

---

## 11. 风险与缓解

| 风险 | 影响 | 缓解措施 |
|------|------|---------|
| Tesseract 中文识别效果不如 PaddleOCR | 中 | 可调整 DPI 参数，或考虑其他 OCR 方案 |
| Rust 学习曲线 | 低 | 用户已接受，可边学边做 |
| Tauri 生态成熟度 | 低 | Tauri 2.0 已稳定，社区活跃 |
| pdfium-render 兼容性 | 低 | 基于 Chrome PDF 引擎，稳定可靠 |

---

## 12. 验收标准

| 场景 | 预期结果 |
|------|---------|
| 安装包大小 | < 60MB |
| 添加扫描版 PDF | OCR 正确识别中文，可搜索到内容 |
| 添加文字型 PDF | 直接提取文字，无需 OCR |
| 批量添加 100 个 PDF | 界面不卡顿，后台逐个处理 |
| 搜索关键词 | 返回匹配的 PDF，高亮显示片段 |
| 点击搜索结果 | 打开预览并定位到对应页面 |
| 删除 PDF | 文件、索引、数据库记录全部清理 |
| 内存占用 | < 200MB（处理时） |
| 启动时间 | < 3 秒 |