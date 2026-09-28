import { writable, derived } from 'svelte/store';
import type { Folder, PdfInfo, SearchResult, OcrStatus, DownloadProgress, QueueStatus, MatchRect } from '../api';

export const selectedFolderId = writable<number | null>(null);
export const folders = writable<Folder[]>([]);
export const pdfList = writable<PdfInfo[]>([]);
export const selectedPdfId = writable<number | null>(null);
export const selectedPdfPath = writable<string | null>(null);
export const selectedPdfPageCount = writable<number>(0);
export const searchResults = writable<SearchResult[]>([]);
export const searchQuery = writable('');
// 搜索提交计数：每次执行搜索 +1。SearchResults 据此重置导航并拉取新高亮，
// 替代旧的「结果条数变化」判断（同条数/零结果的新搜索不会刷新的缺陷）
export const searchRevision = writable(0);
export const isLoading = writable(false);
export const showSearchResults = writable(false);

// 搜索模式
export type SearchMode = 'content' | 'filename';
export const searchMode = writable<SearchMode>('content');
export const filenameSearchResults = writable<PdfInfo[]>([]);

// 搜索结果跳转页码
export const jumpToPage = writable<number | null>(null);

// 搜索预览高亮：当前搜索在预览页上的命中矩形（0~1 归一化）
// null = 无搜索/已清除；pdfId/page 标识命中所在页，rects 按阅读顺序，
// activeIndex = 结果列表的页内序号（导航切换焦点）
export const pageHighlight = writable<{
  pdfId: number;
  page: number;
  query: string;
  rects: MatchRect[];
  activeIndex: number;
} | null>(null);

// OCR 进度
export interface OcrProgress {
  pdf_id: number;
  current: number;
  total: number;
  status: string;
}
export const ocrProgress = writable<Map<number, OcrProgress>>(new Map());

// OCR 任务队列
export const ocrQueue = writable<QueueStatus>({ current: null, pending: [] });

// OCR 模型状态
export const ocrModelStatus = writable<OcrStatus | null>(null);
export const ocrDownloadProgress = writable<DownloadProgress | null>(null);
export const isDownloading = writable(false);

export const filteredPdfList = derived(
  [pdfList, selectedFolderId],
  ([$pdfList, $selectedFolderId]) => {
    if ($selectedFolderId === null) {
      return $pdfList;
    }
    return $pdfList.filter(pdf => pdf.folder_id === $selectedFolderId);
  }
);


// 是否显示下载对话框
export const showDownloadDialog = writable(false);

// 统一设置弹窗：开关 + 当前页签（2026-09-28 统一设置卡片，替代原 showModelPanel）
// 开关由 FolderTree 的 ⚙ 按钮驱动，弹窗挂在 App 根部，必须用 store 跨组件传递
export const showSettings = writable(false);
export type SettingsTab = 'general' | 'model' | 'about';
export const settingsTab = writable<SettingsTab>('general');