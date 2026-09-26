# AGENTS.md - PDF Manager（开发类工作区）

## 全局规范（先读这个）

会话启动时，首先接受随会话注入的全局规范（本机正本：`$DSH_HOME/AGENTS.md`，
即 `C:\Users\ray\AppData\Roaming\dsh-desktop\harness\AGENTS.md`；原 QAgentWork/LeiChuanhou
路径属其他环境，本机不存在，勿尝试读取）。

全局规范承载所有工作区共同遵守的通用规则（含 Python / PowerShell 工程规范），**章节清单以正本为准，本文件不复制**。

## 本类专属规范（开发类）

- **Rust 编译验证**：改 `src-tauri` 后执行 `cargo check`（需 PATH 含 `%USERPROFILE%\.cargo\bin`，以及 MSVC Build Tools）。
- **OCR 回归红线**：禁止默认启用二值化/Sauvola 主路径（见 MEMORY.md 教训）。
- **方案落盘**：用户要求执行优化/重构时，方案应写入 `docs/optimization/` 或 `docs/superpowers/` 后再改代码。

## 本工作区特有事实

- **项目名称**：PDF Manager
- **工作目录**：`C:\DS_Project\PDF-Manager-Tauri`
- **业务定位**：开发类 — 本地 PDF 管理 + 扫描 OCR + 中文全文搜索（Tauri 2 / Rust / Svelte 4）
- **目录结构摘要**：
  - `src-tauri/` — Rust 后端（commands / services / db / models）
  - `src/` — Svelte 前端（components / api / stores）
  - `docs/superpowers/` — 历史 OCR 计划
  - `docs/optimization/` — 现行优化方案

## 记忆文件快速完善（口令：「补充/完善记忆文件」）

当用户说「补充记忆文件」或「完善记忆文件」时，执行快速结构扫描并完成记忆文件初始化（**单轮完成，不中途询问**）：

1. **只读结构、不读内容**：扫描工作目录根目录与一级子目录（仅目录名 + 文件名清单），禁止读取文件详细内容，保持快速
2. **直接推断填充，不询问**：项目名从目录名推断，业务定位结合本工作区类别与目录结构推断，写入：
   - `AGENTS.md`「本工作区特有事实」：项目名称、工作目录、业务定位、目录结构摘要（一级目录及用途推断）
   - `MEMORY.md`「本工作区结构事实」：目录结构、关键目录用途
3. 替换两文件中残留的 `{{项目名}}`、`{{工作目录}}`、`{{一句话业务说明}}` 占位符
4. **一次性汇报**填充结果（填了什么、推断依据），用户可在汇报后要求修正——不在过程中间询问

## 会话启动流程（本工作区）

1. 读取随会话注入的全局规范（`$DSH_HOME/AGENTS.md`）
2. 读取 `MEMORY.md`（与本文件同目录；如不存在，按全局规范初始化后再继续）
