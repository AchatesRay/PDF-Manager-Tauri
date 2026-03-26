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
  import { ocrModelStatus, ocrDownloadProgress, isDownloading } from '../stores';

  let showManualGuide = false;
  let downloadGuides: DownloadGuide[] = [];
  let error: string | null = null;

  onMount(async () => {
    await checkStatus();
    downloadGuides = await getOcrDownloadGuide();

    // 监听下载进度
    const unlistenProgress = await listen<DownloadProgress>('model-download-progress', (event) => {
      ocrDownloadProgress.set(event.payload);
    });

    // 监听下载完成
    const unlistenComplete = await listen<void>('model-download-complete', async () => {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      await checkStatus();
    });

    // 监听下载错误
    const unlistenError = await listen<{ error: string }>('model-download-error', (event) => {
      isDownloading.set(false);
      error = event.payload.error;
    });

    return () => {
      unlistenProgress();
      unlistenComplete();
      unlistenError();
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
</script>

{#if !$ocrModelStatus?.models_ready}
  <div class="ocr-setup">
    <div class="setup-header">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/>
      </svg>
      <h3>OCR 模型未安装</h3>
    </div>

    <p class="setup-desc">
      使用 PaddleOCR 进行文字识别需要下载模型文件（约 30MB）。
    </p>

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
      <div class="actions">
        <button class="download-btn" on:click={handleDownload} disabled={$isDownloading}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 15v4a2 2 0 01-2 2H5a2 2 0 01-2-2v-4"/>
            <polyline points="7 10 12 15 17 10"/>
            <line x1="12" y1="15" x2="12" y2="3"/>
          </svg>
          在线下载
        </button>
        <button class="manual-btn" on:click={() => showManualGuide = !showManualGuide}>
          手动下载
        </button>
      </div>

      {#if showManualGuide}
        <div class="manual-guide">
          <h4>手动下载步骤：</h4>
          <ol>
            <li>下载以下文件并放入目录：<code>{$ocrModelStatus?.models_dir || 'models'}</code></li>
          </ol>
          <div class="file-list">
            {#each downloadGuides as guide}
              <div class="file-item">
                <a href={guide.url} target="_blank" rel="noopener noreferrer" class="file-link">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M18 13v6a2 2 0 01-2 2H5a2 2 0 01-2-2V8a2 2 0 012-2h6"/>
                    <polyline points="15 3 21 3 21 9"/>
                    <line x1="10" y1="14" x2="21" y2="3"/>
                  </svg>
                  {guide.name}
                </a>
                <span class="file-size">{guide.size}</span>
              </div>
            {/each}
          </div>
          <p class="guide-note">下载完成后重启应用即可使用 OCR 功能。</p>
        </div>
      {/if}
    {/if}
  </div>
{/if}

<style>
  .ocr-setup {
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 8px;
    padding: 16px;
    margin: 12px 8px;
  }

  .setup-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 12px;
  }

  .setup-header svg {
    width: 24px;
    height: 24px;
    color: var(--warning, #f59e0b);
  }

  .setup-header h3 {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .setup-desc {
    margin: 0 0 16px;
    font-size: 13px;
    color: var(--text-secondary, #6b7280);
    line-height: 1.5;
  }

  .error-message {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px;
    background: var(--error-soft, #fef2f2);
    border: 1px solid var(--error, #ef4444);
    border-radius: 6px;
    margin-bottom: 12px;
    font-size: 12px;
    color: var(--error, #ef4444);
  }

  .error-message svg {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .actions {
    display: flex;
    gap: 10px;
    margin-bottom: 12px;
  }

  .download-btn, .manual-btn {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 10px 16px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
  }

  .download-btn {
    background: var(--accent, #3b82f6);
    color: white;
    border: none;
  }

  .download-btn:hover:not(:disabled) {
    background: var(--accent-dark, #2563eb);
  }

  .download-btn:disabled {
    opacity: 0.6;
    cursor: not-allowed;
  }

  .download-btn svg, .manual-btn svg {
    width: 16px;
    height: 16px;
  }

  .manual-btn {
    background: var(--bg-primary, #f9fafb);
    color: var(--text-primary, #1f2937);
    border: 1px solid var(--border, #e5e7eb);
  }

  .manual-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .download-progress {
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 6px;
    padding: 12px;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .file-name {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
  }

  .progress-percent {
    font-size: 12px;
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
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    margin-bottom: 10px;
  }

  .cancel-btn {
    width: 100%;
    padding: 8px;
    background: none;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
    transition: all 0.15s;
  }

  .cancel-btn:hover {
    border-color: var(--error, #ef4444);
    color: var(--error, #ef4444);
  }

  .manual-guide {
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 6px;
    padding: 12px;
  }

  .manual-guide h4 {
    margin: 0 0 8px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .manual-guide ol {
    margin: 0;
    padding-left: 20px;
  }

  .manual-guide li {
    font-size: 12px;
    color: var(--text-secondary, #6b7280);
    margin-bottom: 8px;
  }

  .manual-guide code {
    background: var(--bg-secondary, #ffffff);
    padding: 2px 6px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--accent, #3b82f6);
    word-break: break-all;
  }

  .file-list {
    margin-top: 10px;
  }

  .file-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border-light, #f3f4f6);
    border-radius: 4px;
    margin-bottom: 6px;
  }

  .file-link {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
    color: var(--accent, #3b82f6);
    text-decoration: none;
  }

  .file-link:hover {
    text-decoration: underline;
  }

  .file-link svg {
    width: 14px;
    height: 14px;
  }

  .file-size {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
  }

  .guide-note {
    margin: 10px 0 0;
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
  }
</style>