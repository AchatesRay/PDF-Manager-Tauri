<script lang="ts">
  import { pdfList, selectedPdfId, selectedFolderId, isLoading, selectedPdfPath, selectedPdfPageCount, ocrProgress, folders } from '../stores';
  import { getPdfList, addPdf, deletePdf, getPdfDetail, startOcr } from '../api';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open, confirm, message } from '@tauri-apps/plugin-dialog';
  import type { OcrProgress } from '../stores';

  onMount(async () => {
    await loadPdfs();

    const unlisten = await listen<OcrProgress>('ocr-progress', (event) => {
      const progress = event.payload;
      ocrProgress.update(map => {
        map.set(progress.pdf_id, progress);
        return map;
      });

      if (progress.status === 'done') {
        loadPdfs();
      }
    });

    return unlisten;
  });

  async function loadPdfs() {
    isLoading.set(true);
    try {
      pdfList.set(await getPdfList());
    } catch (e) {
      console.error('Failed to load PDFs:', e);
    } finally {
      isLoading.set(false);
    }
  }

  async function handleAddPdf() {
    if ($folders.length === 0) {
      await message('请先创建一个文件夹，然后选中该文件夹再导入PDF文件。', {
        title: '提示',
        kind: 'info',
      });
      return;
    }

    if ($selectedFolderId === null) {
      await message('请先在左侧选择一个文件夹，然后再添加PDF文件。', {
        title: '提示',
        kind: 'info',
      });
      return;
    }

    const selected = await open({
      multiple: true,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      title: '选择 PDF 文件',
    });

    if (!selected) {
      return;
    }

    const files = Array.isArray(selected) ? selected : [selected];

    for (const filePath of files) {
      try {
        await addPdf(filePath, $selectedFolderId ?? undefined);
      } catch (err) {
        console.error('Failed to add PDF:', err);
        alert('添加PDF失败: ' + err);
      }
    }
    await loadPdfs();
  }

  async function handleDelete(id: number) {
    const pdf = $pdfList.find(p => p.id === id);
    const filename = pdf?.filename || '此PDF';

    const confirmed = await confirm(`确定要删除 "${filename}" 吗？`, {
      title: '确认删除',
      kind: 'warning',
    });

    if (confirmed) {
      try {
        await deletePdf(id);
        await loadPdfs();
      } catch (e) {
        alert('删除失败: ' + e);
      }
    }
  }

  async function selectPdf(id: number) {
    selectedPdfId.set(id);
    try {
      const detail = await getPdfDetail(id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
      selectedPdfPath.set(null);
      selectedPdfPageCount.set(0);
    }
  }

  async function handleStartOcr(id: number) {
    try {
      await startOcr(id);
    } catch (e) {
      alert('启动OCR失败: ' + e);
    }
  }

  function getStatusText(status: string): string {
    switch (status) {
      case 'pending': return '待处理';
      case 'processing': return '处理中';
      case 'done': return '已OCR';
      case 'error': return '出错';
      default: return status;
    }
  }

  function getTypeText(type: string): string {
    switch (type) {
      case 'text': return '文字';
      case 'scanned': return '扫描';
      case 'mixed': return '混合';
      default: return type;
    }
  }

  // 递归获取文件夹及其所有子文件夹的 ID
  function getAllFolderIds(folderId: number): number[] {
    const ids = [folderId];
    const children = $folders.filter(f => f.parent_id === folderId);
    for (const child of children) {
      ids.push(...getAllFolderIds(child.id));
    }
    return ids;
  }

  $: filteredPdfs = $selectedFolderId === null
    ? $pdfList
    : $pdfList.filter(p => getAllFolderIds($selectedFolderId!).includes(p.folder_id ?? 0));
</script>

<div class="pdf-list">
  <div class="list-header">
    <span class="list-title">
      PDF 文件
      <span class="count">{filteredPdfs.length} 个文档</span>
    </span>
    <button class="add-btn" on:click={handleAddPdf}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="12" y1="5" x2="12" y2="19"/>
        <line x1="5" y1="12" x2="19" y2="12"/>
      </svg>
      添加
    </button>
  </div>

  {#if $isLoading}
    <div class="loading-state">
      <div class="spinner"></div>
      <span>加载中...</span>
    </div>
  {:else if filteredPdfs.length === 0}
    <div class="empty-state">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
      </svg>
      <p>暂无 PDF 文件</p>
      <span>选择文件夹后点击"添加"导入文档</span>
    </div>
  {:else}
    <ul class="list">
      {#each filteredPdfs as pdf}
        <li
          class="pdf-item"
          class:active={$selectedPdfId === pdf.id}
          on:click={() => selectPdf(pdf.id)}
        >
          <span class="filename" title={pdf.filename}>{pdf.filename}</span>
          <span class="meta">{pdf.page_count} 页</span>
          <span class="meta">{getTypeText(pdf.pdf_type)}</span>
          <div class="status-area">
            <span class="status-indicator status-{pdf.status}">
              <span class="status-dot"></span>
              <span class="status-text">
                {getStatusText(pdf.status)}
                {#if $ocrProgress.has(pdf.id) && $ocrProgress.get(pdf.id)?.status === 'processing'}
                  ({$ocrProgress.get(pdf.id)?.current}/{$ocrProgress.get(pdf.id)?.total})
                {/if}
              </span>
            </span>
          </div>
          <div class="actions">
            {#if pdf.status === 'pending'}
              <button class="ocr-btn" on:click|stopPropagation={() => handleStartOcr(pdf.id)}>OCR</button>
            {:else if pdf.status === 'processing'}
              <span class="processing-indicator">
                <svg class="spinner-small" viewBox="0 0 24 24">
                  <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none" stroke-dasharray="31.4 31.4"/>
                </svg>
              </span>
            {/if}
            <button class="delete-btn" on:click|stopPropagation={() => handleDelete(pdf.id)} title="删除">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6"/>
                <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
              </svg>
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</div>

<style>
  .pdf-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    flex-shrink: 0;
  }

  .list-title {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .count {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-muted, #9ca3af);
  }

  .add-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 5px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    cursor: pointer;
    transition: all 0.15s;
  }

  .add-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .add-btn svg {
    width: 12px;
    height: 12px;
  }

  .list {
    list-style: none;
    padding: 0 8px 8px;
    margin: 0;
    flex: 1;
    overflow-y: auto;
  }

  .pdf-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border-light, #f3f4f6);
    border-radius: 6px;
    margin-bottom: 4px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .pdf-item:hover {
    border-color: var(--border, #e5e7eb);
    box-shadow: 0 1px 2px rgba(0,0,0,0.04);
  }

  .pdf-item.active {
    border-color: var(--accent, #3b82f6);
    background: var(--accent-soft, #eff6ff);
  }

  .filename {
    flex: 1;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .meta {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .status-area {
    flex-shrink: 0;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
  }

  .status-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-pending .status-dot { background: var(--text-muted, #9ca3af); }
  .status-processing .status-dot { background: var(--warning, #f59e0b); }
  .status-done .status-dot { background: var(--success, #10b981); }
  .status-error .status-dot { background: var(--error, #ef4444); }

  .status-text {
    color: var(--text-secondary, #6b7280);
  }

  .actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    margin-left: 6px;
  }

  .ocr-btn {
    padding: 4px 8px;
    background: var(--success, #10b981);
    color: white;
    border: none;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }

  .ocr-btn:hover {
    background: #059669;
  }

  .processing-indicator {
    display: flex;
    align-items: center;
  }

  .spinner-small {
    width: 14px;
    height: 14px;
    color: var(--warning, #f59e0b);
    animation: spin 1s linear infinite;
  }

  .delete-btn {
    width: 24px;
    height: 24px;
    background: none;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #9ca3af);
    transition: all 0.15s;
  }

  .delete-btn:hover {
    border-color: var(--error, #ef4444);
    background: var(--error, #ef4444);
    color: white;
  }

  .delete-btn svg {
    width: 12px;
    height: 12px;
  }

  .loading-state, .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #9ca3af);
    gap: 10px;
  }

  .loading-state {
    flex-direction: row;
  }

  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid var(--border, #e5e7eb);
    border-top-color: var(--accent, #3b82f6);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .empty-state svg {
    width: 40px;
    height: 40px;
    opacity: 0.4;
  }

  .empty-state p {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary, #6b7280);
  }

  .empty-state span {
    font-size: 11px;
  }
</style>