# PDF Manager

本地 PDF 管理工具，支持扫描版 PDF 的 OCR 识别和全文搜索。基于 Tauri 2.0 + Rust + Svelte 构建。

## 版本

**v1.0.0** - 2026-03-24

## 功能特性

- **文件夹管理** - 创建文件夹分类管理 PDF 文件，支持多级嵌套和自定义存储路径
- **智能识别** - 自动检测 PDF 类型（文字型/扫描型），选择最优处理方式
- **OCR 识别** - 使用 Tesseract OCR 识别扫描版 PDF 中的内容，支持进度追踪
- **全文搜索** - 基于 Tantivy 和 jieba 的中文全文搜索，支持内容和文件名搜索
- **PDF 预览** - 内置 PDF 预览器，支持页面渲染和缩放
- **批量导入** - 支持单个文件、多文件、整个文件夹批量导入
- **外部阅读器** - 支持配置外部 PDF 阅读器打开文件
- **OCR 统计** - 文件夹级别显示 OCR 处理进度
- **可调节布局** - 左中右三栏布局可自由调节宽度
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
| PDF 处理 | lopdf, pdf-extract, pdfium |
| OCR 引擎 | Tesseract |

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10/11, macOS 10.15+, Linux |
| Node.js | 18.x 或更高版本 |
| Rust | 1.70 或更高版本 |
| 磁盘 | 至少 500MB 可用空间 |

## 安装

从 [Releases](https://github.com/AchatesRay/PDF-Manager-Tauri/releases) 页面下载对应平台的安装包。

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
npm install
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
├── static/                   # 静态资源
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

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Svelte](https://svelte.dev/) - 前端框架
- [Tantivy](https://github.com/quickwit-oss/tantivy) - 全文搜索引擎
- [Tesseract OCR](https://github.com/tesseract-ocr/tesseract) - OCR 引擎