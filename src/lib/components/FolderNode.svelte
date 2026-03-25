<script lang="ts">
  import { selectedFolderId, pdfList, folders } from '../stores';
  import type { Folder } from '../api';

  interface TreeNode extends Folder {
    children: TreeNode[];
  }

  export let node: TreeNode;
  export let level = 0;
  export let onDelete: (id: number) => void;
  export let onAddSubfolder: (parentId: number) => void;
  export let getPdfCount: (folderId: number | null) => number;
  export let getFolderOcrStats: (folderId: number | null) => { ocrCount: number; totalCount: number };
  export let newFolderParentId: number | null = null;
  export let newFolderName: string = '';
  export let selectedStoragePath: string | null = null;
  export let onCreateFolder: () => void;
  export let onCancelFolder: () => void;
  export let onSelectDirectory: () => void;
  export let onNewFolderNameChange: (value: string) => void;
  export let isFolderExpanded: (folderId: number) => boolean;
  export let toggleFolderExpand: (folderId: number) => void;

  $: hasChildren = node.children && node.children.length > 0;
  $: showNewFolderHere = newFolderParentId === node.id;
  $: isExpanded = isFolderExpanded(node.id);
  // 显式依赖 $pdfList 和 $folders 确保响应式更新
  $: stats = ($pdfList, $folders, getFolderOcrStats(node.id));

  function handleToggleExpand(e: MouseEvent) {
    e.stopPropagation();
    alert('[调试] 点击了展开按钮，文件夹ID: ' + node.id + ', 名称: ' + node.name);
    toggleFolderExpand(node.id);
    alert('[调试] 当前展开状态: ' + isFolderExpanded(node.id));
  }

  function selectFolder() {
    selectedFolderId.set(node.id);
  }

  function handleAddSubfolder(e: MouseEvent) {
    e.stopPropagation();
    onAddSubfolder(node.id);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      onCreateFolder();
    } else if (e.key === 'Escape') {
      onCancelFolder();
    }
  }
</script>

<li
  class="folder-node"
  class:active={$selectedFolderId === node.id}
  style="padding-left: {12 + level * 16}px"
  on:click={selectFolder}
>
  <div class="expand-area">
    {#if hasChildren}
      <button class="expand-btn" on:click={handleToggleExpand}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" class="expand-icon" class:rotated={isExpanded}>
          <polyline points="9 18 15 12 9 6"/>
        </svg>
      </button>
    {:else}
      <span class="expand-placeholder"></span>
    {/if}
  </div>

  <svg class="folder-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
  </svg>

  <span class="folder-name">{node.name}</span>

  <div class="right-area">
    <div class="node-actions">
      <button class="action-btn add" on:click={handleAddSubfolder} title="添加子文件夹">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
      </button>
      <button class="action-btn delete" on:click|stopPropagation={() => onDelete(node.id)} title="删除">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="3 6 5 6 21 6"/>
          <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
        </svg>
      </button>
    </div>
    <span class="folder-count" class:completed={stats.ocrCount === stats.totalCount && stats.totalCount > 0}>
      {stats.ocrCount}/{stats.totalCount}
    </span>
  </div>
</li>

{#if hasChildren && isExpanded}
  {#each node.children as child}
    <svelte:self
      node={child}
      level={level + 1}
      {onDelete}
      {onAddSubfolder}
      {getPdfCount}
      {getFolderOcrStats}
      {newFolderParentId}
      {newFolderName}
      {selectedStoragePath}
      {onCreateFolder}
      {onCancelFolder}
      {onSelectDirectory}
      {onNewFolderNameChange}
      {isFolderExpanded}
      {toggleFolderExpand}
    />
  {/each}
{/if}

{#if showNewFolderHere}
  <li class="new-folder-item" style="padding-left: {12 + (level + 1) * 16}px">
    <div class="new-folder-inline">
      <input
        type="text"
        placeholder="子文件夹名称"
        value={newFolderName}
        on:input={(e) => onNewFolderNameChange(e.currentTarget.value)}
        on:keydown={handleKeydown}
      />
      <button class="inline-btn cancel" on:click={onCancelFolder} title="取消">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18"/>
          <line x1="6" y1="6" x2="18" y2="18"/>
        </svg>
      </button>
      <button class="inline-btn confirm" on:click={onCreateFolder} title="确定">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="20 6 9 17 4 12"/>
        </svg>
      </button>
    </div>
  </li>
{/if}

<style>
  .folder-node {
    display: flex;
    align-items: center;
    padding: 6px 8px;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
    margin-bottom: 1px;
  }

  .folder-node:hover {
    background: var(--bg-tertiary, #f5f7f9);
  }

  .folder-node:hover .node-actions {
    opacity: 1;
  }

  .folder-node.active {
    background: var(--accent-soft, #eff6ff);
  }

  .folder-node.active .folder-name {
    color: var(--accent, #3b82f6);
    font-weight: 500;
  }

  .folder-node.active .folder-icon {
    color: var(--accent, #3b82f6);
  }

  .expand-area {
    width: 14px;
    margin-right: 4px;
    flex-shrink: 0;
  }

  .expand-btn {
    width: 14px;
    height: 14px;
    border: none;
    background: none;
    cursor: pointer;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #9ca3af);
  }

  .expand-icon {
    width: 10px;
    height: 10px;
    transition: transform 0.15s ease;
  }

  .expand-icon.rotated {
    transform: rotate(90deg);
  }

  .expand-placeholder {
    width: 14px;
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

  .right-area {
    display: flex;
    align-items: center;
    margin-left: auto;
    flex-shrink: 0;
    gap: 4px;
  }

  .folder-count {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    background: var(--bg-tertiary, #f5f7f9);
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
    min-width: 32px;
    text-align: right;
  }

  .folder-count.completed {
    background: #dcfce7;
    color: #16a34a;
  }

  .node-actions {
    display: flex;
    gap: 3px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .action-btn {
    width: 18px;
    height: 18px;
    border: none;
    background: var(--bg-tertiary, #f5f7f9);
    border-radius: 3px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .action-btn svg {
    width: 10px;
    height: 10px;
  }

  .action-btn.add {
    color: var(--success, #10b981);
  }

  .action-btn.add:hover {
    background: var(--success, #10b981);
    color: white;
  }

  .action-btn.delete {
    color: var(--error, #ef4444);
  }

  .action-btn.delete:hover {
    background: var(--error, #ef4444);
    color: white;
  }

  .new-folder-item {
    display: flex;
    align-items: center;
    padding: 2px 8px;
    margin-bottom: 1px;
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
</style>