<script lang="ts">
  import { pdfList, selectedPdfId, selectedFolderId, isLoading, selectedPdfPath, ocrProgress } from '../stores';
  import { getPdfList, addPdf, deletePdf, getPdfDetail, startOcr } from '../api';
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open } from '@tauri-apps/plugin-dialog';
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
    if (confirm('确定删除此 PDF？')) {
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
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
      selectedPdfPath.set(null);
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
      case 'pending': return '等待处理';
      case 'processing': return '处理中';
      case 'done': return '已完成';
      case 'error': return '出错';
      default: return status;
    }
  }

  function getTypeText(type: string): string {
    switch (type) {
      case 'text': return '文字型';
      case 'scanned': return '扫描型';
      case 'mixed': return '混合型';
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
    <button on:click={handleAddPdf}>添加 PDF</button>
  </div>

  {#if $isLoading}
    <div class="loading">加载中...</div>
  {:else if filteredPdfs.length === 0}
    <div class="empty">暂无 PDF 文件<br/><small>点击上方按钮添加</small></div>
  {:else}
    <ul class="list">
      {#each filteredPdfs as pdf}
        <li class:active={$selectedPdfId === pdf.id} on:click={() => selectPdf(pdf.id)}>
          <div class="info">
            <span class="filename">{pdf.filename}</span>
            <span class="meta">
              {pdf.page_count} 页 · {getTypeText(pdf.pdf_type)} · {getStatusText(pdf.status)}
              {#if $ocrProgress.has(pdf.id) && $ocrProgress.get(pdf.id)?.status === 'processing'}
                <span class="progress">
                  ({$ocrProgress.get(pdf.id)?.current}/{$ocrProgress.get(pdf.id)?.total})
                </span>
              {/if}
            </span>
          </div>
          <div class="actions">
            {#if pdf.status === 'pending'}
              <button
                class="ocr-btn"
                on:click|stopPropagation={() => handleStartOcr(pdf.id)}
              >
                开始处理
              </button>
            {:else if pdf.status === 'processing'}
              <span class="processing">处理中...</span>
            {/if}
            <button class="delete-btn" on:click|stopPropagation={() => handleDelete(pdf.id)}>删除</button>
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
    padding: 12px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 5px;
    background: #fff;
    border: 1px solid #eee;
  }

  .list li:hover {
    background: #f5f5f5;
  }

  .list li.active {
    border-color: #2196f3;
    background: #e3f2fd;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    overflow: hidden;
  }

  .filename {
    font-weight: 500;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta {
    font-size: 12px;
    color: #666;
  }

  .progress {
    color: #2196f3;
  }

  .actions {
    display: flex;
    gap: 8px;
    align-items: center;
  }

  .ocr-btn {
    padding: 5px 10px;
    background: #4caf50;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .processing {
    font-size: 12px;
    color: #ff9800;
  }

  .delete-btn {
    padding: 5px 10px;
    background: #f44336;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    flex-shrink: 0;
  }

  .loading, .empty {
    text-align: center;
    padding: 40px 20px;
    color: #666;
  }

  .empty small {
    color: #999;
  }
</style>