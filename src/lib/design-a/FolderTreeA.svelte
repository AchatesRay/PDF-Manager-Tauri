<script lang="ts">
  import { open, confirm } from '@tauri-apps/plugin-dialog';
  import { folders, selectedFolderId, isLoading, pdfList } from '../stores';
  import { getFolders, createFolder, deleteFolder, getSettings, setDataDir, resetDataDir, setPdfReader } from '../api';
  import { onMount } from 'svelte';
  import FolderNodeA from './FolderNodeA.svelte';
  import type { Folder, AppSettings } from '../api';

  interface TreeNode extends Folder {
    children: TreeNode[];
  }

  let newFolderName = '';
  let showNewFolder = false;
  let newFolderParentId: number | null = null;
  let selectedStoragePath: string | null = null;
  let showSettings = false;
  let settings: AppSettings | null = null;
  let parentFolderName: string | null = null;

  $: treeNodes = buildTree($folders);

  onMount(async () => {
    try {
      folders.set(await getFolders());
      settings = await getSettings();
    } catch (e) {
      console.error('Failed to load folders:', e);
    }
  });

  function buildTree(folderList: Folder[]): TreeNode[] {
    const map = new Map<number, TreeNode>();
    const roots: TreeNode[] = [];

    folderList.forEach(f => {
      map.set(f.id, { ...f, children: [] });
    });

    folderList.forEach(f => {
      const node = map.get(f.id)!;
      if (f.parent_id && map.has(f.parent_id)) {
        map.get(f.parent_id)!.children.push(node);
      } else {
        roots.push(node);
      }
    });

    const sortChildren = (nodes: TreeNode[]) => {
      nodes.sort((a, b) => a.name.localeCompare(b.name, 'zh'));
      nodes.forEach(n => sortChildren(n.children));
    };
    sortChildren(roots);

    return roots;
  }

  function getFolderNameById(id: number): string | null {
    const folder = $folders.find(f => f.id === id);
    return folder?.name ?? null;
  }

  async function selectDirectory() {
    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择存储目录',
    });

    if (selected) {
      selectedStoragePath = selected as string;
    }
  }

  function handleAddClick() {
    if (showNewFolder) {
      handleCreate();
    } else {
      newFolderParentId = null;
      parentFolderName = null;
      showNewFolder = true;
    }
  }

  function handleAddSubfolder(parentId: number) {
    newFolderParentId = parentId;
    parentFolderName = getFolderNameById(parentId);
    showNewFolder = true;
    newFolderName = '';
    selectedStoragePath = null;
  }

  async function handleCreate() {
    if (newFolderName.trim()) {
      try {
        await createFolder(
          newFolderName.trim(),
          newFolderParentId ?? undefined,
          selectedStoragePath ?? undefined
        );
        folders.set(await getFolders());
        resetNewFolder();
      } catch (e) {
        alert('创建失败: ' + e);
      }
    }
  }

  function resetNewFolder() {
    newFolderName = '';
    showNewFolder = false;
    newFolderParentId = null;
    parentFolderName = null;
    selectedStoragePath = null;
  }

  function handleCancel() {
    resetNewFolder();
  }

  async function handleDelete(id: number) {
    const folder = $folders.find(f => f.id === id);
    const folderName = folder?.name || '此文件夹';

    const confirmed = await confirm(`确定要删除文件夹 "${folderName}" 吗？`, {
      title: '确认删除',
      kind: 'warning',
    });

    if (confirmed) {
      try {
        await deleteFolder(id);
        folders.set(await getFolders());
        if ($selectedFolderId === id) {
          selectedFolderId.set(null);
        }
      } catch (e) {
        alert('删除失败: ' + e);
      }
    }
  }

  function selectFolder(id: number | null) {
    selectedFolderId.set(id);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleCreate();
    } else if (e.key === 'Escape') {
      handleCancel();
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
        settings = await getSettings();
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
        settings = await getSettings();
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
        settings = await getSettings();
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
        settings = await getSettings();
        alert('PDF阅读器设置已清除');
      } catch (e) {
        alert('清除失败: ' + e);
      }
    }
  }

  // 计算每个文件夹的PDF数量
  function getPdfCount(folderId: number | null): number {
    if (folderId === null) {
      return $pdfList.length;
    }
    return $pdfList.filter(p => p.folder_id === folderId).length;
  }
</script>

