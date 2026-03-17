import { writable, derived } from 'svelte/store';
import type { Folder, PdfInfo, SearchResult } from '../api';

export const selectedFolderId = writable<number | null>(null);
export const folders = writable<Folder[]>([]);
export const pdfList = writable<PdfInfo[]>([]);
export const selectedPdfId = writable<number | null>(null);
export const selectedPdfPath = writable<string | null>(null);
export const searchResults = writable<SearchResult[]>([]);
export const searchQuery = writable('');
export const isLoading = writable(false);
export const showSearchResults = writable(false);

// 搜索模式
export type SearchMode = 'content' | 'filename';
export const searchMode = writable<SearchMode>('content');
export const filenameSearchResults = writable<PdfInfo[]>([]);

export const filteredPdfList = derived(
  [pdfList, selectedFolderId],
  ([$pdfList, $selectedFolderId]) => {
    if ($selectedFolderId === null) {
      return $pdfList;
    }
    return $pdfList.filter(pdf => pdf.folder_id === $selectedFolderId);
  }
);