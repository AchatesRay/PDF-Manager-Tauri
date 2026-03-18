<script lang="ts">
  import { open, confirm } from '@tauri-apps/plugin-dialog';
  import { folders, selectedFolderId, isLoading } from '../stores';
  import { getFolders, createFolder, deleteFolder, getSettings, setDataDir, resetDataDir, setPdfReader, openPdfExternally } from '../api';
  import { onMount } from 'svelte';
  import FolderNode from './FolderNode.svelte';
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

    // 初始化所有节点
    folderList.forEach(f => {
      map.set(f.id, { ...f, children: [] });
    });

    // 构建树形结构
    folderList.forEach(f => {
      const node = map.get(f.id)!;
      if (f.parent_id && map.has(f.parent_id)) {
        map.get(f.parent_id)!.children.push(node);
      } else {
        roots.push(node);
      }
    });

    // 按名称排序
    const sortChildren = (nodes: TreeNode[]) => {
      nodes.sort((a, b) => a.name.localeCompare(b.name, 'zh'));
      nodes.forEach(n => sortChildren(n.children));
    };
    sortChildren(roots);

    return roots;
  }

  // 根据ID获取文件夹名称
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

  // 创建根文件夹
  function handleAddClick() {
    if (showNewFolder) {
      handleCreate();
    } else {
      newFolderParentId = null;
      parentFolderName = null;
      showNewFolder = true;
    }
  }

  // 创建子文件夹
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
</script>

<div class="folder-tree">
  <div class="header">
    <h3>文件夹</h3>
    <div class="header-btns">
      <button class="settings-btn" on:click={() => showSettings = !showSettings} title="设置">⚙️</button>
      <button on:click={handleAddClick}>+</button>
    </div>
  </div>

  {#if showSettings}
    <div class="settings-panel">
      <div class="settings-title">存储设置</div>
      <div class="settings-item">
        <label>数据目录:</label>
        <div class="settings-path">{settings?.data_dir || '加载中...'}</div>
        <div class="settings-actions">
          <button on:click={selectDataDir}>选择目录</button>
          <button class="reset-btn" on:click={handleResetDataDir}>重置</button>
        </div>
      </div>
      <div class="settings-item">
        <label>日志目录:</label>
        <div class="settings-path">{settings?.log_dir || '加载中...'}</div>
      </div>
      <div class="settings-title" style="margin-top: 12px;">PDF阅读器</div>
      <div class="settings-item">
        <label>外部阅读器:</label>
        <div class="settings-path">{settings?.pdf_reader_path || '使用系统默认'}</div>
        <div class="settings-actions">
          <button on:click={selectPdfReader}>选择阅读器</button>
          {#if settings?.pdf_reader_path}
            <button class="reset-btn" on:click={clearPdfReader}>清除</button>
          {/if}
        </div>
      </div>
    </div>
  {/if}

  {#if showNewFolder}
    <div class="new-folder">
      {#if parentFolderName}
        <div class="parent-hint">在 "{parentFolderName}" 下创建:</div>
      {/if}
      <input
        type="text"
        bind:value={newFolderName}
        placeholder="文件夹名称"
        on:keydown={handleKeydown}
      />
      {#if !newFolderParentId}
        <button class="path-btn" on:click={selectDirectory} title="选择存储目录">
          📁
        </button>
      {/if}
      <button class="cancel-btn" on:click={handleCancel}>取消</button>
      <button on:click={handleCreate}>确定</button>
    </div>
    {#if !newFolderParentId && selectedStoragePath}
      <div class="selected-path">{selectedStoragePath}</div>
    {/if}
  {/if}

  <ul class="folder-list">
    <li
      class:active={$selectedFolderId === null}
      on:click={() => selectFolder(null)}
    >
      <span class="folder-icon">📚</span>
      <span class="name">全部文件</span>
    </li>
    {#each treeNodes as node}
      <FolderNode {node} level={0} onDelete={handleDelete} onAddSubfolder={handleAddSubfolder} />
    {/each}
  </ul>
</div>

<style>
  .folder-tree {
    width: 100%;
    height: 100%;
    padding: 10px;
    overflow-y: auto;
    background: #fafafa;
    display: flex;
    flex-direction: column;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
    flex-shrink: 0;
  }

  .header h3 {
    font-size: 14px;
    color: #333;
    margin: 0;
  }

  .header button {
    width: 24px;
    height: 24px;
    border: none;
    background: #2196f3;
    color: white;
    border-radius: 4px;
    cursor: pointer;
  }

  .folder-list {
    list-style: none;
    padding: 0;
    margin: 0;
    flex: 1;
    overflow-y: auto;
  }

  .folder-list > li {
    padding: 8px 12px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    align-items: center;
    gap: 4px;
    margin-bottom: 2px;
  }

  .folder-list > li:hover {
    background: #e0e0e0;
  }

  .folder-list > li.active {
    background: #bbdefb;
  }

  .folder-icon {
    font-size: 14px;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .new-folder {
    display: flex;
    gap: 5px;
    margin-bottom: 10px;
    flex-shrink: 0;
    flex-wrap: wrap;
  }

  .parent-hint {
    width: 100%;
    font-size: 11px;
    color: #666;
    margin-bottom: 4px;
  }

  .new-folder input {
    flex: 1;
    padding: 5px;
    border: 1px solid #ddd;
    border-radius: 4px;
  }

  .new-folder button {
    padding: 5px 10px;
    background: #4caf50;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  .new-folder .cancel-btn {
    background: #9e9e9e;
  }

  .path-btn {
    padding: 5px 8px;
    background: #fff;
    border: 1px solid #ddd;
    cursor: pointer;
  }

  .selected-path {
    font-size: 11px;
    color: #666;
    margin-top: 4px;
    margin-bottom: 10px;
    word-break: break-all;
    padding: 0 5px;
  }

  .header-btns {
    display: flex;
    gap: 5px;
  }

  .settings-btn {
    width: 24px;
    height: 24px;
    border: none;
    background: #9e9e9e;
    color: white;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }

  .settings-panel {
    background: #fff;
    border: 1px solid #ddd;
    border-radius: 4px;
    padding: 10px;
    margin-bottom: 10px;
    font-size: 12px;
  }

  .settings-title {
    font-weight: bold;
    margin-bottom: 8px;
    color: #333;
  }

  .settings-item {
    margin-bottom: 8px;
  }

  .settings-item label {
    display: block;
    color: #666;
    margin-bottom: 4px;
  }

  .settings-path {
    background: #f5f5f5;
    padding: 4px 8px;
    border-radius: 4px;
    word-break: break-all;
    margin-bottom: 4px;
  }

  .settings-actions {
    display: flex;
    gap: 5px;
  }

  .settings-actions button {
    padding: 4px 8px;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    background: #2196f3;
    color: white;
    font-size: 11px;
  }

  .settings-actions .reset-btn {
    background: #9e9e9e;
  }
</style>