<div class="folder-tree">
  <div class="panel-header">
    <h3>文件夹</h3>
    <div class="header-actions">
      <button class="icon-btn settings-btn" on:click={() => showSettings = !showSettings} title="设置">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="12" cy="12" r="3"/>
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z"/>
        </svg>
      </button>
      <button class="icon-btn add-btn" on:click={handleAddClick} title="新建文件夹">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
      </button>
    </div>
  </div>

  {#if showSettings}
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
    </div>
  {/if}

  {#if showNewFolder}
    <div class="new-folder-panel">
      {#if parentFolderName}
        <div class="parent-hint">在 "{parentFolderName}" 下创建</div>
      {/if}
      <div class="new-folder-input">
        <input
          type="text"
          bind:value={newFolderName}
          placeholder="文件夹名称"
          on:keydown={handleKeydown}
        />
        {#if !newFolderParentId}
          <button class="path-btn" on:click={selectDirectory} title="选择存储目录">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
          </button>
        {/if}
        <button class="cancel-btn" on:click={handleCancel}>取消</button>
        <button class="confirm-btn" on:click={handleCreate}>确定</button>
      </div>
      {#if !newFolderParentId && selectedStoragePath}
        <div class="selected-path">{selectedStoragePath}</div>
      {/if}
    </div>
  {/if}

  <div class="folder-list">
    <div
      class="folder-item"
      class:active={$selectedFolderId === null}
      on:click={() => selectFolder(null)}
    >
      <svg class="folder-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
      </svg>
      <span class="folder-name">全部文件</span>
      <span class="folder-count">{getPdfCount(null)}</span>
    </div>
    {#each treeNodes as node}
      <FolderNodeA {node} level={0} onDelete={handleDelete} onAddSubfolder={handleAddSubfolder} {getPdfCount} />
    {/each}
  </div>
</div>

<style>
  .folder-tree {
    width: 100%;
    height: 100%;
    background: var(--bg-secondary, #ffffff);
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .panel-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px;
    border-bottom: 1px solid var(--border-light, #f3f4f6);
    flex-shrink: 0;
  }

  .panel-header h3 {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-secondary, #6b7280);
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin: 0;
  }

  .header-actions {
    display: flex;
    gap: 4px;
  }

  .icon-btn {
    width: 24px;
    height: 24px;
    border: none;
    background: var(--bg-tertiary, #f5f7f9);
    border-radius: 5px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .icon-btn:hover {
    background: var(--accent-soft, #eff6ff);
    color: var(--accent, #3b82f6);
  }

  .icon-btn svg {
    width: 14px;
    height: 14px;
  }

  .folder-list {
    flex: 1;
    overflow-y: auto;
    padding: 6px;
  }

  .folder-item {
    display: flex;
    align-items: center;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
    margin-bottom: 1px;
  }

  .folder-item:hover {
    background: var(--bg-tertiary, #f5f7f9);
  }

  .folder-item.active {
    background: var(--accent-soft, #eff6ff);
  }

  .folder-item.active .folder-name {
    color: var(--accent, #3b82f6);
    font-weight: 500;
  }

  .folder-item.active .folder-icon {
    color: var(--accent, #3b82f6);
  }

  .folder-icon {
    width: 16px;
    height: 16px;
    margin-right: 8px;
    color: var(--text-muted, #9ca3af);
    flex-shrink: 0;
  }

  .folder-name {
    font-size: 12px;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  .folder-count {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    background: var(--bg-tertiary, #f5f7f9);
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
  }

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

  .new-folder-panel {
    background: var(--bg-tertiary, #f5f7f9);
    border-bottom: 1px solid var(--border, #e5e7eb);
    padding: 12px;
  }

  .parent-hint {
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    margin-bottom: 8px;
  }

  .new-folder-input {
    display: flex;
    gap: 6px;
  }

  .new-folder-input input {
    flex: 1;
    padding: 8px 12px;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 6px;
    font-size: 13px;
    outline: none;
    transition: all 0.15s;
  }

  .new-folder-input input:focus {
    border-color: var(--accent, #3b82f6);
    box-shadow: 0 0 0 3px var(--accent-soft, #eff6ff);
  }

  .path-btn {
    width: 36px;
    height: 36px;
    border: 1px solid var(--border, #e5e7eb);
    background: var(--bg-secondary, #ffffff);
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .path-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .path-btn svg {
    width: 16px;
    height: 16px;
  }

  .cancel-btn, .confirm-btn {
    padding: 8px 14px;
    border: none;
    border-radius: 6px;
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s;
  }

  .cancel-btn {
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    color: var(--text-secondary, #6b7280);
  }

  .cancel-btn:hover {
    border-color: var(--text-muted, #9ca3af);
  }

  .confirm-btn {
    background: var(--accent, #3b82f6);
    color: white;
  }

  .confirm-btn:hover {
    background: #2563eb;
  }

  .selected-path {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    margin-top: 6px;
    word-break: break-all;
  }
</style>