import { invoke } from '@tauri-apps/api/core';

export interface Folder {
  id: number;
  name: string;
  parent_id: number | null;
  storage_path: string | null;
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

export interface PdfDetail {
  id: number;
  folder_id: number | null;
  filename: string;
  original_path: string;
  storage_path: string;
  file_size: number;
  page_count: number;
  pdf_type: 'text' | 'scanned' | 'mixed';
  status: 'pending' | 'processing' | 'done' | 'error';
  error_message: string | null;
  created_at: string;
  updated_at: string;
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

export async function createFolder(name: string, parentId?: number, storagePath?: string): Promise<Folder> {
  return invoke('create_folder', { name, parentId, storagePath });
}

export async function renameFolder(id: number, name: string, storagePath?: string): Promise<void> {
  return invoke('rename_folder', { id, name, storagePath });
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

export async function getPdfDetail(pdfId: number): Promise<PdfDetail> {
  return invoke('get_pdf_detail', { pdfId });
}

// Search API
export async function search(query: string, folderId?: number): Promise<SearchResult[]> {
  return invoke('search', { query, folderId });
}

export async function searchFilename(query: string, folderId?: number): Promise<PdfInfo[]> {
  return invoke('search_filename', { query, folderId });
}

// OCR API
export async function getOcrStatus(): Promise<{ available: boolean; languages: string[] }> {
  return invoke('get_ocr_status');
}

export async function startOcr(pdfId: number): Promise<void> {
  return invoke('start_ocr', { pdfId });
}

// Settings API
export interface AppSettings {
  data_dir: string;
  log_dir: string;
  pdf_reader_path: string | null;
}

export async function getSettings(): Promise<AppSettings> {
  return invoke('get_settings');
}

export async function setDataDir(path: string): Promise<void> {
  return invoke('set_data_dir', { path });
}

export async function resetDataDir(): Promise<string> {
  return invoke('reset_data_dir');
}

export async function setPdfReader(path: string | null): Promise<void> {
  return invoke('set_pdf_reader', { path });
}

export async function openPdfExternally(pdfPath: string): Promise<void> {
  return invoke('open_pdf_externally', { pdfPath });
}