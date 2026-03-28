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
  match_count: number;
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
export interface OcrStatus {
  available: boolean;
  models_ready: boolean;
  missing_files: string[];
  models_dir: string;
  model_type: string;
}

export interface DownloadProgress {
  file: string;
  current: number;
  total: number;
}

export interface DownloadGuide {
  name: string;
  url: string;
  size: string;
}

export async function getOcrStatus(): Promise<OcrStatus> {
  return invoke('get_ocr_status');
}

export async function refreshOcrStatus(): Promise<OcrStatus> {
  return invoke('refresh_ocr_status');
}

export async function getOcrDownloadGuide(): Promise<DownloadGuide[]> {
  return invoke('get_ocr_download_guide');
}

export async function downloadOcrModels(): Promise<void> {
  return invoke('download_ocr_models');
}

export async function cancelOcrDownload(): Promise<void> {
  return invoke('cancel_ocr_download');
}

export async function startOcr(pdfId: number, force: boolean = false): Promise<void> {
  return invoke('start_ocr', { pdfId, force });
}

// Queue API
export interface QueueStatus {
  current: number | null;
  pending: Array<{ pdf_id: number; created_at: string }>;
}

export async function getOcrQueueStatus(): Promise<QueueStatus> {
  return invoke('get_ocr_queue_status');
}

export async function cancelOcrTask(pdfId: number): Promise<boolean> {
  return invoke('cancel_ocr_task', { pdfId });
}

export interface MemoryInfo {
  total: number;
  available: number;
  used_percent: number;
}

export async function getMemoryInfo(): Promise<MemoryInfo> {
  return invoke('get_memory_info');
}

// Settings API
export interface AppSettings {
  data_dir: string;
  log_dir: string;
  pdf_reader_path: string | null;
  ocr_max_image_dimension: number;
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

export async function setOcrMaxImageDimension(dimension: number): Promise<void> {
  return invoke('set_ocr_max_image_dimension', { dimension });
}

export async function openPdfExternally(pdfPath: string): Promise<void> {
  return invoke('open_pdf_externally', { pdfPath });
}

// PDF Preview API
export async function renderPdfPage(pdfId: number, pageNum: number): Promise<string> {
  return invoke('render_pdf_page', { pdfId, pageNum });
}