<script lang="ts">
  import { onMount } from 'svelte';
  import { getVersion } from '@tauri-apps/api/app';
  import { showSettings, settingsTab, type SettingsTab } from '../stores';
  import SettingsPanel from './SettingsPanel.svelte';
  import ModelManagerPanel from './ModelManagerPanel.svelte';
  import { RELEASES, APP_FEATURES, APP_NOTES, APP_LINKS } from '../about';

  const TABS: { id: SettingsTab; label: string }[] = [
    { id: 'general', label: '通用' },
    { id: 'model', label: '模型' },
    { id: 'about', label: '关于' },
  ];

  let version = '';

  onMount(async () => {
    try {
      version = await getVersion();
    } catch (e) {
      console.error('getVersion failed:', e);
      version = '1.1.0';
    }
  });

  $: releaseDate = RELEASES[version] ?? '—';

  function close() {
    showSettings.set(false);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Escape' && $showSettings) {
      close();
    }
  }

  function openUrl(url: string) {
    window.open(url, '_blank');
  }
</script>

<svelte:window on:keydown={handleKeydown} />

{#if $showSettings}
  <div class="settings-overlay" on:click={close}>
    <div class="settings-dialog" role="dialog" aria-label="设置" on:click|stopPropagation>
      <div class="dialog-header">
        <h4>设置</h4>
        <button class="close-x" on:click={close} title="关闭 (Esc)">✕</button>
      </div>

      <div class="dialog-body">
        <!-- 左侧竖排页签 -->
        <nav class="tab-rail">
          {#each TABS as tab (tab.id)}
            <button
              class="tab-btn"
              class:active={$settingsTab === tab.id}
              on:click={() => settingsTab.set(tab.id)}
            >
              {tab.label}
            </button>
          {/each}
        </nav>

        <!-- 右侧内容区（独立滚动） -->
        <div class="tab-content">
          {#if $settingsTab === 'general'}
            <SettingsPanel />
          {:else if $settingsTab === 'model'}
            <ModelManagerPanel active={true} />
          {:else}
            <div class="about-pane">
              <div class="about-head">
                <div class="app-name">PDF Manager</div>
                <div class="app-meta">
                  <span class="meta-item">当前版本：{version || '...'}</span>
                  <span class="meta-item">版本更新日期：{releaseDate}</span>
                </div>
              </div>

              <div class="about-section">
                <div class="section-title">功能简介</div>
                <ul class="feature-list">
                  {#each APP_FEATURES as f (f.title)}
                    <li>
                      <span class="feature-title">{f.title}</span>
                      <span class="feature-desc">{f.desc}</span>
                    </li>
                  {/each}
                </ul>
              </div>

              <div class="about-section">
                <div class="section-title">说明</div>
                <ul class="note-list">
                  {#each APP_NOTES as note (note)}
                    <li>{note}</li>
                  {/each}
                </ul>
              </div>

              <div class="about-section">
                <div class="section-title">相关链接</div>
                {#each APP_LINKS as link (link.url)}
                  <div class="link-row">
                    <span class="link-label">{link.label}：</span>
                    <a href={link.url} target="_blank" rel="noreferrer" class="link-url">{link.url}</a>
                    <button class="mini-btn" on:click={() => openUrl(link.url)}>打开</button>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-overlay {
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

  .settings-dialog {
    background: var(--bg-secondary, #ffffff);
    border-radius: 8px;
    width: min(92vw, 780px);
    max-height: 85vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 4px 24px rgba(0, 0, 0, 0.2);
  }

  .dialog-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border, #e5e7eb);
    flex-shrink: 0;
  }

  .dialog-header h4 {
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

  .dialog-body {
    display: flex;
    min-height: 0;
    flex: 1;
  }

  /* 左侧竖排页签 */
  .tab-rail {
    width: 130px;
    flex-shrink: 0;
    border-right: 1px solid var(--border, #e5e7eb);
    background: var(--bg-tertiary, #f5f7f9);
    padding: 10px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .tab-btn {
    padding: 8px 12px;
    border: none;
    background: transparent;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
    text-align: left;
    transition: all 0.15s;
  }

  .tab-btn:hover {
    background: var(--bg-secondary, #ffffff);
    color: var(--text-primary, #1f2937);
  }

  .tab-btn.active {
    background: var(--accent-soft, #eff6ff);
    color: var(--accent, #3b82f6);
    font-weight: 600;
  }

  /* 右侧内容区 */
  .tab-content {
    flex: 1;
    min-width: 0;
    overflow-y: auto;
    font-size: 12px;
  }

  /* 关于页 */
  .about-pane {
    padding: 16px;
  }

  .about-head {
    margin-bottom: 16px;
  }

  .app-name {
    font-size: 16px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
    margin-bottom: 6px;
  }

  .app-meta {
    display: flex;
    gap: 16px;
    flex-wrap: wrap;
  }

  .meta-item {
    font-size: 12px;
    color: var(--text-secondary, #6b7280);
  }

  .about-section {
    margin-bottom: 16px;
  }

  .about-section:last-child {
    margin-bottom: 0;
  }

  .section-title {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
    text-transform: uppercase;
    letter-spacing: 0.3px;
    margin-bottom: 8px;
  }

  .feature-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .feature-list li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    background: var(--bg-tertiary, #f5f7f9);
    border-radius: 6px;
    padding: 8px 10px;
  }

  .feature-title {
    flex-shrink: 0;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
    font-size: 12px;
  }

  .feature-desc {
    color: var(--text-secondary, #6b7280);
    font-size: 11px;
    line-height: 1.5;
  }

  .note-list {
    margin: 0;
    padding-left: 18px;
    color: var(--text-secondary, #6b7280);
    font-size: 11px;
    line-height: 1.7;
  }

  .note-list li {
    margin-bottom: 2px;
  }

  .link-row {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 0;
    flex-wrap: wrap;
  }

  .link-label {
    color: var(--text-secondary, #6b7280);
    flex-shrink: 0;
    font-weight: 500;
  }

  .link-url {
    color: var(--accent, #3b82f6);
    word-break: break-all;
    font-size: 11px;
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
    flex-shrink: 0;
  }

  .mini-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }
</style>
