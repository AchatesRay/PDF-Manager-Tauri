<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import * as pdfjsLib from 'pdfjs-dist';
  import { openPdfExternally } from '../api';

  // 设置 worker
  pdfjsLib.GlobalWorkerOptions.workerSrc = 'https://cdnjs.cloudflare.com/ajax/libs/pdf.js/3.11.174/pdf.worker.min.js';

  export let pdfPath: string | null = null;

  let canvas: HTMLCanvasElement;
  let pdfDoc: pdfjsLib.PDFDocumentProxy | null = null;
  let currentPage = 1;
  let totalPages = 0;
  let scale = 1.2;
  let isLoading = false;
  let error: string | null = null;

  $: if (pdfPath) {
    loadPdf(pdfPath);
  }

  async function loadPdf(path: string) {
    isLoading = true;
    error = null;

    try {
      const loadingTask = pdfjsLib.getDocument(path);
      pdfDoc = await loadingTask.promise;
      totalPages = pdfDoc.numPages;
      currentPage = 1;
      await renderPage(currentPage);
    } catch (e) {
      error = '加载PDF失败: ' + e;
      console.error('Failed to load PDF:', e);
    } finally {
      isLoading = false;
    }
  }

  async function renderPage(pageNum: number) {
    if (!pdfDoc || !canvas) return;

    const page = await pdfDoc.getPage(pageNum);
    const viewport = page.getViewport({ scale });

    canvas.height = viewport.height;
    canvas.width = viewport.width;

    const context = canvas.getContext('2d');
    if (!context) return;

    const renderContext = {
      canvasContext: context,
      viewport: viewport
    };

    await page.render(renderContext).promise;
  }

  function prevPage() {
    if (currentPage > 1) {
      currentPage--;
      renderPage(currentPage);
    }
  }

  function nextPage() {
    if (currentPage < totalPages) {
      currentPage++;
      renderPage(currentPage);
    }
  }

  function zoomIn() {
    scale = Math.min(scale + 0.2, 3);
    renderPage(currentPage);
  }

  function zoomOut() {
    scale = Math.max(scale - 0.2, 0.5);
    renderPage(currentPage);
  }

  async function handleOpenExternally() {
    if (!pdfPath) return;
    try {
      await openPdfExternally(pdfPath);
    } catch (e) {
      alert('打开PDF失败: ' + e);
    }
  }

  onDestroy(() => {
    pdfDoc?.destroy();
  });
</script>

<div class="pdf-viewer">
  {#if !pdfPath}
    <div class="placeholder">
      <p>选择PDF文件预览</p>
    </div>
  {:else if isLoading}
    <div class="loading">
      <p>加载中...</p>
    </div>
  {:else if error}
    <div class="error">
      <p>{error}</p>
    </div>
  {:else}
    <div class="toolbar">
      <button on:click={prevPage} disabled={currentPage <= 1}>上一页</button>
      <span>{currentPage} / {totalPages}</span>
      <button on:click={nextPage} disabled={currentPage >= totalPages}>下一页</button>
      <div class="spacer"></div>
      <button on:click={zoomOut} disabled={scale <= 0.5}>-</button>
      <span>{Math.round(scale * 100)}%</span>
      <button on:click={zoomIn} disabled={scale >= 3}>+</button>
      <button class="external-btn" on:click={handleOpenExternally}>外部打开</button>
    </div>
    <div class="canvas-container">
      <canvas bind:this={canvas}></canvas>
    </div>
  {/if}
</div>

<style>
  .pdf-viewer {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    background: #f0f0f0;
  }

  .placeholder, .loading, .error {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    color: #666;
  }

  .error {
    color: #f44336;
  }

  .toolbar {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px;
    background: #fff;
    border-bottom: 1px solid #ddd;
  }

  .toolbar button {
    padding: 5px 10px;
    border: 1px solid #ddd;
    background: #fff;
    cursor: pointer;
    border-radius: 4px;
  }

  .toolbar button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .external-btn {
    background: #4caf50;
    color: white;
    margin-left: 10px;
  }

  .spacer {
    flex: 1;
  }

  .canvas-container {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    padding: 10px;
  }

  canvas {
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.1);
  }
</style>