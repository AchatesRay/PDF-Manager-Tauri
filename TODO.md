# TODO

> 跨会话交接入口。每次整体重写，只反映当前待办，不累加历史。

## 当前状态（2026-10-26）

新增「导入本地已有 PDF 文件夹」功能（本地改动，尚未提交）：

- 后端 `src-tauri/src/commands/import.rs`：选择文件夹后按源目录名创建分类（`parent_id` 可指定父分类），递归扫描 PDF、镜像子目录结构、同名目录复用、重复文件跳过，复制入库；导入过程通过 `folder-import-progress` 事件推送进度。
- 前端 `src/lib/components/PdfList.svelte`：PDF 列表头部新增「导入文件夹」按钮，默认导入到当前选中分类下（未选中时放根级），导入时显示 `当前/总数` 进度，结束后汇总成功/跳过/失败并定位到新分类。
- 前端 `src/lib/components/FolderTree.svelte`：选中项变化时自动展开祖先目录，保证导入后选中新分类在树中可见。
- 复用改造：`commands/pdf.rs` 抽出 `add_pdf_impl`，单文件添加与文件夹导入共用同一套查重/复制/元数据/入库逻辑。
- 验证情况：`npm run build` 通过（仅存量 a11y 警告）；**本机未安装 Rust / MSVC Build Tools，`cargo check` 未执行**。

## 待办

- [ ] 在有 Rust + MSVC 的环境执行 `cargo check --tests`，并跑 `cargo test --lib -- import::tests`。
- [ ] 用户实测：选择一个含多层子目录的 PDF 文件夹导入，确认目录名、目录结构、重复导入跳过行为。
- [ ] 验证通过后提交并推送（本机无 cargo，仅前端可验证）。
- [ ] 旧待办：用户验收「模型下载提示 / 目录分离 / 旧模型自动复制」四项。
- [ ] README「数据存储」章节仍待更新：`models/`、`logs/` 固定于安装目录，不随数据目录迁移。
- [ ] GitHub Release 尚未发布新 tag/版本（当前最新 Release 仍是 v1.2.0）。
- [ ] GitHub classic PAT 缺 `workflow` scope，改 `.github/workflows/*.yml` 的提交会被拒推。
- 推送通道备注：本机 github.com:443 时常不可达，可用 `git push ssh://git@ssh.github.com:443/AchatesRay/PDF-Manager-Tauri.git main`，推完执行 `git update-ref refs/remotes/origin/main <sha>` 同步本地引用。

## 接手指引

- 本次改动集中在 `src-tauri/src/commands/{import,pdf,mod}.rs`、`src-tauri/src/lib.rs`、`src/lib/api/index.ts`、`src/lib/components/PdfList.svelte`。
- 前端构建：`npm run build`；后端验证需先 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"` 再 `cargo check --tests`。
- 导入功能设计要点：顶级目录名 = 源文件夹名；子目录按相对路径镜像；同名同父目录复用（不创建 `xxx (2)`）；已导入文件按 `original_path` / 同目录文件名查重跳过。
