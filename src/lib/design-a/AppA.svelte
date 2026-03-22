<script lang="ts">
  import FolderTreeA from './FolderTreeA.svelte';
  import PdfListA from './PdfListA.svelte';
  import SearchBarA from './SearchBarA.svelte';
  import SearchResultsA from './SearchResultsA.svelte';
  import PdfViewerA from './PdfViewerA.svelte';
  import { selectedPdfPath, selectedPdfPageCount } from '../stores';

  // 面板宽度状态
  let leftWidth = 220;
  let rightWidth = 400;

  // 拖动状态
  let isDraggingLeft = false;
  let isDraggingRight = false;

  // 最小/最大宽度限制
  const leftMinWidth = 180;
  const leftMaxWidth = 400;
  const rightMinWidth = 300;
  const rightMaxWidth = 800;

  function startDragLeft(e: MouseEvent) {
    e.preventDefault();
    isDraggingLeft = true;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function startDragRight(e: MouseEvent) {
    e.preventDefault();
    isDraggingRight = true;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function handleMouseMove(e: MouseEvent) {
    if (isDraggingLeft) {
      const newWidth = e.clientX;
      leftWidth = Math.max(leftMinWidth, Math.min(leftMaxWidth, newWidth));
    } else if (isDraggingRight) {
      const newWidth = window.innerWidth - e.clientX;
      rightWidth = Math.max(rightMinWidth, Math.min(rightMaxWidth, newWidth));
    }
  }

  function handleMouseUp() {
    isDraggingLeft = false;
    isDraggingRight = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
  }
</script>

<svelte:window on:mousemove={handleMouseMove} on:mouseup={handleMouseUp} />

<main class="app-a">
  <!-- 左侧面板 -->
  <div class="panel-left" style="width: {leftWidth}px">
    <FolderTreeA />
  </div>

  <!-- 左侧分隔条 -->
  <div class="resizer left-resizer" on:mousedown={startDragLeft}></div>

  <!-- 中间面板 -->
  <div class="panel-center">
    <SearchBarA />
    <PdfListA />
    <SearchResultsA />
  </div>

  <!-- 右侧分隔条 -->
  <div class="resizer right-resizer" on:mousedown={startDragRight}></div>

  <!-- 右侧面板 -->
  <div class="panel-right" style="width: {rightWidth}px">
    <PdfViewerA pdfPath={$selectedPdfPath} pageCount={$selectedPdfPageCount} />
  </div>
</main>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    font-family: 'IBM Plex Sans', 'Noto Sans SC', -apple-system, BlinkMacSystemFont, sans-serif;
    background: #fafbfc;
    color: #1f2937;
    font-size: 14px;
    line-height: 1.5;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
  }

  /* CSS Variables for Design A - defined globally so child components can access them */
  :global(:root) {
    --bg-primary: #fafbfc;
    --bg-secondary: #ffffff;
    --bg-tertiary: #f5f7f9;
    --text-primary: #1f2937;
    --text-secondary: #6b7280;
    --text-muted: #9ca3af;
    --accent: #3b82f6;
    --accent-soft: #eff6ff;
    --border: #e5e7eb;
    --border-light: #f3f4f6;
    --success: #10b981;
    --warning: #f59e0b;
    --error: #ef4444;
  }

  .app-a {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
    background: var(--bg-primary);
  }

  .panel-left {
    min-width: 180px;
    border-right: 1px solid var(--border, #e5e7eb);
    overflow: hidden;
    flex-shrink: 0;
    background: var(--bg-secondary, #ffffff);
  }

  .panel-center {
    flex: 1;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--bg-primary, #fafbfc);
  }

  .panel-right {
    min-width: 300px;
    border-left: 1px solid var(--border, #e5e7eb);
    overflow: hidden;
    flex-shrink: 0;
  }

  .resizer {
    width: 5px;
    cursor: col-resize;
    background: transparent;
    z-index: 10;
    transition: background 0.15s ease;
    flex-shrink: 0;
  }

  .resizer:hover {
    background: var(--accent, #3b82f6);
  }

  .left-resizer {
    margin-left: -1px;
  }

  .right-resizer {
    margin-right: -1px;
  }
</style>