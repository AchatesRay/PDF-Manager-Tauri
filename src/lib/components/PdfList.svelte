<script lang="ts">
  import { pdfList, selectedPdfId, selectedFolderId, isLoading, selectedPdfPath, selectedPdfPageCount, ocrProgress, folders, ocrModelStatus, ocrQueue, showDownloadDialog } from '../stores';
  import { getPdfList, addPdf, deletePdf, getPdfDetail, startOcr, getOcrStatus, getOcrQueueStatus, cancelOcrTask, refreshOcrStatus } from '../api';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open, confirm, message } from '@tauri-apps/plugin-dialog';
  import type { OcrProgress } from '../stores';
  import OcrModelSetup from './OcrModelSetup.svelte';
  import PdfListItem from './PdfListItem.svelte';

  let isRefreshing = false;

  onMount(() => {
    loadPdfs();
    refreshQueueStatus();

    // 检查 OCR 模型状态
    getOcrStatus()
      .then(status => ocrModelStatus.set(status))
      .catch(e => console.error('Failed to get OCR status:', e));

    // 监听 OCR 进度（事件驱动，无轮询）
    let unlistenProgress: (() => void) | null = null;
    let unlistenQueued: (() => void) | null = null;

    // 每个 pdf 上一次已知状态，用于检测转换以触发列表/队列刷新
    const lastStatus = new Map<number, string>();

    listen<OcrProgress>('ocr-progress', (event) => {
      const progress = event.payload;
      const prev = lastStatus.get(progress.pdf_id);

      // 最小更新：内容无变化则不触碰 store
      ocrProgress.update(map => {
        const old = map.get(progress.pdf_id);
        if (
          old &&
          old.current === progress.current &&
          old.total === progress.total &&
          old.status === progress.status
        ) {
          return map;
        }
        const next = new Map(map);
        next.set(progress.pdf_id, progress);
        return next;
      });

      if (prev !== progress.status) {
        lastStatus.set(progress.pdf_id, progress.status);

        if (progress.status === 'done' || progress.status === 'error') {
          loadPdfs();
          refreshQueueStatus();
        } else if (progress.status === 'processing') {
          // 任务真正开跑时同步一次队列（替代原轮询）
          refreshQueueStatus();
        }
      }
    }).then(unlisten => unlistenProgress = unlisten);

    // 监听排队事件
    listen<{ pdf_id: number; position: number }>('ocr-queued', (event) => {
      console.log('OCR 任务已排队:', event.payload);
      refreshQueueStatus();
    }).then(unlisten => unlistenQueued = unlisten);

    return () => {
      unlistenProgress?.();
      unlistenQueued?.();
    };
  });

  async function refreshQueueStatus() {
    try {
      const status = await getOcrQueueStatus();
      ocrQueue.set(status);
    } catch (e) {
      console.error('Failed to get queue status:', e);
    }
  }

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

  async function handleRefresh() {
    isRefreshing = true;
    try {
      const status = await refreshOcrStatus();
      ocrModelStatus.set(status);
      if (!status.models_ready) {
        showDownloadDialog.set(true);
      }
    } catch (e) {
      console.error('Refresh OCR status failed:', e);
    } finally {
      isRefreshing = false;
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

  async function handleStartOcr(id: number, force: boolean = false) {
    try {
      await startOcr(id, force);
      await refreshQueueStatus();
    } catch (e) {
      alert('启动OCR失败: ' + e);
    }
  }

  async function handleCancelTask(id: number) {
    try {
      const cancelled = await cancelOcrTask(id);
      if (cancelled) {
        await refreshQueueStatus();
      } else {
        alert('无法取消正在运行的任务');
      }
    } catch (e) {
      alert('取消任务失败: ' + e);
    }
  }

  function getQueuePosition(pdfId: number): number | null {
    if ($ocrQueue.current === pdfId) return 0;
    const idx = $ocrQueue.pending.findIndex(t => t.pdf_id === pdfId);
    return idx >= 0 ? idx + 1 : null;
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
    <div class="header-actions">
      <!-- OCR 模型状态和重新检测按钮 -->
      <div class="model-status-area">
        <OcrModelSetup />
        <button
          class="refresh-btn"
          on:click={handleRefresh}
          disabled={isRefreshing}
          title={$ocrModelStatus?.models_ready ? '模型状态正常' : '重新检测模型'}
        >
          <svg class:spinning={isRefreshing} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M23 4v6h-6M1 20v-6h6"/>
            <path d="M3.51 9a9 9 0 0114.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0020.49 15"/>
          </svg>
          {#if $ocrModelStatus?.models_ready}
            模型正常
          {:else}
            重新检测
          {/if}
        </button>
      </div>
      <button class="add-btn" on:click={handleAddPdf}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        添加
      </button>
    </div>
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
        <PdfListItem
          {pdf}
          active={$selectedPdfId === pdf.id}
          queuePosition={getQueuePosition(pdf.id)}
          progress={$ocrProgress.get(pdf.id)}
          onSelect={selectPdf}
          onStartOcr={handleStartOcr}
          onCancel={handleCancelTask}
          onDelete={handleDelete}
        />
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

  .header-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .model-status-area {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .refresh-btn {
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
    white-space: nowrap;
  }

  .refresh-btn:hover:not(:disabled) {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .refresh-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .refresh-btn svg {
    width: 12px;
    height: 12px;
  }

  .refresh-btn svg.spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
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

  /* 条目样式已随组件拆分移至 PdfListItem.svelte */

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