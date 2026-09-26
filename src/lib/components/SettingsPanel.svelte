<script lang="ts">
  import { open, confirm } from '@tauri-apps/plugin-dialog';
  import { getSettings, setDataDir, resetDataDir, setPdfReader, setOcrMaxImageDimension, setOcrPreprocessMode } from '../api';
  import { onMount } from 'svelte';
  import type { AppSettings, PreprocessMode } from '../api';

  let settings: AppSettings | null = null;
  let ocrDimensionInput: string = '';
  let preprocessMode: PreprocessMode = 'auto';

  onMount(() => {
    loadSettings();
  });

  async function loadSettings() {
    try {
      settings = await getSettings();
      if (settings) {
        ocrDimensionInput = settings.ocr_max_image_dimension.toString();
        preprocessMode = settings.ocr_preprocess_mode;
      }
    } catch (e) {
      console.error('Failed to load settings:', e);
    }
  }

  async function selectDataDir() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择数据存储目录',
    });

    if (selected) {
      try {
        await setDataDir(selected as string);
        await loadSettings();
        alert('数据目录已更新，重启应用后生效');
      } catch (e) {
        alert('设置失败: ' + e);
      }
    }
  }

  async function handleResetDataDir() {
    const confirmed = await confirm('确定重置数据目录为默认值？', {
      title: '确认重置',
      kind: 'warning',
    });

    if (confirmed) {
      try {
        await resetDataDir();
        await loadSettings();
        alert('数据目录已重置，重启应用后生效');
      } catch (e) {
        alert('重置失败: ' + e);
      }
    }
  }

  async function selectPdfReader() {
    const selected = await open({
      multiple: false,
      filters: [{ name: '可执行文件', extensions: ['exe'] }],
      title: '选择PDF阅读器',
    });

    if (selected) {
      try {
        await setPdfReader(selected as string);
        await loadSettings();
        alert('PDF阅读器已设置');
      } catch (e) {
        alert('设置失败: ' + e);
      }
    }
  }

  async function clearPdfReader() {
    const confirmed = await confirm('确定清除PDF阅读器设置？将使用系统默认程序打开PDF。', {
      title: '确认清除',
      kind: 'warning',
    });

    if (confirmed) {
      try {
        await setPdfReader(null);
        await loadSettings();
        alert('PDF阅读器设置已清除');
      } catch (e) {
        alert('清除失败: ' + e);
      }
    }
  }

  async function saveOcrDimension() {
    const dimension = parseInt(ocrDimensionInput, 10);
    // 与后端校验一致：MIN/MAX = 500/2000（原实现写 500-4000 会放行到后端被拒）
    if (isNaN(dimension) || dimension < 500 || dimension > 2000) {
      alert('请输入 500-2000 之间的数字');
      return;
    }
    try {
      await setOcrMaxImageDimension(dimension);
      await loadSettings();
      alert('OCR 图像尺寸已保存');
    } catch (e) {
      alert('保存失败: ' + e);
    }
  }

  async function savePreprocessMode() {
    try {
      await setOcrPreprocessMode(preprocessMode);
      await loadSettings();
      alert('预处理模式已保存，对下一个 OCR 任务生效');
    } catch (e) {
      alert('保存失败: ' + e);
      await loadSettings();
    }
  }
</script>

<div class="settings-panel">
  <div class="settings-title">存储设置</div>
  <div class="settings-item">
    <label>数据目录</label>
    <div class="settings-path">{settings?.data_dir || '加载中...'}</div>
    <div class="settings-actions">
      <button on:click={selectDataDir}>选择目录</button>
      <button class="secondary-btn" on:click={handleResetDataDir}>重置</button>
    </div>
  </div>
  <div class="settings-item">
    <label>日志目录</label>
    <div class="settings-path">{settings?.log_dir || '加载中...'}</div>
  </div>
  <div class="settings-divider"></div>
  <div class="settings-title">PDF阅读器</div>
  <div class="settings-item">
    <label>外部阅读器</label>
    <div class="settings-path">{settings?.pdf_reader_path || '使用系统默认'}</div>
    <div class="settings-actions">
      <button on:click={selectPdfReader}>选择阅读器</button>
      {#if settings?.pdf_reader_path}
        <button class="secondary-btn" on:click={clearPdfReader}>清除</button>
      {/if}
    </div>
  </div>
  <div class="settings-divider"></div>
  <div class="settings-title">OCR 设置</div>
  <div class="settings-item">
    <label>最大图像尺寸 (像素)</label>
    <div class="settings-hint">控制 OCR 处理时的图像大小，较小值可减少内存占用。范围: 500-2000</div>
    <div class="settings-row">
      <input
        type="number"
        min="500"
        max="2000"
        bind:value={ocrDimensionInput}
        placeholder="1000"
      />
      <button on:click={saveOcrDimension}>保存</button>
    </div>
  </div>
  <div class="settings-item">
    <label>图像预处理</label>
    <div class="settings-hint">
      自动=按图像质量增强（推荐）；关闭=原图直出（识别异常时可排除预处理干扰）；强制增强=总是做对比度拉伸
    </div>
    <div class="settings-row">
      <select bind:value={preprocessMode} on:change={savePreprocessMode}>
        <option value="auto">自动</option>
        <option value="off">关闭</option>
        <option value="on">强制增强</option>
      </select>
    </div>
  </div>
</div>

<style>
  .settings-panel {
    background: var(--bg-tertiary, #f5f7f9);
    border-bottom: 1px solid var(--border, #e5e7eb);
    padding: 12px;
    font-size: 12px;
  }

  .settings-title {
    font-weight: 600;
    color: var(--text-primary, #1f2937);
    margin-bottom: 10px;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.3px;
  }

  .settings-item {
    margin-bottom: 10px;
  }

  .settings-item label {
    display: block;
    color: var(--text-secondary, #6b7280);
    margin-bottom: 4px;
    font-size: 11px;
  }

  .settings-path {
    background: var(--bg-secondary, #ffffff);
    padding: 6px 10px;
    border-radius: 6px;
    word-break: break-all;
    margin-bottom: 6px;
    font-size: 11px;
    color: var(--text-primary, #1f2937);
    border: 1px solid var(--border-light, #f3f4f6);
  }

  .settings-actions {
    display: flex;
    gap: 6px;
  }

  .settings-actions button {
    padding: 5px 10px;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 11px;
    background: var(--accent, #3b82f6);
    color: white;
    transition: background 0.15s;
  }

  .settings-actions button:hover {
    background: #2563eb;
  }

  .settings-actions .secondary-btn {
    background: var(--text-muted, #9ca3af);
  }

  .settings-divider {
    height: 1px;
    background: var(--border, #e5e7eb);
    margin: 12px 0;
  }

  .settings-hint {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    margin-bottom: 6px;
  }

  .settings-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .settings-row input,
  .settings-row select {
    flex: 1;
    padding: 5px 10px;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 5px;
    font-size: 11px;
    background: var(--bg-secondary, #ffffff);
  }

  .settings-row input:focus,
  .settings-row select:focus {
    outline: none;
    border-color: var(--accent, #3b82f6);
  }

  .settings-row button {
    padding: 5px 10px;
    border: none;
    border-radius: 5px;
    cursor: pointer;
    font-size: 11px;
    background: var(--accent, #3b82f6);
    color: white;
    transition: background 0.15s;
  }

  .settings-row button:hover {
    background: #2563eb;
  }
</style>
