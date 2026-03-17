# PDF Manager (Tauri)

本地PDF管理工具，支持扫描版PDF的OCR识别和全文搜索。基于 Tauri 2.0 + Rust + Svelte 构建。

## 功能特性

- **文件夹管理** - 创建文件夹分类管理PDF文件，支持多级嵌套
- **智能识别** - 自动检测PDF类型（文字型/扫描型），选择最优处理方式
- **OCR识别** - 使用 Tesseract OCR 识别扫描版PDF中的内容
- **全文搜索** - 基于 Tantivy 和 jieba 的中文全文搜索
- **PDF预览** - 内置PDF预览器
- **批量导入** - 支持单个文件、多文件、整个文件夹批量导入
- **跨平台** - 支持 Windows、macOS、Linux
- **高性能** - Rust 后端，内存占用低，响应速度快

## 技术栈

| 层级 | 技术 |
|------|------|
| 前端框架 | Svelte 4 + TypeScript |
| 构建工具 | Vite 5 |
| 桌面框架 | Tauri 2.0 |
| 后端语言 | Rust |
| 数据库 | SQLite (rusqlite) |
| 搜索引擎 | Tantivy + jieba-rs |
| PDF处理 | lopdf, pdf-extract |
| OCR引擎 | Tesseract |

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10/11, macOS 10.15+, Linux |
| Node.js | 18.x 或更高版本 |
| Rust | 1.70 或更高版本 |
| 磁盘 | 至少 500MB 可用空间 |

## 开发指南

### 环境准备

1. **安装 Node.js**
   - 从 [nodejs.org](https://nodejs.org/) 下载 LTS 版本

2. **安装 Rust**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

3. **安装系统依赖** (Linux)
   ```bash
   sudo apt install libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev
   ```

### 安装依赖

```bash
# 安装前端依赖
npm install

# Rust 依赖会在首次运行时自动编译
```

### 开发运行

```bash
npm run tauri dev
```

### 构建发布

```bash
npm run tauri build
```

构建产物位于 `src-tauri/target/release/bundle/` 目录。

## 项目结构

```
pdf-manager-tauri/
├── src/                      # Svelte 前端
│   ├── lib/
│   │   ├── api/              # Tauri API 封装
│   │   ├── components/       # UI 组件
│   │   └── stores/           # Svelte stores
│   ├── App.svelte            # 主应用组件
│   └── main.ts               # 入口文件
├── src-tauri/                # Rust 后端
│   ├── src/
│   │   ├── commands/         # Tauri 命令
│   │   ├── db/               # 数据库操作
│   │   ├── models/           # 数据模型
│   │   ├── services/         # 业务服务
│   │   ├── lib.rs            # 库入口
│   │   └── main.rs           # 程序入口
│   ├── Cargo.toml            # Rust 依赖
│   └── tauri.conf.json       # Tauri 配置
├── tesseract/                # Tesseract 配置
├── index.html                # HTML 模板
├── package.json              # Node 依赖
├── tsconfig.json             # TypeScript 配置
└── vite.config.ts            # Vite 配置
```

## 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                    Svelte Frontend                          │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ FolderTree  │ │  PdfList    │ │  SearchBar/Results  │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                    Tauri Commands                           │
│  folder.rs  │  pdf.rs  │  search.rs  │  ocr.rs             │
├─────────────────────────────────────────────────────────────┤
│                    Rust Services                            │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────────────┐   │
│  │ PdfService  │ │ OcrService  │ │   SearchService     │   │
│  └─────────────┘ └─────────────┘ └─────────────────────┘   │
├─────────────────────────────────────────────────────────────┤
│                    Data Layer                               │
│  ┌─────────────────┐  ┌───────────────────────────────┐    │
│  │ SQLite Database │  │ Tantivy Search Index          │    │
│  └─────────────────┘  └───────────────────────────────┘    │
└─────────────────────────────────────────────────────────────┘
```

## API 参考

### 文件夹操作

```typescript
// 获取所有文件夹
invoke('get_folders'): Promise<Folder[]>

// 创建文件夹
invoke('create_folder', { name: string, parentId?: number }): Promise<Folder>

// 删除文件夹
invoke('delete_folder', { id: number }): Promise<void>
```

### PDF 操作

```typescript
// 导入PDF
invoke('import_pdf', { path: string, folderId?: number }): Promise<Pdf>

// 获取文件夹中的PDF列表
invoke('get_pdfs_by_folder', { folderId?: number }): Promise<Pdf[]>

// 删除PDF
invoke('delete_pdf', { id: number }): Promise<void>
```

### 搜索

```typescript
// 搜索PDF内容
invoke('search_content', { query: string }): Promise<SearchResult[]>
```

### OCR

```typescript
// 处理PDF OCR
invoke('process_ocr', { pdfId: number }): Promise<void>

// 获取OCR状态
invoke('get_ocr_status', { pdfId: number }): Promise<OcrStatus>
```

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Svelte](https://svelte.dev/) - 前端框架
- [Tantivy](https://github.com/quickwit-oss/tantivy) - 全文搜索引擎
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract) - OCR引擎