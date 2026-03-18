<script lang="ts">
  import FolderTree from './lib/components/FolderTree.svelte';
  import PdfList from './lib/components/PdfList.svelte';
  import SearchBar from './lib/components/SearchBar.svelte';
  import SearchResults from './lib/components/SearchResults.svelte';
  import PdfViewer from './lib/components/PdfViewer.svelte';
  import { selectedPdfPath, selectedPdfPageCount } from './lib/stores';

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

<main>
  <div class="left-panel" style="width: {leftWidth}px">
    <FolderTree />
  </div>
  <div class="resizer left-resizer" on:mousedown={startDragLeft}></div>
  <div class="center-panel">
    <SearchBar />
    <PdfList />
    <SearchResults />
  </div>
  <div class="resizer right-resizer" on:mousedown={startDragRight}></div>
  <div class="right-panel" style="width: {rightWidth}px">
    <PdfViewer pdfPath={$selectedPdfPath} pageCount={$selectedPdfPageCount} />
  </div>
</main>

<style>
  main {
    display: flex;
    height: 100vh;
    width: 100vw;
    overflow: hidden;
  }

  .left-panel {
    min-width: 180px;
    border-right: 1px solid #ddd;
    overflow: hidden;
    flex-shrink: 0;
  }

  .center-panel {
    flex: 1;
    min-width: 300px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .right-panel {
    min-width: 300px;
    border-left: 1px solid #ddd;
    overflow: hidden;
    flex-shrink: 0;
  }

  .resizer {
    width: 5px;
    cursor: col-resize;
    background: transparent;
    z-index: 10;
    transition: background 0.2s;
  }

  .resizer:hover {
    background: #2196f3;
  }

  .left-resizer {
    margin-left: -1px;
  }

  .right-resizer {
    margin-right: -1px;
  }
</style>