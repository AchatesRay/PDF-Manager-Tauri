/**
 * 「关于」页签数据（2026-09-28 统一设置卡片）
 *
 * 版本号不在这里维护：由 SettingsDialog 调 getVersion() 读 tauri.conf.json，
 * 本文件只提供 版本→发布日期 映射与文案数据。
 * 每次发版：改 tauri.conf.json 的 version + 在 RELEASES 加一行（日期同步）。
 */

// 版本 → 发布日期映射（与 tauri.conf.json 的 version 同步）
export const RELEASES: Record<string, string> = {
  '1.2.0': '2026-09-28',
  '1.1.0': '2026-09-28',
};

// 功能简介要点
export const APP_FEATURES: { title: string; desc: string }[] = [
  { title: '本地文档管理', desc: '文件夹树分类，PDF 导入、移动、重命名、批量管理' },
  { title: '扫描 OCR 识别', desc: '中文识别任务队列、进度显示、多模型切换（Mobile/Balanced 等）' },
  { title: '中文全文搜索', desc: '正文全文搜索 + 文件名搜索，命中结果预览高亮定位' },
  { title: '文档阅读', desc: '内置分页预览，也可调用外部阅读器打开' },
];

// 说明（技术栈 / 隐私 / 致谢）
export const APP_NOTES: string[] = [
  '技术栈：Tauri 2 + Rust + Svelte 4，Windows 桌面应用。',
  '数据全部保存在本地（数据目录见「通用」页），文档与 OCR 内容不上传任何服务器。',
  'OCR 模型来自 oar-ocr（PP-OCRv5），PDF 渲染基于 pdfium。',
];

// 相关链接（可点开）
export const APP_LINKS: { label: string; url: string }[] = [
  { label: 'OCR 模型下载页', url: 'https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0' },
];
