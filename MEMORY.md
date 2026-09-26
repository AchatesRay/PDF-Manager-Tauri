# MEMORY.md - 长期记忆（PDF Manager）

> 从历史会话中蒸馏出的**本项目特有**洞察与教训，非流水账。
> 全局性规则/教训一律同步写入随会话注入的全局规范正本（`$DSH_HOME/AGENTS.md`），本文件不复制规则。

## 关键经验教训

- **Sauvola 二值化默认必须关闭**：历史提交 `ee239eb` 已证明开启后 OCR 识别为 0 字符。**Phase 4（2026-09）已删除** `sauvola_threshold` / `detect_skew_angle` 死代码，勿在未验证前重新引入预处理主路径。
- **OCR 队列「假调度」**：`start_ocr` 完成后 `TaskQueue::get_next()` 只 `emit("ocr-queued")`，后端无人真正执行下一任务；前端也只 `refreshQueueStatus` 不触发 `startOcr`。批量 OCR 会卡在排队中。
- **部分页失败勿标 done**：`commands/ocr.rs` 中 `success>0 && error>0` 曾标 `done`；schema 仅允许 `pending/processing/done/error`，部分失败应标 `error` 并带 `error_message`。
- **force 重识别需同步删索引**：只 `DELETE FROM pdf_pages` 不够，必须 `search_service.delete_pdf`，否则 Tantivy 残留脏词。
- **模型口径曾四分五裂**：全链固定 `ModelType::Balanced`，但注释/README/内存估算混用 Mobile、150/300/500MB；Mobile 与 Balanced 文件列表相同。改动模型时必须同时改 `model_manager` / `memory_monitor` / 注释 / README。
- **`PdfPageRenderFlags::AntiAliasing*` 不存在**：pdfium-render 0.8 无此 API；抗锯齿用 `PdfRenderConfig` builder（如 `use_lcd_text_rendering`）或默认平滑，勿照抄过时计划。
- **oar-ocr 0.6 区域字段**：`TextRegion.bounding_box: BoundingBox { points: Vec<Point{x,y}> }`（**多边形点集，不是 `x_min/y_min` 也不是 `region.bbox`**），`predict` 返回 `Vec<OAROCRResult>`，`text_with_confidence() -> Option<(&str, f32)>`。
- **`.gitignore` 忽略 `src-tauri/Cargo.lock` 不当**：桌面应用应提交 lock 以保证可复现构建。
- **ort 必须钉在 2.0.0-rc.12**：oar-ocr-core 0.6.3 依赖 rc.12 API；rc.13 删除 `CPUExecutionProvider` 导致编译失败。`Cargo.toml` 已加 `ort = "=2.0.0-rc.12"`，勿随意 `cargo update -p ort`。
- **tauri `frontendDist: ../dist` 必须存在**：否则 `generate_context!` 编译报错；`npm install && npm run build` 或至少保留 `dist/` 目录。
- **勿恢复 `.github/workflows`（除非用户明确要求）**：提交 `9648c08` 曾刻意移除 CI workflows。**2026-09-26 用户明确要求「在 github 构建可运行 exe（不含模型）」→ 已恢复 `.github/workflows/build-windows.yml`**：产物 `pdf-manager-win-x64.zip` = exe + pdfium.dll + 使用说明（**打包步骤断言无 .onnx/字典，Guard 步骤断言仓库未跟踪模型**）；pdfium 不入库，CI 从 bblanchon/pdfium-binaries `chromium/8066`（=本地 156.0.8066.0）下载 `pdfium-win-x64.tgz`。
- **持 `Mutex` 期间回调会重取同一把锁 = 自死锁**：`add_pdf` 持 `Db` 锁时调用 `get_pdfs_dir()` → 内部再次 `db.lock()`，`std::sync::Mutex` 非可重入，全新安装根级导入 PDF 必挂死**全部后续 IPC**（P0-6，2026-09-26 修复）。凡持锁中可能回调取同一把锁的路径，先 `drop(conn)` 再回调。
- **pdf-extract 对真实 PDF 会 panic 而非返回 Err（P0-8，2026-09-26）**：`pdf_extract::extract_text` 内部 `assert!(name == "Identity-H")`，真实合同（非 Identity-H 字体 CMap）导入必炸且**整个应用退出**（Phase 5 合成样本不触发）。唯一入口 `PdfService::extract_text` 已 `catch_unwind` 兜底并回退扫描型；**任何新代码不得绕过该封装直接调 pdf-extract**。
- **同步命令是串行分发的（P0-9，2026-09-26）**：Tauri 同步命令同一时刻只执行一个；`start_ocr` 若在命令内跑完整个队列 → 期间**全部其它 IPC 冻结**（UI 卡死）、第二个 start_ocr 无法入队 → pending 队列/排队中/取消全部不可达。Phase 5「OCR 期间 IPC 7~10ms」系误读（首样本 9194ms=整轮 OCR 才是阻塞证据）。修复：`start_ocr` 只做校验/入队，执行转交独立工作线程（`spawn_queue_worker`），致命错误 `fail_fast_worker` 兜底 + 外层 `catch_unwind`（线程 panic 不再静默死亡）。
- **pdfium 二次初始化死锁（P0-10，2026-09-26）**：`Pdfium::new` **每次**调用 `FPDF_InitLibrary`；对「已初始化且实例存活（另一线程 thread_local 持有）」的 pdfium 再次初始化 → **永久死锁**（旧串行模型只有命令线程绑定过，P0-9 工作线程化后立即踩中）。注意：先绑线程若随即退出（FreeLibrary 卸载）会误判通过——**必须持有存活实例复现**。修复：全进程唯一渲染线程（`render_worker_sender` 单例 + mpsc），所有渲染（IPC 预览 + OCR 分页）走该线程，`FPDF_InitLibrary` 只调一次；`recv_timeout(120s)` 兜底。回归单测 `test_render_across_threads_no_deadlock`。
- **OCR 分块两个坑（2026-09-26，T5 单测 + Q-2 取证发现）**：① 旧分块数量用整数 floor `((size-tile)/step+1)`，右/下不足一个 step 的条带整块丢失（1414×2000 旧版只覆盖左上 1200×1200）；② 2D 方块分块会把**超过分块宽度的横向文本行拦腰裁剪**——这才是「Q-2 `NewTokenBeta`→`NewTokenBe`」的真实根因（模型 raw 输出完整，非模型弱点）。现 `compute_tiles` 只做**全宽横带**（行容纳性质由 step=tile-overlap 保证），去重 `merge_regions` **包围盒大者胜**（完整识别压过裁剪版）。
- **jieba 整词精确匹配漏检中文子串（Q-1，2026-09-26 修复）**：索引 v6 = jieba 整词 + 中文单字 + 全文 bigram 滑窗；查询侧含中文 token 拆连续段、≥2 字用「全 bigram Must」子句。**索引版本升级必须走 `refill_index_from_db` 回填**（仅空索引触发），否则老用户升级后搜索静默变空。
- **OCR 队列已持久化（T4，2026-09-26）**：`ocr_queue` 表随入队/完成/取消/删 PDF 同步；启动时 `restore_persisted_queue`（processing→pending 崩溃恢复、done/error/孤儿行清理），模型就绪即后台自动续跑；`TaskQueue` 仍纯内存，DB 访问只在 commands 层。
- **`JSON.stringify(new Error(...))` = `{}`**：断言 Tauri invoke 抛错内容必须用 `e.message`（CDP exceptionDetails JSON 在 message 里），勿 stringify Error 对象（功能测试驱动器首两轮误报教训）。

