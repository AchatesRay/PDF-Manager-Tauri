# PDF Manager

本地 PDF 管理工具，支持扫描版 PDF 的 OCR 识别和全文搜索。基于 Tauri 2.0 + Rust + Svelte 构建。

## 版本

**v1.1.0** - 2026-03-28

## 功能特性

- **文件夹管理** - 创建文件夹分类管理 PDF 文件，支持多级嵌套和自定义存储路径
- **智能识别** - 自动检测 PDF 类型（文字型/扫描型），选择最优处理方式
- **OCR 识别** - 使用 PP-OCRv5 Mobile 模型（运行时标识 Balanced）识别扫描版 PDF 中的内容，支持中文识别，内存占用约 300MB
- **全文搜索** - 基于 Tantivy 和 jieba 的中文全文搜索，支持内容和文件名搜索
- **PDF 预览** - 内置 PDF 预览器，支持页面渲染和缩放
- **批量导入** - 支持单个文件、多文件、整个文件夹批量导入
- **外部阅读器** - 支持配置外部 PDF 阅读器打开文件
- **OCR 统计** - 文件夹级别显示 OCR 处理进度
- **可调节布局** - 左中右三栏布局可自由调节宽度
- **Windows 优先** - 当前打包目标为 Windows（NSIS）；后端为 Rust，具备跨平台潜力
- **高性能** - Rust 后端，内存占用低，响应速度快

## 系统要求

| 项目 | 要求 |
|------|------|
| 操作系统 | Windows 10/11（当前发布平台） |
| 磁盘 | 至少 1GB 可用空间（含 OCR 模型） |
| 内存 | 建议 4GB 以上 |

## 安装

从 [Releases](https://github.com/AchatesRay/PDF-Manager-Tauri/releases) 页面下载对应平台的安装包。

亦可从 GitHub Actions 的 **`build-windows`** 工作流产物（Artifacts）下载 `pdf-manager-win-x64.zip`
（`pdf-manager.exe` + `pdfium.dll` + 使用说明，**不含 OCR 模型**，push 到 main 或手动触发即构建）。

### OCR 模型

首次使用时，程序会自动检测 OCR 模型。如未安装，点击"重新检测"按钮即可：
- 支持在线自动下载模型
- 也可手动下载模型文件放到 `models` 目录

模型文件（PP-OCRv5 Mobile，运行时标识 Balanced）：
- `pp-ocrv5_mobile_det.onnx` - 文字检测模型
- `pp-ocrv5_mobile_rec.onnx` - 文字识别模型
- `ppocrv5_dict.txt` - 字符字典

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

默认存储在应用程序所在目录，可在设置中改为自定义数据目录：

- `pdf-manager.db` - SQLite 数据库（WAL 模式）
- `pdfs/` - PDF 文件存储
- `index/` - 搜索索引（schema 升级时旧索引备份为 `index.bak`）
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
| OCR 引擎 | PP-OCRv5 Mobile (ONNX Runtime / oar-ocr) |

## 更新日志

### v1.1.0 (2026-03-28)

- 切换 OCR 模型为 PP-OCRv5 Mobile（运行时标识 Balanced）
- 简化模型选择，固定使用 Balanced 模型
- 优化模型状态检测和下载流程

### 优化（2026-09-24，未升版本号）

- Phase 0：OCR 队列后端真调度、部分失败标 error、force 清搜索索引
- Phase 1：批量索引提交、Pdfium 线程本地复用、同步命令线程模型
- Phase 2：索引安全重建（备份）、folder 查询下推、SQLite WAL、页面唯一索引
- Phase 3：事件驱动队列刷新、预览页 LRU 缓存、CSP/权限收紧

### 修复（2026-09-26，`4d3360e`）

- Phase 5 真机验收：串行 OCR / force 清索引 / folder 过滤三项全通过
- P0-6：根级导入 PDF 时持数据库锁二次加锁导致应用挂起
- P0-7：低对比度预处理把留白页压黑导致 OCR 空结果被标为完成

### 待办清零与真机测试（2026-09-26，plan §6 待办 1–7）

- **Q-1 中文子串搜索**：索引 v6（jieba 整词 + 中文单字 + bigram 滑窗）、查询侧 bigram Must 拆解、
  空索引自动回填（`refill_index_from_db`），`北斗` 类子串查询不再漏检
- **Q-2 长 ASCII 词截断**：取证推翻「模型侧弱点」结论——真实根因是 dim≥1400 时 2D 方块分块把
  超宽文本行拦腰裁剪；改为**全宽横带分块** + 去重保留完整识别（dim500–2000 复测全部完整），
  另修复分块数量 floor 公式导致的右/下条带丢失
- **OCR 队列持久化**：`ocr_queue` 表，重启恢复 pending 任务并自动续跑（含崩溃 processing 恢复）
- **可配置预处理**：设置 `ocr_preprocess_mode`（auto/off/on）+ 设置面板下拉
- **组件拆分**：FolderTree → `SettingsPanel`、PdfList → `PdfListItem`
- **OCR 单测套件**：分块覆盖/行容纳、去重、P0-7 回归、预处理模式、模型集成（`#[ignore]`）
- 同批真机测试发现并修复 **P0-8**（pdf-extract 对真实 PDF 断言 panic 导致导入崩溃 → catch_unwind 兜底）、
  **P0-9**（同步命令串行分发：OCR 期间全部 IPC 冻结、排队/取消不可达 → 执行转交独立工作线程）、
  **P0-10**（pdfium 二次 `FPDF_InitLibrary` 死锁 → 全进程唯一渲染线程）
- 真机功能测试：CDP 驱动真实应用，`PDF file/` 15 份真实合同全流程（29 项断言，29/29 PASS），
  见 `Output/待办执行与真机功能测试/`

### CI（2026-09-26，应用户要求恢复）

- `build-windows` 工作流（push `main` / 手动触发）：单测门禁 → 构建 release exe → 打包
  `pdf-manager-win-x64.zip`（`pdf-manager.exe` + `pdfium.dll` + 使用说明，**不含 OCR 模型**，
  仓库与产物双重断言），产物在 Actions Artifacts 保留 30 天

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