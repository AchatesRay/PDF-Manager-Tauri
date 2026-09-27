<script lang="ts">
  import { onMount } from 'svelte';
  import { message } from '@tauri-apps/plugin-dialog';
  import {
    getModelOverview,
    setActiveModel,
    type ModelOverview,
  } from '../api';
  import { ocrModelStatus } from '../stores';

  export let show: boolean = false;
  export let onClose: () => void = () => {};

  const DOWNLOAD_SITE = 'https://github.com/GreatV/oar-ocr/releases/tag/v0.3.0';
  // 备份镜像（2026-09-27 实测：三者均可用文件直链；ghproxy.net 页面亦可）
  // 用法 = 镜像前缀 + GitHub 完整地址
  const MIRRORS = [
    { prefix: 'https://ghproxy.net/', pageOk: true },
    { prefix: 'https://gh-proxy.com/', pageOk: false },
    { prefix: 'https://ghfast.top/', pageOk: true },
  ];
  // mobile / balanced 套件（同一套文件）的直链示例
  const DIRECT_SAMPLE = 'https://github.com/GreatV/oar-ocr/releases/download/v0.3.0/pp-ocrv5_mobile_det.onnx';

  let overview: ModelOverview[] = [];
  let loading = false;
  let busyType: string | null = null; // 正在启用/禁用的模型类型
  let error: string | null = null;
  let copiedText: string | null = null;

  onMount(() => {
    // 面板首次打开时加载（show 变化时由 reactive 语句兜底刷新）
    if (show) loadOverview();
  });

  // 面板每次打开时加载概览（依赖仅 show，不会因 loadOverview 内部状态形成循环）
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

  // 手动刷新：用户按文字指引手动放入模型文件后重新扫描
  async function handleManualRefresh() {
    await loadOverview();
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

{#if show}
  <div class="panel-overlay" on:click={onClose}>
    <div class="panel" on:click|stopPropagation>
      <div class="panel-header">
        <h4>OCR 模型配置</h4>
        <div class="header-acts">
          <button class="mini-btn" on:click={handleManualRefresh} disabled={loading} title="重新扫描已下载的模型文件">
            {loading ? '扫描中...' : '刷新'}
          </button>
          <button class="close-x" on:click={onClose} title="关闭">✕</button>
        </div>
      </div>

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
            Mobile 与 Balanced 是同一套文件（PP-OCRv5 Mobile），<b>下载一套即可通用</b>，无需重复下载。
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
        <div class="error-box">{error}</div>
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

  .header-acts {
    display: flex;
    align-items: center;
    gap: 6px;
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

  .mini-btn:disabled {
    opacity: 0.6;
    cursor: wait;
  }
</style>
