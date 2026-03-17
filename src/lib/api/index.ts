import { invoke } from '@tauri-apps/api/core';

export interface Folder {
  id: number;
  name: string;
  parent_id: number | null;
  created_at: string;
  updated_at: string;
}

export interface PdfInfo {
  id: number;
  folder_id: number | null;
  filename: string;
  page_count: number;
  pdf_type: 'text' | 'scanned' | 'mixed';
  status: 'pending' | 'processing' | 'done' | 'error';
  progress?: number;
}

export interface SearchResult {
  page_id: number;
  pdf_id: number;
  folder_id: number | null;
  page_number: number;
  filename: string;
  score: number;
  snippet: string;
}

// Folder APIs
export async function getFolders(): Promise<Folder[]> {
  return invoke('get_folders');
}

export async function createFolder(name: string, parentId?: number): Promise<Folder> {
  return invoke('create_folder', { name, parentId });
}

export async function renameFolder(id: number, name: string): Promise<void> {
  return invoke('rename_folder', { id, name });
}

export async function deleteFolder(id: number): Promise<void> {
  return invoke('delete_folder', { id });
}

// PDF APIs
export async function addPdf(path: string, folderId?: number): Promise<PdfInfo> {
  return invoke('add_pdf', { path, folderId });
}

export async function getPdfList(folderId?: number): Promise<PdfInfo[]> {
  return invoke('get_pdf_list', { folderId });
}

export async function deletePdf(pdfId: number): Promise<void> {
  return invoke('delete_pdf', { pdfId });
}

// Search API
export async function search(query: string, folderId?: number): Promise<SearchResult[]> {
  return invoke('search', { query, folderId });
}

// OCR API
export async function getOcrStatus(): Promise<{ available: boolean; languages: string[] }> {
  return invoke('get_ocr_status');
}