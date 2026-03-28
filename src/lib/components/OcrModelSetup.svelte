<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import {
    getOcrStatus,
    getOcrDownloadGuide,
    downloadOcrModels,
    cancelOcrDownload,
    type OcrStatus,
    type DownloadGuide,
    type DownloadProgress,
  } from '../api';
  import {
    ocrModelStatus,
    ocrDownloadProgress,
    isDownloading,
    showDownloadDialog,
  } from '../stores';

  let error: string | null = null;
  let downloadGuides: DownloadGuide[] = [];

  onMount(() => {
    checkStatus();
    getOcrDownloadGuide().then(guides => downloadGuides = guides);

    // 监听下载进度
    let unlistenProgress: (() => void) | null = null;
    let unlistenComplete: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    listen<DownloadProgress>('model-download-progress', (event) => {
      ocrDownloadProgress.set(event.payload);
    }).then(unlisten => unlistenProgress = unlisten);

    // 监听下载完成
    listen<void>('model-download-complete', async () => {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      showDownloadDialog.set(false);
      await checkStatus();
    }).then(unlisten => unlistenComplete = unlisten);

    // 监听下载错误
    listen<{ error: string }>('model-download-error', (event) => {
      isDownloading.set(false);
      error = event.payload.error;
    }).then(unlisten => unlistenError = unlisten);

    return () => {
      unlistenProgress?.();
      unlistenComplete?.();
      unlistenError?.();
    };
  });

  async function checkStatus() {
    try {
      const status = await getOcrStatus();
      ocrModelStatus.set(status);
    } catch (e) {
      console.error('Failed to get OCR status:', e);
    }
  }

  async function handleDownload() {
    error = null;
    isDownloading.set(true);
    ocrDownloadProgress.set(null);

    try {
      await downloadOcrModels();
    } catch (e) {
      isDownloading.set(false);
      error = String(e);
    }
  }

  function handleCancel() {
    cancelOcrDownload();
    isDownloading.set(false);
    ocrDownloadProgress.set(null);
  }

  function formatProgress(current: number, total: number): string {
    if (total === 0) return '0%';
    const percent = Math.round((current / total) * 100);
    return `${percent}%`;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  $: statusText = $ocrModelStatus?.models_ready
    ? '✓ 模型已就绪'
    : '✗ 模型未安装';
</script>

<!-- 状态显示（嵌入在 PdfList header 中） -->
<div class="status-text" class:ready={$ocrModelStatus?.models_ready}>
  {statusText}
</div>

<!-- 错误提示 -->
{#if error}
  <div class="error-message">
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="10"/>
      <line x1="15" y1="9" x2="9" y2="15"/>
      <line x1="9" y1="9" x2="15" y2="15"/>
    </svg>
    <span>{error}</span>
  </div>
{/if}

<!-- 下载对话框 -->
{#if $showDownloadDialog && !$ocrModelStatus?.models_ready}
  <div class="download-dialog-overlay" on:click={() => showDownloadDialog.set(false)}>
    <div class="download-dialog" on:click|stopPropagation>
      <div class="dialog-header">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
        </svg>
        <h4>OCR 模型未安装</h4>
      </div>

      <p class="dialog-desc">请下载以下模型文件：</p>

      <ul class="file-list">
        {#each downloadGuides as guide}
          <li>
            <span class="file-name">{guide.name}</span>
            <span class="file-size">{guide.size}</span>
          </li>
        {/each}
      </ul>

      <p class="models-dir">模型目录: {$ocrModelStatus?.models_dir || 'models'}</p>

      {#if $isDownloading && $ocrDownloadProgress}
        <div class="download-progress">
          <div class="progress-info">
            <span class="file-name">{$ocrDownloadProgress.file}</span>
            <span class="progress-percent">
              {formatProgress($ocrDownloadProgress.current, $ocrDownloadProgress.total)}
            </span>
          </div>
          <div class="progress-bar">
            <div
              class="progress-fill"
              style="width: {($ocrDownloadProgress.current / ($ocrDownloadProgress.total || 1)) * 100}%"
            ></div>
          </div>
          <div class="progress-bytes">
            {formatBytes($ocrDownloadProgress.current)} / {formatBytes($ocrDownloadProgress.total)}
          </div>
          <button class="cancel-btn" on:click={handleCancel}>
            取消下载
          </button>
        </div>
      {:else}
        <div class="dialog-actions">
          <button class="download-btn" on:click={handleDownload}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
              <polyline points="7 10 12 15 17 10"/>
              <line x1="12" y1="15" x2="12" y2="3"/>
            </svg>
            在线下载
          </button>
          <button class="manual-btn" on:click={() => window.open('https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0', '_blank')}>
            手动下载
          </button>
          <button class="close-btn" on:click={() => showDownloadDialog.set(false)}>
            取消
          </button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .status-text {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
  }

  .status-text.ready {
    color: var(--success, #10b981);
  }

  .error-message {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px;
    background: var(--error-soft, #fef2f2);
    border: 1px solid var(--error, #ef4444);
    border-radius: 4px;
    margin-top: 8px;
    font-size: 11px;
    color: var(--error, #ef4444);
  }

  .error-message svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  /* 下载对话框 */
  .download-dialog-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .download-dialog {
    background: var(--bg-secondary, #ffffff);
    border-radius: 8px;
    padding: 16px;
    max-width: 400px;
    width: 90%;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  }

  .dialog-header {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .dialog-header svg {
    width: 20px;
    height: 20px;
    color: var(--warning, #f59e0b);
  }

  .dialog-header h4 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .dialog-desc {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-secondary, #6b7280);
  }

  .file-list {
    margin: 0;
    padding: 0;
    list-style: none;
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 4px;
    padding: 8px;
    margin-bottom: 8px;
  }

  .file-list li {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    padding: 4px 0;
  }

  .file-name {
    color: var(--text-primary, #1f2937);
  }

  .file-size {
    color: var(--text-muted, #9ca3af);
  }

  .models-dir {
    margin: 0 0 12px;
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
  }

  .dialog-actions {
    display: flex;
    gap: 8px;
  }

  .download-btn, .manual-btn, .close-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    padding: 8px;
    border-radius: 4px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
  }

  .download-btn {
    background: var(--accent, #3b82f6);
    color: white;
    border: none;
  }

  .download-btn:hover {
    background: var(--accent-dark, #2563eb);
  }

  .download-btn svg {
    width: 14px;
    height: 14px;
  }

  .manual-btn {
    background: var(--bg-primary, #f9fafb);
    color: var(--text-primary, #1f2937);
    border: 1px solid var(--border, #e5e7eb);
  }

  .close-btn {
    background: transparent;
    color: var(--text-secondary, #6b7280);
    border: 1px solid var(--border, #e5e7eb);
  }

  .download-progress {
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 4px;
    padding: 10px;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 6px;
  }

  .progress-percent {
    font-weight: 600;
    color: var(--accent, #3b82f6);
  }

  .progress-bar {
    height: 6px;
    background: var(--border, #e5e7eb);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 6px;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent, #3b82f6);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .progress-bytes {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    margin-bottom: 8px;
  }

  .cancel-btn {
    width: 100%;
    padding: 6px;
    background: none;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
  }

  .cancel-btn:hover {
    border-color: var(--error, #ef4444);
    color: var(--error, #ef4444);
  }
</style>