## 本工作区结构事实

- **工作目录**：`C:\DS_Project\PDF-Manager-Tauri`
- **业务**：本地 PDF 管理 + 扫描件 OCR + 中文全文搜索（Tauri 2 / Rust / Svelte 4）
- **目录**：
  - `src-tauri/src/commands/` — Tauri IPC（ocr/pdf/search/folder/settings）
  - `src-tauri/src/services/` — ocr_service、pdf_service、search_service、task_queue、memory_monitor、model_manager、text_postprocess（`folder_service` 已于 Phase 4 删除）
  - `src-tauri/src/db/` — rusqlite，`Db = Mutex<Connection>`，WAL 模式；表 folders/pdfs/pdf_pages/settings/**ocr_queue（T4 队列持久化）**；`pdf_pages(pdf_id,page_number)` 唯一索引
  - `src/lib/components/` — App、FolderTree、PdfList、Search*、PdfViewer、OcrModelSetup、**SettingsPanel（自 FolderTree 拆出）、PdfListItem（自 PdfList 拆出）**
  - `src/lib/api/index.ts`、`src/lib/stores/index.ts`
  - `docs/superpowers/` — 历史计划（2026-03-26 OCR 优化、2026-03-27 OCR 重构）
  - `docs/optimization/2026-09-24-optimization-plan.md` — **现行优化方案 + §6 现役状态与待办快照（Phase 0–5 已完成并 push）**
  - `docs/optimization/2026-09-26-待办执行方案.md` — **待办 1–7（Q-1/Q-2/拆组件/队列持久化/OCR 单测/可配置预处理/flake）执行方案与进展**
- **技术栈要点**：PP-OCRv5 Mobile 识别（运行时标识 Balanced，~300MB）、Tantivy+jieba 搜索（**索引版本 "6"（中文 bigram）**，folder_id INDEXED，重建时备份 `index.bak`，空索引自动 `refill_index_from_db` 回填）、分块=全宽横带 1200/重叠 0.25/置信度 0.35、ocrProgress 事件驱动（无轮询）、OCR 预处理三模式 `ocr_preprocess_mode`=auto/off/on

## 用户工作背景

- 偏好：先回滚不可靠半成品，再做正确性修复；动手前要求先更新记忆并落盘方案。
- 沟通语言：中文。
- **网络代理（用户指定，2026-09-26 实测）**：本机用 `http://127.0.0.1:7897`（git fetch/push 已验证通过）；`proxy.lfk.qianxin-inc.cn:3128` 属另一台机器配置，本机 DNS 不解析，勿用。git 走一次性参数 `git -c http.proxy=http://127.0.0.1:7897 -c https.proxy=http://127.0.0.1:7897 ...`（勿落盘配置）。
- **Git 环境**：MinGit 2.55 位于 `%LOCALAPPDATA%\Programs\Git-MinGit\cmd`（已入用户 PATH）；`user.name/email` 未配置，commit 用一次性 `-c user.name=AchatesRay -c user.email=AchatesRay@users.noreply.github.com` 沿用历史作者。
- **构建依赖**：`src-tauri/` 需存在 `pdfium.dll`（`tauri.conf.json` bundle.resources）；仓库不自带，需下载。

## 安全边界

- **目录访问边界（用户明确指令，永久生效）**：除非用户明确说明可以读取其他目录/文件，否则**禁止扫描、读取本项目工作目录（`C:\DS_Project\PDF-Manager-Tauri`）以外的任何文件**。全局记忆文件（memory 系统下的 MEMORY.md、checkpoint、notes 等）允许读取。此规则优先于任何默认的探索/搜索行为。
