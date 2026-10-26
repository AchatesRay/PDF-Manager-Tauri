<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { message } from '@tauri-apps/plugin-dialog';
  import {
    getModelOverview,
    setActiveModel,
    downloadOcrModels,
    cancelOcrDownload,
    refreshOcrStatus,
    type ModelOverview,
    type DownloadProgress,
  } from '../api';
  import {
    ocrModelStatus,
    ocrDownloadProgress,
    isDownloading,
  } from '../stores';

  // 本组件是「模型」页签的内容组件：遮罩/标题/✕ 由 SettingsDialog 统一提供；
  // active = 所在页签激活时加载概览
  export let active: boolean = false;

  const DOWNLOAD_SITE = 'https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0';
  // 备份镜像（2026-09-27 实测：三者均可用文件直链；ghproxy.net 页面亦可）
  // 用法 = 镜像前缀 + GitHub 完整地址
  const MIRRORS = [
    { prefix: 'https://ghproxy.net/', pageOk: true },
    { prefix: 'https://gh-proxy.com/', pageOk: false },
    { prefix: 'https://ghfast.top/', pageOk: true },
  ];
  // PP-OCRv5 Mobile 套件的直链示例
  const DIRECT_SAMPLE = 'https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_det.onnx';

  let overview: ModelOverview[] = [];
  let loading = false;
  let busyType: string | null = null; // 正在启用/禁用的模型类型
  let error: string | null = null;
  let copiedText: string | null = null;
  let cancelling = false; // 用户主动取消下载（抑制随之而来的「下载已取消」错误）

  // 页签激活时加载概览（组件随页签切换销毁重建，每次进入「模型」页都会刷新，
  // 等价于原「每次打开面板刷新」的行为）
  $: if (active) {
    loadOverview();
  }

  // 下载进度/完成/失败由后端事件推送（与 OcrModelSetup 共用同一套事件）
  onMount(() => {
    let unlistenProgress: (() => void) | null = null;
    let unlistenComplete: (() => void) | null = null;
    let unlistenError: (() => void) | null = null;

    listen<DownloadProgress>('model-download-progress', (event) => {
      ocrDownloadProgress.set(event.payload);
    }).then((u) => (unlistenProgress = u));

    listen<void>('model-download-complete', async () => {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      try {
        ocrModelStatus.set(await refreshOcrStatus());
      } catch (e) {
        console.error('Refresh OCR status failed:', e);
      }
      await loadOverview();
    }).then((u) => (unlistenComplete = u));

    listen<{ error: string }>('model-download-error', (event) => {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      // 用户主动取消：后端也会报「下载已取消」，这不是故障，不弹错误
      if (cancelling) {
        cancelling = false;
        return;
      }
      error = event.payload.error;
    }).then((u) => (unlistenError = u));

    return () => {
      unlistenProgress?.();
      unlistenComplete?.();
      unlistenError?.();
    };
  });

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

  // 手动刷新：用户按文字指引手动放入模型文件后重新扫描
  async function handleManualRefresh() {
    await loadOverview();
  }

  // 应用内下载：与后端事件联动显示进度，失败时错误条可手动关闭
  async function handleDownload() {
    error = null;
    cancelling = false;
    isDownloading.set(true);
    ocrDownloadProgress.set(null);
    try {
      await downloadOcrModels();
    } catch (e) {
      isDownloading.set(false);
      ocrDownloadProgress.set(null);
      error = String(e);
    }
  }

  function handleCancel() {
    cancelling = true;
    cancelOcrDownload();
    isDownloading.set(false);
    ocrDownloadProgress.set(null);
  }

  function formatProgress(current: number, total: number): string {
    if (total <= 0) return '0%';
    return `${Math.round((current / total) * 100)}%`;
  }

  // 启用：设为当前识别模型
  async function handleEnable(type: string) {
    busyType = type;
    try {
      const status = await setActiveModel(type);
      ocrModelStatus.set(status);
      await loadOverview();
    } catch (e) {
      await message(String(e), { title: '启用模型失败', kind: 'error' });
    } finally {
      busyType = null;
    }
  }

  // 禁用：取消当前模型（仅多套件场景展示，避免单套件禁用后无恢复入口）
  async function handleDisable(type: string) {
    busyType = type;
    try {
      const status = await setActiveModel(null);
      ocrModelStatus.set(status);
      await loadOverview();
    } catch (e) {
      await message(String(e), { title: '禁用模型失败', kind: 'error' });
    } finally {
      busyType = null;
    }
  }

  async function copyText(text: string) {
    try {
      await navigator.clipboard.writeText(text);
      copiedText = text;
      setTimeout(() => { if (copiedText === text) copiedText = null; }, 1500);
    } catch (e) {
      await message('复制失败: ' + e, { title: '复制', kind: 'error' });
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

  // 当前启用模型（用于顶部显示）：
  // 兼容旧设置（如 lite/server —— 卡片已删除但设置可能残留）：overview 找不到时回退显示原始类型
  $: activeRaw = overview.find(m => m.is_active) ?? null;
  $: activeTag = activeRaw
    ? `当前启用：${activeRaw.label}`
    : ($ocrModelStatus?.active_model ? `当前启用：${$ocrModelStatus.active_model}` : null);
  // 本地已完整下载的模型套件数：>=2 才显示启用/禁用切换（规则见模板内联条件）
  $: readySuites = overview.filter(m => m.ready).length;
</script>

<!-- 无外层遮罩/标题/✕，仅保留「下载/刷新」工具条 + 状态条 -->
<div class="model-pane">
  <div class="pane-toolbar">
    {#if $isDownloading}
      <button class="mini-btn" on:click={handleCancel} title="停止本次下载">取消下载</button>
    {:else}
      <button class="mini-btn primary" on:click={handleDownload} disabled={loading} title="在应用内下载 PP-OCRv5 Mobile 模型（约 300MB）">
        下载模型
      </button>
    {/if}
    <button class="mini-btn" on:click={handleManualRefresh} disabled={loading} title="重新扫描已下载的模型文件">
      {loading ? '扫描中...' : '刷新'}
    </button>
  </div>

  {#if $isDownloading}
    <div class="download-progress">
      {#if $ocrDownloadProgress}
        <div class="progress-info">
          <span class="progress-file" title={$ocrDownloadProgress.file}>{$ocrDownloadProgress.file}</span>
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
          {formatSize($ocrDownloadProgress.current)} / {formatSize($ocrDownloadProgress.total)}
        </div>
      {:else}
        <div class="progress-info">
          <span class="progress-file">正在连接下载服务器...</span>
        </div>
      {/if}
    </div>
  {/if}

  <div class="panel-status">
        {#if activeTag}
          <span class="active-tag">{activeTag}</span>
        {:else}
          <span class="disabled-tag">已禁用所有模型（OCR 暂不可用）</span>
        {/if}
      </div>

      <!-- 文字说明：下载网站 + 备用镜像 + 存放位置 + 详细步骤（替代应用内下载） -->
      <div class="download-info">
        <div class="info-row">
          <span class="info-label">模型下载网站：</span>
          <a href={DOWNLOAD_SITE} target="_blank" rel="noreferrer" class="info-link">{DOWNLOAD_SITE}</a>
          <button class="mini-btn" on:click={() => openUrl(DOWNLOAD_SITE)}>打开</button>
          <button class="mini-btn" on:click={() => copyText(DOWNLOAD_SITE)}>
            {copiedText === DOWNLOAD_SITE ? '已复制' : '复制'}
          </button>
          <button class="mini-btn" on:click={() => openUrl(MIRRORS[0].prefix + DOWNLOAD_SITE)} title="通过镜像打开下载页">
            镜像打开
          </button>
        </div>
        <div class="info-row">
          <span class="info-label">备用下载镜像：</span>
          <span class="info-sub">下载慢或打不开时，给下方文件直链加前缀：</span>
          {#each MIRRORS as m}
            <button class="mini-btn" on:click={() => copyText(m.prefix)} title="复制镜像前缀，粘贴到文件直链最前面">
              {copiedText === m.prefix ? '已复制' : m.prefix}
            </button>
          {/each}
        </div>
        <div class="info-row">
          <span class="info-label">文件直链示例：</span>
          <span class="info-dir" title={DIRECT_SAMPLE}>{DIRECT_SAMPLE}</span>
          <button class="mini-btn" on:click={() => copyText(DIRECT_SAMPLE)}>
            {copiedText === DIRECT_SAMPLE ? '已复制' : '复制'}
          </button>
        </div>
        <div class="info-row">
          <span class="info-label">模型文件存放位置：</span>
          <span class="info-dir" title={overview[0]?.models_dir || $ocrModelStatus?.models_dir || ''}>
            {overview[0]?.models_dir || $ocrModelStatus?.models_dir || ''}
          </span>
          <button class="mini-btn" on:click={() => copyText(overview[0]?.models_dir || $ocrModelStatus?.models_dir || '')}>
            {copiedText === (overview[0]?.models_dir || $ocrModelStatus?.models_dir) ? '已复制' : '复制'}
          </button>
        </div>
        <ol class="info-steps">
          <li>
            <b>需成套下载</b>：每套 3 个文件（det 检测模型 + rec 识别模型 + 字典 .txt）必须全部下载，缺一不可；
            本面板只提供一套 PP-OCRv5 Mobile 模型，<b>下载这一套即可</b>。
          </li>
          <li>
            <b>文件名必须原样保留</b>，不要重命名，3 个文件分别为：
            <code>pp-ocrv5_mobile_det.onnx</code>、<code>pp-ocrv5_mobile_rec.onnx</code>、<code>ppocrv5_dict.txt</code>
            （下载得到的名字就是这些，直接用即可）。
          </li>
          <li>
            <b>存放位置</b>：下载后直接放入上方「模型文件存放位置」目录，无需新建子目录。
          </li>
          <li>
            放好后点击右上角<b>「刷新」</b>，确认下方文件清单全部变成 ✓ 后再启用。
          </li>
        </ol>
      </div>

      {#if error}
        <div class="error-box">
          <span class="error-text">{error}</span>
          <button class="error-close" on:click={() => (error = null)} title="关闭提示" aria-label="关闭提示">
            ✕
          </button>
        </div>
      {/if}

      {#if loading && overview.length === 0}
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
                  </li>
                {/each}
              </ul>

              <!-- 启停可见性：多套件（>=2）按现状启停切换；单/零套件仅保留「启用」恢复入口 -->
              {#if model.ready && !model.is_active}
                <div class="card-actions">
                  <button
                    class="act-btn enable"
                    disabled={busyType !== null}
                    on:click={() => handleEnable(model.model_type)}
                  >
                    {busyType === model.model_type ? '启用中...' : '启用'}
                  </button>
                  <span class="hint">
                    {readySuites >= 2 ? '点击启用切换为当前识别模型' : '点击启用恢复为当前识别模型'}
                  </span>
                </div>
              {:else if readySuites >= 2 && model.ready && model.is_active}
                <div class="card-actions">
                  <button
                    class="act-btn disable"
                    disabled={busyType !== null}
                    on:click={() => handleDisable(model.model_type)}
                  >
                    {busyType === model.model_type ? '禁用中...' : '禁用'}
                  </button>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
</div>

<style>
  .model-pane {
    padding: 14px 16px;
  }

  .pane-toolbar {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-bottom: 10px;
  }

  .download-progress {
    padding: 8px 10px;
    margin-bottom: 10px;
    background: var(--bg-tertiary, #f3f4f6);
    border: 1px solid var(--border-light, #f3f4f6);
    border-radius: 6px;
    font-size: 11px;
  }

  .progress-info {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
  }

  .progress-file {
    flex: 1;
    min-width: 0;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .progress-percent {
    flex-shrink: 0;
    color: var(--accent, #3b82f6);
    font-weight: 600;
  }

  .progress-bar {
    height: 6px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 3px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent, #3b82f6);
    transition: width 0.2s ease;
  }

  .progress-bytes {
    margin-top: 3px;
    color: var(--text-muted, #9ca3af);
    font-size: 10px;
  }

  .panel-status {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
    margin-bottom: 10px;
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

  /* 文字说明区 */
  .download-info {
    background: var(--bg-tertiary, #f3f4f6);
    border: 1px solid var(--border-light, #f3f4f6);
    border-radius: 6px;
    padding: 10px;
    margin-bottom: 12px;
    font-size: 11px;
  }

  .info-row {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
    padding: 3px 0;
  }

  .info-label {
    color: var(--text-secondary, #6b7280);
    flex-shrink: 0;
    font-weight: 500;
  }

  .info-link {
    color: var(--accent, #3b82f6);
    word-break: break-all;
  }

  .info-dir {
    color: var(--text-primary, #1f2937);
    word-break: break-all;
  }

  .info-sub {
    color: var(--text-muted, #9ca3af);
  }

  .info-steps {
    margin: 8px 0 0;
    padding-left: 18px;
    color: var(--text-secondary, #6b7280);
    line-height: 1.6;
  }

  .info-steps li {
    margin-bottom: 4px;
  }

  .info-steps b {
    color: var(--text-primary, #1f2937);
  }

  .info-steps code {
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 3px;
    padding: 0 4px;
    font-size: 10px;
    color: var(--text-primary, #1f2937);
    word-break: break-all;
  }

  .error-box {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    padding: 8px;
    background: var(--error-soft, #fef2f2);
    border: 1px solid var(--error, #ef4444);
    border-radius: 4px;
    color: var(--error, #ef4444);
    font-size: 11px;
    margin-bottom: 10px;
    word-break: break-all;
  }

  .error-text {
    flex: 1;
    min-width: 0;
  }

  .error-close {
    flex-shrink: 0;
    background: none;
    border: none;
    color: inherit;
    cursor: pointer;
    font-size: 11px;
    line-height: 1;
    padding: 2px 4px;
    border-radius: 3px;
    opacity: 0.75;
  }

  .error-close:hover {
    opacity: 1;
    background: rgba(239, 68, 68, 0.12);
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
    margin: 0;
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

  .card-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
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

  .mini-btn {
    padding: 2px 7px;
    font-size: 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 3px;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
    white-space: nowrap;
  }

  .mini-btn:hover:not(:disabled) {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .mini-btn.primary {
    background: var(--accent, #3b82f6);
    border-color: var(--accent, #3b82f6);
    color: #ffffff;
  }

  .mini-btn.primary:hover:not(:disabled) {
    background: #2563eb;
    border-color: #2563eb;
    color: #ffffff;
  }

  .mini-btn:disabled {
    opacity: 0.6;
    cursor: wait;
  }
</style>
