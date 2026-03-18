<script lang="ts">
  import { openPdfExternally } from '../api';

  export let pdfPath: string | null = null;

  async function handleOpenPdf() {
    if (!pdfPath) return;
    try {
      await openPdfExternally(pdfPath);
    } catch (e) {
      alert('打开PDF失败: ' + e);
    }
  }
</script>

<div class="pdf-viewer">
  {#if !pdfPath}
    <div class="placeholder">
      <p>选择PDF文件预览</p>
    </div>
  {:else}
    <div class="preview-container">
      <div class="file-icon">📄</div>
      <p class="filename">{pdfPath.split(/[/\\]/).pop()}</p>
      <button class="open-btn" on:click={handleOpenPdf}>
        使用本地阅读器打开
      </button>
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

  .placeholder, .preview-container {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: #666;
  }

  .preview-container {
    gap: 16px;
  }

  .file-icon {
    font-size: 64px;
  }

  .filename {
    font-size: 16px;
    color: #333;
    max-width: 80%;
    text-align: center;
    word-break: break-all;
    margin: 0;
  }

  .open-btn {
    padding: 12px 24px;
    font-size: 16px;
    background: #2196f3;
    color: white;
    border: none;
    border-radius: 8px;
    cursor: pointer;
    transition: background 0.2s;
  }

  .open-btn:hover {
    background: #1976d2;
  }
</style>