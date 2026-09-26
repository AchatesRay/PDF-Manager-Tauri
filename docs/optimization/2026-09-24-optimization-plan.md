# PDF Manager 优化方案（2026-09-24）

> 状态：**Phase 0–5 已完成**（`9648c08..7feb241` 已 push；Phase 5 缺陷修复已于 2026-09-26 真机复验 9/9，**待 commit**）。CI 不添加（workflows 曾于 `9648c08` 被刻意移除）。
> 验证基线：`cargo check` 零警告 · `cargo test --lib` 57/57 · `npm run build` 通过。
> 历史计划见 `docs/superpowers/plans/`（2026-03-26 OCR 优化、2026-03-27 OCR 重构），与本方案冲突处以本方案为准。

## 1. 项目画像

| 项 | 内容 |
|----|------|
| 定位 | 本地 PDF 管理 + 扫描件 OCR + 中文全文搜索 |
| 栈 | Tauri 2 / Rust / Svelte 4 / Vite 5 / SQLite / Tantivy+jieba / oar-ocr(PP-OCRv5) |
| OCR 流 | `start_ocr` → 队列 → 渲染 → 识别(分块 1200/重叠 0.25/置信度 0.35) → 后处理 → 入库+索引 |
| 队列 | 单任务串行、不持久化（`task_queue.rs`） |

## 2. 问题清单

### P0 正确性

| ID | 问题 | 位置 |
|----|------|------|
| P0-1 | 工作区半成品可能无法编译（`PdfPageRenderFlags` 不存在等）→ **已决定回滚** | `ocr_service.rs`, `pdf_service.rs` |
| P0-2 | **队列断链**：`complete` 后 `get_next` 仅 emit，无人执行下一任务 | `commands/ocr.rs` |
| P0-3 | 部分页失败仍标 `done` | `commands/ocr.rs` final_status |
| P0-4 | force 清 `pdf_pages` 不删 Tantivy → 脏搜索 | `commands/ocr.rs` |
| P0-5 | 模型口径矛盾（Balanced/Mobile、150/300/500MB） | model_manager / memory_monitor / README |

### P1 性能

- `start_ocr` 同步跑整本、长持 Mutex
- 每页新建 Pdfium 实例
- 每页新建 IndexWriter + commit
- 分块去重 O(n²)
- 前端 3s 轮询（后端已有 `ocr-progress` 事件）
- 预览整页 base64 无缓存

### P2 安全 / 工程

- `csp: null`、生产 `devtools: true`、`withGlobalTauri`
- shell 插件/权限前端零引用
- `Cargo.lock` 被 gitignore
- 版本号 1.0.0 / 1.1.0 / 0.1.0 不一致
- README 模型清单与「跨平台」描述过时
- 大组件 FolderTree/PdfList；死代码 FolderService、sauvola、deskew 等

## 3. 分阶段计划

### Phase 0 — 止血（本轮执行）

1. [x] 方案落盘 + 更新 MEMORY/AGENTS
2. [x] `git checkout -- src-tauri/src/services/ocr_service.rs src-tauri/src/services/pdf_service.rs`
3. [x] **修队列**：任务完成后后端自动启动 `next`（不依赖前端）— Phase 5 真机验收1 通过
4. [x] **修状态**：存在失败页 → `error` + `error_message`（schema 无 partial）
5. [x] （若继续）force 同步 `delete_pdf` 索引 — Phase 5 真机验收2 通过；统一模型口径；Cargo.lock

**验收**：`cargo check` 通过；≥2 个 PDF 连续入队能自动串行跑完；有失败页时状态为 `error`。

### Phase 1 — OCR/后端性能 ✅ `a3ad94f`

- 批量索引、Pdfium 线程本地复用、同步命令线程模型

### Phase 2 — 搜索与数据 ✅ `2afb039`

- 索引 v5 安全重建（备份）；folder 过滤下推；唯一索引；WAL

### Phase 3 — 前端 ✅ `4cfdb9f`

- 事件驱动队列刷新；ocrProgress 最小更新；预览 LRU 缓存；CSP/devtools/shell 收紧

### Phase 4 — 工程化 ✅ `0d16d3c`

- 删 folder_service 空壳与 sauvola/deskew 死代码；修 README（平台/数据目录/更新日志）；版本 1.1.0；CI 不添加

### Phase 5 — 真机验收 ✅ 2026-09-26

报告：`Output/环境安装与OCR验证/2026-09-26-Phase5真机验收报告.md`（CDP 驱动真实应用执行，非 mock）

