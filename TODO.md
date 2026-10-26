# TODO

> 跨会话交接入口。每次整体重写，只反映当前待办，不累加历史。

## 当前状态（2026-10-26）

两项需求已实现、验证、提交并推送（commit `32b8aa5`，main），CI 构建成功（run #16），本地便携版 zip 已生成，等待用户实测验收：

1. **模型下载失败提示**：设置 → 模型 页的错误提示可手动关闭（带 ✕ 按钮）。
2. **设置 → 模型 页直接下载**：新增「下载模型 / 取消下载 / 刷新」按钮与进度条，与首次启动引导页共用同一套后端下载逻辑。
3. **目录分离**：日志（`logs/`）与 OCR 模型（`models/`）固定存放在安装目录；切换数据目录只迁移 `pdfs/`、`thumbnails/`、`index/` 与数据库，不再搬运日志和模型。
4. **旧模型自动复制**：若旧数据目录里已有模型、而安装目录 `models/` 为空，启动时自动复制一份过去（旧文件保留，不删除）。

## 待办

- [ ] 用户验收上述 1–4 项：用 `Output/模型下载与目录分离/pdf-manager-win-x64.zip` 解压实测。
- [ ] 待用户确认后，更新 `README.md`「数据存储」章节：`models/`、`logs/` 现固定于安装目录，不随数据目录迁移（README 已过时，尚未改动）。
- [ ] GitHub Release 尚未发布新 tag/版本（当前最新 Release 仍是 v1.2.0）；如要发布需用户明确指示。
- [ ] GitHub classic PAT 缺 `workflow` scope，改 `.github/workflows/*.yml` 的提交会被拒推（本次已回滚一处 CI 文案修正，改在本地打包脚本内生效）。

## 接手指引

- 改动集中在 `src-tauri/src/commands/settings.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/db/mod.rs`、`src-tauri/src/services/ocr_service.rs`、`src/lib/components/{OcrModelSetup,ModelManagerPanel,SettingsPanel}.svelte`。
- 打包脚本：`Output/模型下载与目录分离/Temp/package.ps1`（须 UTF-8 with BOM，经 `powershell -ExecutionPolicy Bypass -File` 运行；PS 5.1 默认 Restricted）。
- 验证方式：前端 `npm run build`；后端 `cargo check --tests`；单测 `cargo test --lib -- settings::tests`（共 9 个用例，需把 TEMP/TMP 指到工作区内目录，系统临时目录在本机被拒写）；便携版冒烟：`cmd /c start "" <exe>` 后进程存在且安装目录出现 index/logs/models/pdfs/thumbnails 即正常（`Start-Process` 在沙箱内会挂起）。
- 过程产物与备份：`Output/模型下载与目录分离/`（`backup/` 为改动前原文件）。
