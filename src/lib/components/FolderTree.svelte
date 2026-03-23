<script lang="ts">
  import { open, confirm } from '@tauri-apps/plugin-dialog';
  import { folders, selectedFolderId, isLoading, pdfList } from '../stores';
  import { getFolders, createFolder, deleteFolder, getSettings, setDataDir, resetDataDir, setPdfReader } from '../api';
  import { onMount, tick } from 'svelte';
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
  let newFolderInput: HTMLInputElement;

  $: treeNodes = buildTree($folders);

  onMount(async () => {
    try {
      // 只加载 folders，pdfList 由 PdfList 负责加载
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
    newFolderParentId = null;
    newFolderName = '';
    selectedStoragePath = null;
    showNewFolder = true;
    tick().then(() => {
      newFolderInput?.focus();
    });
  }

  function handleAddSubfolder(parentId: number) {
    newFolderParentId = parentId;
    newFolderName = '';
    selectedStoragePath = null;
    showNewFolder = true;
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
    selectedStoragePath = null;
  }

  function handleCancel() {
    resetNewFolder();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleCreate();
    } else if (e.key === 'Escape') {
      handleCancel();
    }
  }

  // 递归获取文件夹及其所有子文件夹的 ID
  function getAllFolderIds(folderId: number | null): number[] {
    if (folderId === null) {
      return $folders.map(f => f.id);
    }

    const ids = [folderId];
    const children = $folders.filter(f => f.parent_id === folderId);
    for (const child of children) {
      ids.push(...getAllFolderIds(child.id));
    }
    return ids;
  }

  // 获取文件夹的文件数量（包括子文件夹）
  function getPdfCount(folderId: number | null): number {
    const ids = getAllFolderIds(folderId);
    return $pdfList.filter(p => ids.includes(p.folder_id ?? 0)).length;
  }

  // 获取文件夹的OCR统计（包括子文件夹）
  function getFolderOcrStats(folderId: number | null): { ocrCount: number; totalCount: number } {
    const ids = getAllFolderIds(folderId);
    const pdfs = $pdfList.filter(p => ids.includes(p.folder_id ?? 0));
    return {
      ocrCount: pdfs.filter(p => p.status === 'done').length,
      totalCount: pdfs.length
    };
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

  $: allStats = getFolderOcrStats(null);

  $: showNewFolderAtRoot = showNewFolder && newFolderParentId === null;
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

  <ul class="folder-list">
    <li
      class="folder-item"
      class:active={$selectedFolderId === null}
      on:click={() => selectFolder(null)}
    >
      <svg class="folder-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
      </svg>
      <span class="folder-name">全部文件</span>
      <span class="folder-count" class:completed={allStats.ocrCount === allStats.totalCount && allStats.totalCount > 0}>
        {allStats.ocrCount}/{allStats.totalCount}
      </span>
    </li>

    {#each treeNodes as node}
      <FolderNode
        {node}
        level={0}
        onDelete={handleDelete}
        onAddSubfolder={handleAddSubfolder}
        {getPdfCount}
        {getFolderOcrStats}
        newFolderParentId={newFolderParentId}
        newFolderName={newFolderName}
        {selectedStoragePath}
        onCreateFolder={handleCreate}
        onCancelFolder={handleCancel}
        onSelectDirectory={selectDirectory}
        onNewFolderNameChange={(v) => newFolderName = v}
      />
    {/each}

    <!-- 根目录新建文件夹输入框 -->
    {#if showNewFolderAtRoot}
      <li class="new-folder-item">
        <div class="new-folder-inline">
          <input
            type="text"
            bind:this={newFolderInput}
            placeholder="文件夹名称"
            bind:value={newFolderName}
            on:keydown={handleKeydown}
          />
          <button class="path-btn" on:click={selectDirectory} title="选择存储目录">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
            </svg>
          </button>
          <button class="inline-btn cancel" on:click={handleCancel} title="取消">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18"/>
              <line x1="6" y1="6" x2="18" y2="18"/>
            </svg>
          </button>
          <button class="inline-btn confirm" on:click={handleCreate} title="确定">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="20 6 9 17 4 12"/>
            </svg>
          </button>
        </div>
      </li>
      {#if selectedStoragePath}
        <li class="selected-path-item">
          <span class="selected-path">{selectedStoragePath}</span>
        </li>
      {/if}
    {/if}
  </ul>
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
    list-style: none;
    margin: 0;
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
    min-width: 0;
  }

  .folder-count {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    background: var(--bg-tertiary, #f5f7f9);
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
    margin-left: auto;
    font-variant-numeric: tabular-nums;
    min-width: 32px;
    text-align: center;
  }

  .folder-count.completed {
    background: #dcfce7;
    color: #16a34a;
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

  .new-folder-item {
    display: flex;
    align-items: center;
    padding: 2px 8px;
    margin-bottom: 1px;
    list-style: none;
  }

  .new-folder-inline {
    display: flex;
    align-items: center;
    gap: 3px;
    flex: 1;
    background: var(--bg-tertiary, #f5f7f9);
    padding: 3px 6px;
    border-radius: 4px;
    border: 1px dashed var(--accent, #3b82f6);
    min-width: 0;
  }

  .new-folder-inline input {
    flex: 1;
    border: none;
    background: transparent;
    font-size: 11px;
    outline: none;
    min-width: 60px;
  }

  .path-btn {
    width: 18px;
    height: 18px;
    border: 1px solid var(--border, #e5e7eb);
    background: var(--bg-secondary, #ffffff);
    border-radius: 3px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.15s;
  }

  .path-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .path-btn svg {
    width: 10px;
    height: 10px;
  }

  .inline-btn {
    width: 18px;
    height: 18px;
    border: none;
    border-radius: 3px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: all 0.15s;
  }

  .inline-btn svg {
    width: 10px;
    height: 10px;
  }

  .inline-btn.cancel {
    background: transparent;
    color: var(--text-muted, #9ca3af);
  }

  .inline-btn.cancel:hover {
    color: var(--error, #ef4444);
  }

  .inline-btn.confirm {
    background: var(--success, #10b981);
    color: white;
  }

  .inline-btn.confirm:hover {
    background: #059669;
  }

  .selected-path-item {
    list-style: none;
    padding: 0 8px 4px;
  }

  .selected-path {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    word-break: break-all;
  }
</style>