<script lang="ts">
  import { renderPdfPage } from '../../api';
  import { selectedPdfId, jumpToPage } from '../../stores';
  import { onMount, tick } from 'svelte';

  export let pdfPath: string | null = null;
  export let pageCount: number = 0;

  let currentPage = 1;
  let imageSrc: string | null = null;
  let isLoading = false;
  let error: string | null = null;
  let scale = 1.0;
  let fitToWidth = true;
  let containerWidth = 800;
  let imageWidth = 2480;
  let imageHeight = 3508;
  let contentElement: HTMLElement;
  let resizeObserver: ResizeObserver;
  let pendingJumpPage: number | null = null;
  let lastLoadedPdfId: number | null = null;

  $: if ($jumpToPage !== null && $jumpToPage >= 1) {
    pendingJumpPage = $jumpToPage;
    jumpToPage.set(null);
  }

  $: if (pendingJumpPage !== null && pageCount > 0 && pendingJumpPage <= pageCount) {
    currentPage = pendingJumpPage;
    loadPage(currentPage);
    pendingJumpPage = null;
  }

  $: if ($selectedPdfId && $selectedPdfId !== lastLoadedPdfId && pageCount > 0 && pendingJumpPage === null) {
    currentPage = 1;
    loadPage(currentPage);
  }

  $: currentScale = fitToWidth && containerWidth > 0 && imageWidth > 0
    ? containerWidth / imageWidth
    : scale;

  function handleImageLoad(e: Event) {
    const img = e.target as HTMLImageElement;
    imageWidth = img.naturalWidth;
    imageHeight = img.naturalHeight;
  }

  function updateContainerWidth() {
    if (contentElement) {
      const newWidth = contentElement.clientWidth - 40;
      if (newWidth > 0 && newWidth !== containerWidth) {
        containerWidth = newWidth;
      }
    }
  }

  onMount(() => {
    updateContainerWidth();

    resizeObserver = new ResizeObserver(() => {
      updateContainerWidth();
    });

    if (contentElement) {
      resizeObserver.observe(contentElement);
    }

    return () => {
      if (resizeObserver) {
        resizeObserver.disconnect();
      }
    };
  });

  async function loadPage(page: number) {
    if (!$selectedPdfId || page < 1 || page > pageCount) return;

    isLoading = true;
    error = null;
    lastLoadedPdfId = $selectedPdfId;

    try {
      imageSrc = await renderPdfPage($selectedPdfId!, page);
      await tick();
      updateContainerWidth();
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
    updateContainerWidth();
  }
</script>

<div class="pdf-viewer">
  {#if !pdfPath}
    <div class="placeholder">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
      </svg>
      <p>选择PDF文件预览</p>
    </div>
  {:else}
    <div class="toolbar">
      <div class="nav">
        <button class="nav-btn" on:click={prevPage} disabled={currentPage <= 1 || isLoading}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="15 18 9 12 15 6"/>
          </svg>
        </button>
        <span class="page-info">
          {currentPage} / {pageCount}
        </span>
        <button class="nav-btn" on:click={nextPage} disabled={currentPage >= pageCount || isLoading}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="9 18 15 12 9 6"/>
          </svg>
        </button>
      </div>
      <div class="zoom">
        <button class="zoom-btn" on:click={zoomOut} title="缩小">−</button>
        <span class="zoom-level">{Math.round(currentScale * 100)}%</span>
        <button class="zoom-btn" on:click={zoomIn} title="放大">+</button>
        <button class="zoom-btn" on:click={resetZoom} title="100%">100%</button>
        <button class="zoom-btn fit" class:active={fitToWidth} on:click={fitWidth} title="适应窗口">适应</button>
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
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
            <circle cx="12" cy="12" r="10"/>
            <line x1="12" y1="8" x2="12" y2="12"/>
            <line x1="12" y1="16" x2="12.01" y2="16"/>
          </svg>
          <p>{error}</p>
        </div>
      {:else if imageSrc}
        <div class="image-container">
          <img
            src={imageSrc}
            alt="PDF Page {currentPage}"
            style="width: {imageWidth * currentScale}px;"
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
    background: #374151;
  }

  .placeholder, .loading, .error {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #9ca3af;
  }

  .placeholder svg {
    width: 48px;
    height: 48px;
    margin-bottom: 12px;
    opacity: 0.4;
  }

  .placeholder p {
    font-size: 14px;
  }

  .toolbar {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px 16px;
    background: #1f2937;
    flex-shrink: 0;
  }

  .nav {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .nav-btn {
    width: 32px;
    height: 32px;
    background: #4b5563;
    color: #e5e7eb;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: background 0.15s;
  }

  .nav-btn:hover:not(:disabled) {
    background: #6b7280;
  }

  .nav-btn:disabled {
    background: #374151;
    cursor: not-allowed;
    opacity: 0.5;
  }

  .nav-btn svg {
    width: 16px;
    height: 16px;
  }

  .page-info {
    color: #e5e7eb;
    font-size: 13px;
    font-weight: 500;
    min-width: 60px;
    text-align: center;
  }

  .zoom {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .zoom-btn {
    padding: 6px 10px;
    background: #4b5563;
    color: #e5e7eb;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .zoom-btn:hover {
    background: #6b7280;
  }

  .zoom-btn.active {
    background: var(--accent, #3b82f6);
    color: white;
  }

  .zoom-level {
    color: #9ca3af;
    font-size: 12px;
    min-width: 45px;
    text-align: center;
  }

  .content {
    flex: 1;
    overflow: auto;
    display: flex;
    justify-content: center;
    background: #374151;
  }

  .image-container {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 24px;
    min-height: 100%;
  }

  .image-container img {
    box-shadow: 0 4px 16px rgba(0,0,0,0.4);
    border-radius: 4px;
  }

  .loading {
    background: #374151;
  }

  .spinner {
    width: 32px;
    height: 32px;
    border: 3px solid #4b5563;
    border-top-color: var(--accent, #3b82f6);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
    margin-bottom: 12px;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .error {
    color: #f87171;
    background: #374151;
  }

  .error svg {
    width: 32px;
    height: 32px;
    margin-bottom: 8px;
  }
</style>