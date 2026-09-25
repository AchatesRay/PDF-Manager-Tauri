# PDF Manager 优化方案（2026-09-24）

> 状态：**Phase 0–4 全部完成**。CI 不添加（workflows 曾于 `9648c08` 被刻意移除）。
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
2. [ ] `git checkout -- src-tauri/src/services/ocr_service.rs src-tauri/src/services/pdf_service.rs`
3. [ ] **修队列**：任务完成后后端自动启动 `next`（不依赖前端）
4. [ ] **修状态**：存在失败页 → `error` + `error_message`（schema 无 partial）
5. [ ] （若继续）force 同步 `delete_pdf` 索引；统一模型口径；Cargo.lock

**验收**：`cargo check` 通过；≥2 个 PDF 连续入队能自动串行跑完；有失败页时状态为 `error`。

### Phase 1 — OCR/后端性能 ✅ `a3ad94f`

- 批量索引、Pdfium 线程本地复用、同步命令线程模型

### Phase 2 — 搜索与数据 ✅ `2afb039`

- 索引 v5 安全重建（备份）；folder 过滤下推；唯一索引；WAL

### Phase 3 — 前端 ✅ `4cfdb9f`

- 事件驱动队列刷新；ocrProgress 最小更新；预览 LRU 缓存；CSP/devtools/shell 收紧

### Phase 4 — 工程化 ✅

- 删 folder_service 空壳与 sauvola/deskew 死代码；修 README（平台/数据目录/更新日志）；版本 1.1.0；CI 不添加

## 4. 明确不做

- 默认启用 Sauvola/二值化（`ee239eb` 教训）
- 默认 Server 模型
- 无必要大重构

## 5. 用户已确认决策（2026-09-24）

1. 回滚两个半成品文件，不保留分块去重/抗锯齿未验证改动
2. 本轮只修队列与状态，然后再继续
3. 动手前先更新记忆文件并保存本方案