- 三项验收全 PASS：多 PDF 连续 OCR 自动串行 / force 重识别后旧词不可搜 / folder 过滤结果数正确
- 新发现并修复 2 个 P0（已真机复验 9/9，**待 commit**）：
  - **P0-6** 根级 `add_pdf` 持 `Db` 锁回调 `get_pdfs_dir()` 自死锁 → 全部 IPC 挂起（全新安装必现）
  - **P0-7** `enhance_contrast` 在留白扫描件上选出 254~255 伪动态范围 → 整页压黑 → OCR 空文本静默标 `done`（默认尺寸 1000 触发；已加动态范围保护 + 空白文本 WARN）

未启动候选：拆大组件（FolderTree/PdfList）/ 队列持久化 / OCR 单测套件 / 搜索 flake 排查。
新登记质量问题：Q-1 jieba 整词精确匹配致中文子串查询漏检；Q-2 OCR 截断长 ASCII 词。

## 4. 明确不做

- 默认启用 Sauvola/二值化（`ee239eb` 教训）
- 默认 Server 模型
- 无必要大重构

## 5. 用户已确认决策（2026-09-24）

1. 回滚两个半成品文件，不保留分块去重/抗锯齿未验证改动
2. 本轮只修队列与状态，然后再继续
3. 动手前先更新记忆文件并保存本方案

## 6. 当前状态与待办（2026-09-26 快照，本节为现役权威状态）

### 当前状态

| 维度 | 终态 |
|------|------|
| 分支 | `main` 工作树干净；本地在 origin 之上新增 1 个未推送 commit（清零待办，head 以 `git log` 为准，本节不内嵌哈希以免自指过期） |
| 进度 | Phase 0–5 + **待办 1–7 全部完成**；终轮真机 commit 含 Q-1/Q-2/拆组件/队列持久化/单测套件/可配置预处理/flake 取证（全量见 `git log 7feb241..HEAD`） |
| 缺陷 | P0-6/P0-7（历史）+ **P0-8 pdf-extract panic、P0-9 串行命令分发、P0-10 pdfium 二次初始化死锁 —— 均已修复并有回归测试** |
| 验证基线 | `cargo check` 零警告 · `cargo test --lib` **87/87**（+1 忽略态模型集成实跑通过）· `npm run build` 通过 · Phase 5 真机 9/9 · **真实文件功能测试 29/29 ALL PASS（15 份合同）** |
| 交付物 | `Output/环境安装与OCR验证/`（Phase 5 报告）+ `Output/待办执行与真机功能测试/`（待办清零报告、CDP 驱动器、结果 JSON；均受 `.gitignore` 保护） |
| 环境 | 编译前置 / Git 位置 / 代理 `127.0.0.1:7897` → 见 `AGENTS.md`「Rust 编译验证」与 `MEMORY.md`「用户工作背景」，此处不重复 |

### 待办（2026-09-26 晚：1–7 全部完成，详见 `2026-09-26-待办执行方案.md`）

1. [x] **Q-1 搜索质量**：jieba 整词精确匹配致中文子串查询漏检 → 索引 v6（单字+bigram 滑窗）+ 查询侧 bigram Must + 空索引回填（2026-09-26 完成，单测+真机语料召回验证）
2. [x] **Q-2 搜索质量**：OCR 截断长 ASCII 词 → **取证推翻「模型侧」结论**：真因是 2D 方块分块裁剪超宽文本行（dim≥1400）；改全宽横带分块 + 去重保留完整识别（2026-09-26 完成）
3. [x] **拆大组件**：FolderTree→SettingsPanel、PdfList→PdfListItem（776→536 / 673→453 行）
4. [x] **队列持久化**：`ocr_queue` 表 + 启动恢复 + 自动续跑 + 崩溃 processing 恢复
5. [x] **OCR 单测套件**：分块/去重/P0-7 回归/预处理模式 + `#[ignore]` 模型集成测试（57→88 用例）
6. [x] **可配置预处理**：`ocr_preprocess_mode`（auto/off/on）后端 + 设置面板
7. [x] **搜索测试 flake 排查**：40× 循环 0 失败（当前代码无复现），未做无根因改动

真机测试中另发现并修复：**P0-8** pdf-extract 真实 PDF 断言 panic（导入崩溃）、
**P0-9** 同步命令串行分发（OCR 期间 IPC 冻结/排队取消不可达）、**P0-10** pdfium 二次初始化死锁。

### 明确不做（沿用 §4 + 2026-09-26 决策）

- 恢复 CI workflows、默认 Sauvola/Server 模型、无必要大重构
- `PDF file/`（用户真实合同数据）、`docs/superpowers/` 历史计划：保持原样不动
