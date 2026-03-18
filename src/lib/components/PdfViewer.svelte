<script lang="ts">
  import { renderPdfPage } from '../api';
  import { selectedPdfId, jumpToPage } from '../stores';
  import { onMount } from 'svelte';

  export let pdfPath: string | null = null;
  export let pageCount: number = 0;

  let currentPage = 1;
  let imageSrc: string | null = null;
  let isLoading = false;
  let error: string | null = null;
  let scale = 1.0;
  let fitToWidth = true; // 默认适应窗口
  let containerWidth = 0;
  let imageWidth = 0;
  let imageHeight = 0;
  let contentElement: HTMLElement;

  // 监听 jumpToPage 变化，跳转到指定页面
  $: if ($jumpToPage !== null && $jumpToPage >= 1 && $jumpToPage <= pageCount) {
    currentPage = $jumpToPage;
    loadPage(currentPage);
    jumpToPage.set(null);
  }

  $: if ($selectedPdfId && pageCount > 0) {
    loadPage(currentPage);
  }

  // 响应式计算缩放比例
  $: currentScale = calculateScale();

  function calculateScale(): number {
    if (fitToWidth && containerWidth > 0 && imageWidth > 0) {
      return containerWidth / imageWidth;
    }
    return scale;
  }

  function handleImageLoad(e: Event) {
    const img = e.target as HTMLImageElement;
    imageWidth = img.naturalWidth;
    imageHeight = img.naturalHeight;
  }

  onMount(() => {
    // 监听窗口大小变化
    const resizeObserver = new ResizeObserver(() => {
      if (contentElement) {
        containerWidth = contentElement.clientWidth - 40;
      }
    });

    if (contentElement) {
      containerWidth = contentElement.clientWidth - 40;
      resizeObserver.observe(contentElement);
    }

    return () => resizeObserver.disconnect();
  });

  async function loadPage(page: number) {
    if (!$selectedPdfId || page < 1 || page > pageCount) return;

    isLoading = true;
    error = null;

    try {
      imageSrc = await renderPdfPage($selectedPdfId!, page);
    } catch (e) {
      error = '加载页面失败: ' + e;
      imageSrc = null;
    } finally {
      isLoading = false;
    }
  }

  function prevPage() {
    if (currentPage > 1) {
      currentPage--;
      loadPage(currentPage);
    }
  }

  function nextPage() {
    if (currentPage < pageCount) {
      currentPage++;
      loadPage(currentPage);
    }
  }

  function zoomIn() {
    fitToWidth = false;
    scale = Math.min(scale + 0.25, 3.0);
  }

  function zoomOut() {
    fitToWidth = false;
    scale = Math.max(scale - 0.25, 0.25);
  }

  function resetZoom() {
    fitToWidth = false;
    scale = 1.0;
  }

  function fitWidth() {
    fitToWidth = true;
  }
</script>

<div class="pdf-viewer">
  {#if !pdfPath}
    <div class="placeholder">
      <p>选择PDF文件预览</p>
    </div>
  {:else}
    <div class="toolbar">
      <div class="nav">
        <button on:click={prevPage} disabled={currentPage <= 1 || isLoading}>
          &#8592; 上一页
        </button>
        <span class="page-info">
          {currentPage} / {pageCount}
        </span>
        <button on:click={nextPage} disabled={currentPage >= pageCount || isLoading}>
          下一页 &#8594;
        </button>
      </div>
      <div class="zoom">
        <button on:click={zoomOut} title="缩小">-</button>
        <span class="zoom-level">{Math.round(currentScale * 100)}%</span>
        <button on:click={zoomIn} title="放大">+</button>
        <button on:click={resetZoom} title="100%">100%</button>
        <button on:click={fitWidth} class:active={fitToWidth} title="适应窗口">适应</button>
      </div>
    </div>

    <div class="content" bind:this={contentElement}>
      {#if isLoading}
        <div class="loading">
          <div class="spinner"></div>
          <p>正在加载页面 {currentPage}...</p>
        </div>
      {:else if error}
        <div class="error">
          <p>{error}</p>
        </div>
      {:else if imageSrc}
        <div class="image-container">
          <img
            src={imageSrc}
            alt="PDF Page {currentPage}"
            style="width: {imageWidth * currentScale}px; height: auto;"
            on:load={handleImageLoad}
          />
        </div>
      {:else}
        <div class="placeholder">
          <p>点击PDF文件开始预览</p>
        </div>
      {/if}
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
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #666;
  }

  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 16px;
    background: #fff;
    border-bottom: 1px solid #ddd;
    flex-shrink: 0;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .nav button {
    padding: 6px 12px;
    background: #2196f3;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .nav button:disabled {
    background: #ccc;
    cursor: not-allowed;
  }

  .page-info {
    font-weight: 500;
    min-width: 60px;
    text-align: center;
  }

  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .zoom button {
    padding: 4px 8px;
    background: #e0e0e0;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .zoom button:hover {
    background: #d0d0d0;
  }

  .zoom button.active {
    background: #2196f3;
    color: white;
  }

  .zoom-level {
    min-width: 50px;
    text-align: center;
  }

  .content {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    background: #525659;
  }

  .image-container {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 20px;
    min-height: 100%;
  }

  .image-container img {
    box-shadow: 0 2px 8px rgba(0,0,0,0.3);
  }

  .spinner {
    width: 40px;
    height: 40px;
    border: 3px solid #ddd;
    border-top-color: #2196f3;
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .error {
    color: #f44336;
  }
</style>