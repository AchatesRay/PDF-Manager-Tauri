import { writable, derived } from 'svelte/store';
import type { Folder, PdfInfo, SearchResult, OcrStatus, DownloadProgress } from '../api';

export const selectedFolderId = writable<number | null>(null);
export const folders = writable<Folder[]>([]);
export const pdfList = writable<PdfInfo[]>([]);
export const selectedPdfId = writable<number | null>(null);
export const selectedPdfPath = writable<string | null>(null);
export const selectedPdfPageCount = writable<number>(0);
export const searchResults = writable<SearchResult[]>([]);
export const searchQuery = writable('');
export const isLoading = writable(false);
export const showSearchResults = writable(false);

// 搜索模式
export type SearchMode = 'content' | 'filename';
export const searchMode = writable<SearchMode>('content');
export const filenameSearchResults = writable<PdfInfo[]>([]);

// 搜索结果跳转页码
export const jumpToPage = writable<number | null>(null);

// OCR 进度
export interface OcrProgress {
  pdf_id: number;
  current: number;
  total: number;
  status: string;
}
export const ocrProgress = writable<Map<number, OcrProgress>>(new Map());

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