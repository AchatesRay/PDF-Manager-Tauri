# TODO

> 跨会话交接入口。每次整体重写，只反映当前待办，不累加历史。

## 当前状态（2026-10-27）

新增「内容搜索结果按 PDF 聚合 + 文件列表联动」功能（commit `13abd46`，已推送 main）：

- 前端 `src/lib/components/SearchResults.svelte`：内容搜索结果按 `pdf_id` 聚合，一个 PDF 只显示一行；行内显示文件名、该 PDF 总命中数、当前预览页片段；去掉 `P3` 页码徽标；上下条切换时片段与高亮同步更新。
- 结果行文件名列与片段列之间新增拖拽分隔条，宽度限制 80–420px，写入 `localStorage` 持久化。
- 新增 store `revealPdfId`（`src/lib/stores/index.ts`），搜索面板点击结果后通知 PDF 列表定位。
- `src/lib/components/PdfList.svelte` + `PdfListItem.svelte`：按 `data-pdf-id` 查找文件行，目标不在当前目录时先切换目录，再 `scrollIntoView` 并用描边短暂高亮。
- 文件名搜索结果点击也复用同一套列表定位逻辑。
- 不涉及后端搜索接口/索引结构变更；方案记录见 `docs/optimization/content-search-grouping.md`。
- 验证情况：本地 `npm run build` 通过（仅存量 a11y 警告）；GitHub Actions `build-windows`（run 40428256181）**成功**：Rust 单测、release 构建、便携包打包全部通过。
- 便携包：`Output/内容搜索聚合/pdf-manager-win-x64.zip`（22.9 MB，SHA256 `51FBF7F75125AF92C86DF4B12AC0ADC7C5BB4D52C21DAA85FDDF0337DBF4C8E0`），已解压到同目录 `pdf-manager-win-x64/`；exe 版本 1.2.0。
- 本机仍无 Rust / MSVC Build Tools，未做本地 `cargo check`；便携包由 GitHub Actions 构建。

## 待办

- [ ] 用户实测：内容搜索一个 PDF 多页命中只显示一行；点击结果自动切换目录并在 PDF 列表定位；拖动文件名列宽后重启应用保持。
- [ ] 旧待办：用户验收「导入文件夹 / 模型下载提示 / 目录分离 / 旧模型自动复制」。
- [ ] README「数据存储」章节仍待更新：`models/`、`logs/` 固定于安装目录，不随数据目录迁移。
- [ ] GitHub Release 尚未发布新 tag/版本（当前最新 Release 仍是 v1.2.0）。
- [ ] GitHub classic PAT 缺 `workflow` scope，改 `.github/workflows/*.yml` 的提交会被拒推。
- 推送通道备注：本机 github.com:443 时常不可达，可用 `git push ssh://git@ssh.github.com:443/AchatesRay/PDF-Manager-Tauri.git main`，推完执行 `git update-ref refs/remotes/origin/main <sha>` 同步本地引用。

## 接手指引

- 本次改动集中在 `src/lib/components/SearchResults.svelte`、`src/lib/components/PdfList.svelte`、`src/lib/components/PdfListItem.svelte`、`src/lib/stores/index.ts`。
- 前端构建：`npm run build`；后端验证需先 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"` 再 `cargo check --tests`。
- 内容搜索聚合要点：后端仍按页返回，前端用 `Map<number, PdfSearchGroup>` 按 `pdf_id` 聚合；保留每页原始结果与首次出现的扁平索引，供上下条匹配级导航复用。
- 列表定位要点：`revealPdfId` 只发一次请求，`PdfList` 消费后立即置空；跨目录判断使用 `folderContains`，已包含目标文件的父目录不重复切换。
