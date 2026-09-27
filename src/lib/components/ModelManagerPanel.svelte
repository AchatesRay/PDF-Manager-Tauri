<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { message } from '@tauri-apps/plugin-dialog';
  import {
    getModelOverview,
    setActiveModel,
    downloadOcrModels,
    cancelOcrDownload,
    type ModelOverview,
    type DownloadProgress,
  } from '../api';
  import { ocrDownloadProgress, isDownloading, ocrModelStatus } from '../stores';

  export let show: boolean = false;
  export let onClose: () => void = () => {};

  let overview: ModelOverview[] = [];
  let loading = false;
  let busyType: string | null = null; // 正在启用/禁用的模型类型
  let error: string | null = null;
  let copiedUrl: string | null = null;

  onMount(() => {
    let unlistenProgress: (() => void) | null = null;
    let unlistenComplete: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    listen<DownloadProgress>('model-download-progress', (event) => {
      ocrDownloadProgress.set(event.payload);
    }).then(u => unlistenProgress = u);

    listen<void>('model-download-complete', async () => {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      await refresh();
    }).then(u => unlistenComplete = u);

    listen<{ error: string }>('model-download-error', (event) => {
      isDownloading.set(false);
      error = event.payload.error;
    }).then(u => unlistenError = u);

    return () => {
      unlistenProgress?.();
      unlistenComplete?.();
      unlistenError?.();
    };
  });

  // 面板打开时加载概览
  $: if (show) {
    loadOverview();
  }

  async function loadOverview() {
    loading = true;
    error = null;
    try {
      overview = await getModelOverview();
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function refresh() {
    try {
      overview = await getModelOverview();
    } catch (e) {
      error = String(e);
    }
  }

  async function handleDownload(type: string) {
    error = null;
    isDownloading.set(true);
    ocrDownloadProgress.set(null);
    try {
      await downloadOcrModels(type);
    } catch (e) {
      isDownloading.set(false);
      error = String(e);
    }
  }

  function handleCancelDownload() {
    cancelOcrDownload();
    isDownloading.set(false);
    ocrDownloadProgress.set(null);
  }

  // 启用：设为当前识别模型
  async function handleEnable(type: string) {
    busyType = type;
    try {
      const status = await setActiveModel(type);
      ocrModelStatus.set(status);
      await refresh();
    } catch (e) {
      await message(String(e), { title: '启用模型失败', kind: 'error' });
    } finally {
      busyType = null;
    }
  }

  // 禁用：取消当前模型（OCR 暂不可用，直到启用其它模型）
  async function handleDisable(type: string) {
    busyType = type;
    try {
      const status = await setActiveModel(null);
      ocrModelStatus.set(status);
      await refresh();
    } catch (e) {
      await message(String(e), { title: '禁用模型失败', kind: 'error' });
    } finally {
      busyType = null;
    }
  }

  async function copyUrl(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      copiedUrl = url;
      setTimeout(() => { if (copiedUrl === url) copiedUrl = null; }, 1500);
    } catch (e) {
      await message('复制失败: ' + e, { title: '复制 URL', kind: 'error' });
    }
  }

  function openUrl(url: string) {
    window.open(url, '_blank');
  }

  function formatSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  function formatProgress(current: number, total: number): string {
    if (total === 0) return '0%';
    return `${Math.round((current / total) * 100)}%`;
  }

  // 当前启用模型（用于顶部显示）
  $: activeType = overview.find(m => m.is_active)?.model_type ?? null;
  $: activeLabel = overview.find(m => m.is_active)?.label ?? null;
</script>

{#if show}
  <div class="panel-overlay" on:click={onClose}>
    <div class="panel" on:click|stopPropagation>
      <div class="panel-header">
        <h4>OCR 模型管理</h4>
        <button class="close-x" on:click={onClose} title="关闭">✕</button>
      </div>

      <div class="panel-status">
        {#if activeType}
          <span class="active-tag">当前启用：{activeLabel}</span>
        {:else}
          <span class="disabled-tag">已禁用所有模型（OCR 暂不可用）</span>
        {/if}
        <span class="dir">模型目录: {overview[0]?.models_dir || $ocrModelStatus?.models_dir || ''}</span>
      </div>

      {#if error}
        <div class="error-box">{error}</div>
      {/if}

      {#if loading}
        <div class="loading">加载中...</div>
      {:else}
        <div class="cards">
          {#each overview as model (model.model_type)}
            <div class="model-card" class:active-card={model.is_active}>
              <div class="card-head">
                <div class="card-title">
                  <span class="type-name">{model.model_type}</span>
                  {#if model.is_active}
                    <span class="badge active-badge">启用中</span>
                  {:else if model.ready}
                    <span class="badge ready-badge">已下载</span>
                  {:else}
                    <span class="badge not-badge">未下载</span>
                  {/if}
                </div>
                <span class="label">{model.label}</span>
              </div>

              <ul class="file-list">
                {#each model.files as file (file.name)}
                  <li class="file-row">
                    <span class="file-name" title={file.name}>
                      {file.downloaded ? '✓' : '○'} {file.name}
                    </span>
                    <span class="file-size">{formatSize(file.size)}</span>
                    <span class="file-acts">
                      <button class="mini-btn" on:click={() => copyUrl(file.url)} title="复制下载地址">
                        {copiedUrl === file.url ? '已复制' : '复制URL'}
                      </button>
                      <button class="mini-btn" on:click={() => openUrl(file.url)} title="浏览器打开下载地址">打开</button>
                    </span>
                  </li>
                {/each}
              </ul>

              <div class="card-actions">
                {#if !model.ready}
                  <button
                    class="act-btn download"
                    disabled={$isDownloading}
                    on:click={() => handleDownload(model.model_type)}
                  >
                    下载模型
                  </button>
                {:else if !model.is_active}
                  <button
                    class="act-btn enable"
                    disabled={busyType !== null || $isDownloading}
                    on:click={() => handleEnable(model.model_type)}
                  >
                    {busyType === model.model_type ? '启用中...' : '启用'}
                  </button>
                {:else}
                  <button
                    class="act-btn disable"
                    disabled={busyType !== null || $isDownloading}
                    on:click={() => handleDisable(model.model_type)}
                  >
                    {busyType === model.model_type ? '禁用中...' : '禁用'}
                  </button>
                {/if}
                {#if model.ready && !model.is_active}
                  <span class="hint">已下载，可启用为当前识别模型</span>
                {/if}
              </div>
            </div>
          {/each}
        </div>
      {/if}

      <!-- 下载进度（与既有下载对话框共用事件） -->
      {#if $isDownloading && $ocrDownloadProgress}
        <div class="download-progress">
          <div class="progress-info">
            <span>{$ocrDownloadProgress.file}</span>
            <span class="percent">{formatProgress($ocrDownloadProgress.current, $ocrDownloadProgress.total)}</span>
          </div>
          <div class="progress-bar">
            <div
              class="progress-fill"
              style="width: {($ocrDownloadProgress.current / ($ocrDownloadProgress.total || 1)) * 100}%"
            ></div>
          </div>
          <button class="cancel-btn" on:click={handleCancelDownload}>取消下载</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .panel-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .panel {
    background: var(--bg-secondary, #ffffff);
    border-radius: 8px;
    padding: 16px;
    max-width: 640px;
    width: 92%;
    max-height: 85vh;
    overflow-y: auto;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.2);
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .panel-header h4 {
    margin: 0;
    font-size: 15px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .close-x {
    width: 26px;
    height: 26px;
    border: 1px solid var(--border, #e5e7eb);
    background: none;
    border-radius: 4px;
    cursor: pointer;
    color: var(--text-secondary, #6b7280);
  }

  .close-x:hover {
    border-color: var(--error, #ef4444);
    color: var(--error, #ef4444);
  }

  .panel-status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    font-size: 11px;
  }

  .active-tag {
    padding: 3px 8px;
    background: var(--success, #10b981);
    color: white;
    border-radius: 10px;
    font-weight: 500;
  }

  .disabled-tag {
    padding: 3px 8px;
    background: var(--warning, #f59e0b);
    color: white;
    border-radius: 10px;
    font-weight: 500;
  }

  .dir {
    color: var(--text-muted, #9ca3af);
    word-break: break-all;
  }

  .error-box {
    padding: 8px;
    background: var(--error-soft, #fef2f2);
    border: 1px solid var(--error, #ef4444);
    border-radius: 4px;
    color: var(--error, #ef4444);
    font-size: 11px;
    margin-bottom: 10px;
    word-break: break-all;
  }

  .loading {
    padding: 20px;
    text-align: center;
    color: var(--text-muted, #9ca3af);
    font-size: 12px;
  }

  .cards {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }

  @media (max-width: 640px) {
    .cards { grid-template-columns: 1fr; }
  }

  .model-card {
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 6px;
    padding: 10px;
    background: var(--bg-primary, #f9fafb);
  }

  .model-card.active-card {
    border-color: var(--success, #10b981);
    background: var(--success-soft, #ecfdf5);
  }

  .card-head {
    margin-bottom: 8px;
  }

  .card-title {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 3px;
  }

  .type-name {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
    text-transform: uppercase;
  }

  .badge {
    font-size: 10px;
    padding: 1px 6px;
    border-radius: 8px;
    font-weight: 500;
  }

  .active-badge { background: var(--success, #10b981); color: white; }
  .ready-badge { background: var(--accent-soft, #eff6ff); color: var(--accent, #3b82f6); }
  .not-badge { background: var(--bg-tertiary, #f3f4f6); color: var(--text-muted, #9ca3af); }

  .label {
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
  }

  .file-list {
    list-style: none;
    margin: 0 0 8px;
    padding: 6px;
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 4px;
  }

  .file-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 0;
    font-size: 11px;
  }

  .file-name {
    flex: 1;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .file-size {
    color: var(--text-muted, #9ca3af);
    flex-shrink: 0;
  }

  .file-acts {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .mini-btn {
    padding: 1px 5px;
    font-size: 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 3px;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
    white-space: nowrap;
  }

  .mini-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .card-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .act-btn {
    padding: 5px 14px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 500;
    cursor: pointer;
    border: none;
  }

  .act-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .act-btn.download {
    background: var(--accent, #3b82f6);
    color: white;
  }

  .act-btn.download:hover:not(:disabled) { background: var(--accent-dark, #2563eb); }

  .act-btn.enable {
    background: var(--success, #10b981);
    color: white;
  }

  .act-btn.enable:hover:not(:disabled) { background: #059669; }

  .act-btn.disable {
    background: transparent;
    color: var(--error, #ef4444);
    border: 1px solid var(--error, #ef4444);
  }

  .act-btn.disable:hover:not(:disabled) {
    background: var(--error, #ef4444);
    color: white;
  }

  .hint {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
  }

  .download-progress {
    margin-top: 12px;
    background: var(--bg-tertiary, #f3f4f6);
    border-radius: 4px;
    padding: 10px;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    font-size: 11px;
    margin-bottom: 6px;
    color: var(--text-primary, #1f2937);
  }

  .percent {
    font-weight: 600;
    color: var(--accent, #3b82f6);
  }

  .progress-bar {
    height: 6px;
    background: var(--border, #e5e7eb);
    border-radius: 3px;
    overflow: hidden;
    margin-bottom: 8px;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent, #3b82f6);
    transition: width 0.3s ease;
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
