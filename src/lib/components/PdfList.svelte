<script lang="ts">
  import { pdfList, selectedPdfId, selectedFolderId, isLoading, selectedPdfPath, selectedPdfPageCount, ocrProgress, folders } from '../stores';
  import { getPdfList, addPdf, deletePdf, getPdfDetail, startOcr } from '../api';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open, confirm, message } from '@tauri-apps/plugin-dialog';
  import type { OcrProgress } from '../stores';

  onMount(async () => {
    await loadPdfs();

    // 监听 OCR 进度事件
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
    // 检查是否有文件夹
    if ($folders.length === 0) {
      await message('请先创建一个文件夹，然后选中该文件夹再导入PDF文件。', {
        title: '提示',
        kind: 'info',
      });
      return;
    }

    // 检查是否选中了文件夹
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
    console.log('Selected files:', files.length);

    for (const filePath of files) {
      try {
        console.log('Adding PDF:', filePath, 'to folder:', $selectedFolderId);
        const result = await addPdf(filePath, $selectedFolderId ?? undefined);
        console.log('PDF added successfully:', result);
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
      case 'pending': return '等待';
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

  $: filteredPdfs = $selectedFolderId === null
    ? $pdfList
    : $pdfList.filter(p => p.folder_id === $selectedFolderId);
</script>

<div class="pdf-list">
  <div class="header">
    <h3>PDF 文件 ({filteredPdfs.length})</h3>
    <button on:click={handleAddPdf}>添加</button>
  </div>

  {#if $isLoading}
    <div class="loading">加载中...</div>
  {:else if filteredPdfs.length === 0}
    <div class="empty">暂无 PDF 文件</div>
  {:else}
    <ul class="list">
      {#each filteredPdfs as pdf}
        <li class:active={$selectedPdfId === pdf.id} on:click={() => selectPdf(pdf.id)}>
          <span class="filename" title={pdf.filename}>{pdf.filename}</span>
          <span class="meta">{pdf.page_count}页</span>
          <span class="meta">{getTypeText(pdf.pdf_type)}</span>
          <span class="status status-{pdf.status}">
            {getStatusText(pdf.status)}
            {#if $ocrProgress.has(pdf.id) && $ocrProgress.get(pdf.id)?.status === 'processing'}
              ({$ocrProgress.get(pdf.id)?.current}/{$ocrProgress.get(pdf.id)?.total})
            {/if}
          </span>
          <div class="actions">
            {#if pdf.status === 'pending'}
              <button class="ocr-btn" on:click|stopPropagation={() => handleStartOcr(pdf.id)}>OCR</button>
            {:else if pdf.status === 'processing'}
              <span class="processing-indicator">...</span>
            {/if}
            <button class="delete-btn" on:click|stopPropagation={() => handleDelete(pdf.id)}>删</button>
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
    padding: 10px;
    min-width: 0;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .header h3 {
    font-size: 14px;
    color: #333;
  }

  .header button {
    padding: 6px 12px;
    background: #2196f3;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .list {
    list-style: none;
    padding: 0;
    margin: 0;
    flex: 1;
    overflow-y: auto;
  }

  .list li {
    padding: 8px 10px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
    background: #fff;
    border: 1px solid #eee;
    font-size: 13px;
  }

  .list li:hover {
    background: #f5f5f5;
  }

  .list li.active {
    border-color: #2196f3;
    background: #e3f2fd;
  }

  .filename {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 500;
    min-width: 0;
  }

  .meta {
    font-size: 12px;
    color: #666;
    white-space: nowrap;
    flex-shrink: 0;
  }

  .status {
    font-size: 12px;
    padding: 2px 6px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .status-pending {
    background: #f5f5f5;
    color: #666;
  }

  .status-processing {
    background: #fff3e0;
    color: #e65100;
  }

  .status-done {
    background: #e8f5e9;
    color: #2e7d32;
  }

  .status-error {
    background: #ffebee;
    color: #c62828;
  }

  .actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .ocr-btn {
    padding: 3px 8px;
    background: #4caf50;
    color: white;
    border: none;
    border-radius: 3px;
    cursor: pointer;
    font-size: 12px;
  }

  .processing-indicator {
    font-size: 12px;
    color: #ff9800;
  }

  .delete-btn {
    padding: 3px 8px;
    background: #f44336;
    color: white;
    border: none;
    border-radius: 3px;
    cursor: pointer;
    font-size: 12px;
  }

  .loading, .empty {
    text-align: center;
    padding: 40px 20px;
    color: #666;
  }
</style>