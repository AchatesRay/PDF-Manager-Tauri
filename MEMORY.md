# MEMORY.md - 长期记忆（PDF Manager）

> 从历史会话中蒸馏出的**本项目特有**洞察与教训，非流水账。
> 全局性规则/教训一律同步写入 AGENTS-GLOBAL.md 正本，本文件不复制规则。

## 关键经验教训

- **Sauvola 二值化默认必须关闭**：历史提交 `ee239eb` 已证明开启后 OCR 识别为 0 字符；`sauvola_threshold` / `detect_skew_angle` 现为死代码，勿在未验证前重新接入预处理主路径。
- **OCR 队列「假调度」**：`start_ocr` 完成后 `TaskQueue::get_next()` 只 `emit("ocr-queued")`，后端无人真正执行下一任务；前端也只 `refreshQueueStatus` 不触发 `startOcr`。批量 OCR 会卡在排队中。
- **部分页失败勿标 done**：`commands/ocr.rs` 中 `success>0 && error>0` 曾标 `done`；schema 仅允许 `pending/processing/done/error`，部分失败应标 `error` 并带 `error_message`。
- **force 重识别需同步删索引**：只 `DELETE FROM pdf_pages` 不够，必须 `search_service.delete_pdf`，否则 Tantivy 残留脏词。
- **模型口径曾四分五裂**：全链固定 `ModelType::Balanced`，但注释/README/内存估算混用 Mobile、150/300/500MB；Mobile 与 Balanced 文件列表相同。改动模型时必须同时改 `model_manager` / `memory_monitor` / 注释 / README。
- **`PdfPageRenderFlags::AntiAliasing*` 不存在**：pdfium-render 0.8 无此 API；抗锯齿用 `PdfRenderConfig` builder（如 `use_lcd_text_rendering`）或默认平滑，勿照抄过时计划。
- **oar-ocr 0.6 区域字段**：`TextRegion.bounding_box: BoundingBox`（`x_min/y_min/x_max/y_max`），不是旧计划里的 `region.bbox`；`predict` 返回 `Vec<OAROCRResult>`。
- **`.gitignore` 忽略 `src-tauri/Cargo.lock` 不当**：桌面应用应提交 lock 以保证可复现构建。
- **ort 必须钉在 2.0.0-rc.12**：oar-ocr-core 0.6.3 依赖 rc.12 API；rc.13 删除 `CPUExecutionProvider` 导致编译失败。`Cargo.toml` 已加 `ort = "=2.0.0-rc.12"`，勿随意 `cargo update -p ort`。
- **tauri `frontendDist: ../dist` 必须存在**：否则 `generate_context!` 编译报错；`npm install && npm run build` 或至少保留 `dist/` 目录。

## 本工作区结构事实

- **工作目录**：`D:\Qagent\Project\PDF-Manager-Tauri`
- **业务**：本地 PDF 管理 + 扫描件 OCR + 中文全文搜索（Tauri 2 / Rust / Svelte 4）
- **目录**：
  - `src-tauri/src/commands/` — Tauri IPC（ocr/pdf/search/folder/settings）
  - `src-tauri/src/services/` — ocr_service、pdf_service、search_service、task_queue、memory_monitor、model_manager、text_postprocess；`folder_service` 为空壳
  - `src-tauri/src/db/` — rusqlite，`Db = Mutex<Connection>`；表 folders/pdfs/pdf_pages/settings
  - `src/lib/components/` — App、FolderTree、PdfList、PdfViewer、Search*、OcrModelSetup
  - `src/lib/api/index.ts`、`src/lib/stores/index.ts`
  - `docs/superpowers/` — 历史计划（2026-03-26 OCR 优化、2026-03-27 OCR 重构）
  - `docs/optimization/2026-09-24-optimization-plan.md` — **现行优化方案**
- **技术栈要点**：PP-OCRv5 Mobile 识别（检查/下载路径标 Balanced）、Tantivy+jieba 搜索（索引版本 "4"）、分块 1200/重叠 0.25/置信度 0.35

## 用户工作背景

- 偏好：先回滚不可靠半成品，再做正确性修复；动手前要求先更新记忆并落盘方案。
- 沟通语言：中文。
- **网络代理（用户指定）**：外网不可达时用 `http://proxy.lfk.qianxin-inc.cn:3128`（`curl -x` / `Invoke-WebRequest -Proxy`）。
- **构建依赖**：`src-tauri/` 需存在 `pdfium.dll`（`tauri.conf.json` bundle.resources）；仓库不自带，需下载。

## 安全边界

- **目录访问边界（用户明确指令，永久生效）**：除非用户明确说明可以读取其他目录/文件，否则**禁止扫描、读取本项目工作目录（`D:\Qagent\Project\PDF-Manager-Tauri`）以外的任何文件**。全局记忆文件（memory 系统下的 MEMORY.md、checkpoint、notes 等）允许读取。此规则优先于任何默认的探索/搜索行为。
