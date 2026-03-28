# PDF Manager

本地 PDF 管理工具，支持扫描版 PDF 的 OCR 识别和全文搜索。基于 Tauri 2.0 + Rust + Svelte 构建。

## 版本

**v1.1.0** - 2026-03-28

## 功能特性

- **文件夹管理** - 创建文件夹分类管理 PDF 文件，支持多级嵌套和自定义存储路径
- **智能识别** - 自动检测 PDF 类型（文字型/扫描型），选择最优处理方式
- **OCR 识别** - 使用 PP-OCRv5 Balanced 模型识别扫描版 PDF 中的内容，支持中文识别，内存占用约 300MB
- **全文搜索** - 基于 Tantivy 和 jieba 的中文全文搜索，支持内容和文件名搜索
- **PDF 预览** - 内置 PDF 预览器，支持页面渲染和缩放
- **批量导入** - 支持单个文件、多文件、整个文件夹批量导入
- **外部阅读器** - 支持配置外部 PDF 阅读器打开文件
- **OCR 统计** - 文件夹级别显示 OCR 处理进度
- **可调节布局** - 左中右三栏布局可自由调节宽度
- **跨平台** - 支持 Windows、macOS、Linux
- **高性能** - Rust 后端，内存占用低，响应速度快

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10/11, macOS 10.15+, Linux |
| 磁盘 | 至少 1GB 可用空间（含 OCR 模型） |
| 内存 | 建议 4GB 以上 |

## 安装

从 [Releases](https://github.com/AchatesRay/PDF-Manager-Tauri/releases) 页面下载对应平台的安装包。

### OCR 模型

首次使用时，程序会自动检测 OCR 模型。如未安装，点击"重新检测"按钮即可：
- 支持在线自动下载模型
- 也可手动下载模型文件放到 `models` 目录

模型文件（PP-OCRv5 Balanced）：
- `pp-ocrv5_mobile_det.onnx` - 文字检测模型
- `ch_repsvtr_rec.onnx` - 文字识别模型
- `ppocr_keys_v1.txt` - 字符字典

## 使用方法

### 1. 创建文件夹

首次使用时，先在左侧面板创建一个文件夹来分类管理 PDF 文件。

### 2. 导入 PDF

选中文件夹后，点击"添加"按钮导入 PDF 文件，支持：
- 单个文件导入
- 多文件批量导入
- 文件夹批量导入

### 3. OCR 识别

对于扫描版 PDF：
- 点击 PDF 列表中的"OCR"按钮开始识别
- 支持进度追踪，显示处理进度
- 识别完成后即可搜索内容

### 4. 搜索

在顶部搜索框输入关键词：
- 支持**内容搜索**：在 OCR 识别结果中搜索
- 支持**文件名搜索**：按文件名查找 PDF
- 点击搜索结果可直接跳转到对应页面

### 5. 预览 PDF

点击 PDF 文件，右侧面板将显示预览：
- 支持页面缩放
- 支持翻页浏览
- 可配置外部阅读器打开

## 数据存储

所有数据存储在应用程序所在目录下：
- `pdf-manager.db` - SQLite 数据库
- `pdfs/` - PDF 文件存储
- `index/` - 搜索索引
- `models/` - OCR 模型文件
- `logs/` - 日志文件

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
| OCR 引擎 | PP-OCRv5 Balanced (ONNX Runtime) |

## 更新日志

### v1.1.0 (2026-03-28)

- 切换 OCR 模型为 PP-OCRv5 Balanced 模型
- 简化模型选择，固定使用 Balanced 模型
- 优化模型状态检测和下载流程

### v1.0.0 (2026-03-24)

- 初始版本发布
- 支持文件夹管理、PDF 导入
- 支持 OCR 识别和全文搜索
- 支持 PDF 预览

## 许可证

MIT License

## 致谢

- [Tauri](https://tauri.app/) - 跨平台桌面应用框架
- [Svelte](https://svelte.dev/) - 前端框架
- [Tantivy](https://github.com/quickwit-oss/tantivy) - 全文搜索引擎
- [PaddleOCR](https://github.com/PaddlePaddle/PaddleOCR) - OCR 模型
- [oar-ocr](https://github.com/GreatV/oar-ocr) - ONNX OCR 推理库