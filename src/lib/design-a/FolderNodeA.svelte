<script lang="ts">
  import { selectedFolderId } from '../../stores';
  import type { Folder } from '../../api';

  interface TreeNode extends Folder {
    children: TreeNode[];
  }

  export let node: TreeNode;
  export let level = 0;
  export let onDelete: (id: number) => void;
  export let onAddSubfolder: (parentId: number) => void;
  export let getPdfCount: (folderId: number | null) => number;

  let isExpanded = false;
  $: hasChildren = node.children && node.children.length > 0;

  function toggleExpand(e: MouseEvent) {
    e.stopPropagation();
    isExpanded = !isExpanded;
  }

  function selectFolder() {
    selectedFolderId.set(node.id);
  }

  function handleAddSubfolder(e: MouseEvent) {
    e.stopPropagation();
    onAddSubfolder(node.id);
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
      <button class="expand-btn" on:click={toggleExpand}>
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
  <span class="folder-count">{getPdfCount(node.id)}</span>

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
</li>

{#if hasChildren && isExpanded}
  {#each node.children as child}
    <svelte:self
      node={child}
      level={level + 1}
      {onDelete}
      {onAddSubfolder}
      {getPdfCount}
    />
  {/each}
{/if}

<style>
  .folder-node {
    display: flex;
    align-items: center;
    padding: 10px 12px;
    border-radius: 8px;
    cursor: pointer;
    transition: all 0.15s ease;
    margin-bottom: 2px;
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
    width: 16px;
    margin-right: 6px;
    flex-shrink: 0;
  }

  .expand-btn {
    width: 16px;
    height: 16px;
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
    width: 12px;
    height: 12px;
    transition: transform 0.15s ease;
  }

  .expand-icon.rotated {
    transform: rotate(90deg);
  }

  .expand-placeholder {
    width: 16px;
  }

  .folder-icon {
    width: 18px;
    height: 18px;
    margin-right: 10px;
    color: var(--text-muted, #9ca3af);
    flex-shrink: 0;
  }

  .folder-name {
    font-size: 13px;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex: 1;
  }

  .folder-count {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    background: var(--bg-tertiary, #f5f7f9);
    padding: 2px 6px;
    border-radius: 4px;
    margin-left: 8px;
    flex-shrink: 0;
  }

  .node-actions {
    display: flex;
    gap: 4px;
    margin-left: 8px;
    opacity: 0;
    transition: opacity 0.15s;
  }

  .action-btn {
    width: 22px;
    height: 22px;
    border: none;
    background: var(--bg-tertiary, #f5f7f9);
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .action-btn svg {
    width: 12px;
    height: 12px;
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
</style>