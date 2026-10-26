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

/** 导入本地已有 PDF 文件夹的结果 */
export interface FolderImportFailure {
  path: string;
  reason: string;
}

export interface FolderImportReport {
  folder_id: number;
  folder_name: string;
  total_pdfs: number;
  imported: number;
  skipped: string[];
  failed: FolderImportFailure[];
  created_folders: number;
  reused_folder: boolean;
}

/** 导入文件夹进度事件 */
export interface FolderImportProgress {
  current: number;
  total: number;
  filename: string;
  relative_dir: string;
}

/**
 * 导入本地已有 PDF 文件夹。
 * 顶级目录名使用所选文件夹的名称，子目录结构会一并镜像。
 */
export async function importPdfFolder(path: string, parentId?: number): Promise<FolderImportReport> {
  return invoke('import_pdf_folder', { path, parentId });
}

export async function getPdfList(folderId?: number): Promise<PdfInfo[]> {
  return invoke('get_pdf_list', { folderId });
}

export async function deletePdf(pdfId: number): Promise<void> {
  return invoke('delete_pdf', { pdfId });
}

// 批量移动
export interface MoveReport {
  moved: number[];
  skipped: Array<[number, string]>;
}

export async function movePdfs(pdfIds: number[], targetFolderId?: number): Promise<MoveReport> {
  return invoke('move_pdfs', { pdfIds, targetFolderId });
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

/** 命中位置的归一化矩形（0~1，相对页面宽高）——搜索预览高亮 */
export interface MatchRect {
  x0: number;
  y0: number;
  x1: number;
  y1: number;
}

/** 获取某页命中关键字的归一化矩形（旧数据无坐标返回 []） */
export async function getPageMatches(
  pdfId: number,
  pageNumber: number,
  query: string
): Promise<MatchRect[]> {
  return invoke('get_page_matches', { pdfId, pageNumber, query });
}

/** 某页的识别质量信息 */
export interface PageOcrInfo {
  /** null = 该页还没有识别记录 */
  ocr_status: string | null;
  /** null = 升级前的旧数据，没有记录（不代表没丢字） */
  low_conf: number | null;
  /** 是否保存了可高亮的坐标（旧数据为 false） */
  has_regions: boolean;
}

export async function getPageOcrInfo(
  pdfId: number,
  pageNumber: number
): Promise<PageOcrInfo> {
  return invoke('get_page_ocr_info', { pdfId, pageNumber });
}

// OCR API
export interface OcrStatus {
  available: boolean;
  models_ready: boolean;
  missing_files: string[];
  models_dir: string;
  /** 当前启用的模型（null = 已禁用） */
  active_model: string | null;
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

export async function getOcrDownloadGuide(modelType?: string): Promise<DownloadGuide[]> {
  return invoke('get_ocr_download_guide', { modelType });
}

export async function downloadOcrModels(modelType?: string): Promise<void> {
  return invoke('download_ocr_models', { modelType });
}

// 模型管理面板
export interface ModelFileInfo {
  name: string;
  url: string;
  size: number;
  downloaded: boolean;
}

export interface ModelOverview {
  model_type: string;
  label: string;
  files: ModelFileInfo[];
  ready: boolean;
  is_active: boolean;
  models_dir: string;
}

export async function getModelOverview(): Promise<ModelOverview[]> {
  return invoke('get_model_overview');
}

/** 启用 modelType；传 null 禁用当前模型 */
export async function setActiveModel(modelType: string | null): Promise<OcrStatus> {
  return invoke('set_active_model', { modelType });
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
export type PreprocessMode = 'auto' | 'off' | 'on';

export interface AppSettings {
  data_dir: string;
  log_dir: string;
  pdf_reader_path: string | null;
  ocr_max_image_dimension: number;
  ocr_preprocess_mode: PreprocessMode;
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

export async function setOcrPreprocessMode(mode: PreprocessMode): Promise<void> {
  return invoke('set_ocr_preprocess_mode', { mode });
}

export async function openPdfExternally(pdfPath: string): Promise<void> {
  return invoke('open_pdf_externally', { pdfPath });
}

// PDF Preview API
export async function renderPdfPage(pdfId: number, pageNum: number): Promise<string> {
  return invoke('render_pdf_page', { pdfId, pageNum });
}