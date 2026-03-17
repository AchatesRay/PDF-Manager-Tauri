<script lang="ts">
  import { selectedFolderId } from '../stores';
  import type { Folder } from '../api';

  export let folder: Folder & { children?: (Folder & { children?: Folder[] })[] };
  export let level = 0;
  export let onDelete: (id: number) => void;

  let isExpanded = false;
  let hasChildren = folder.children && folder.children.length > 0;

  function toggleExpand(e: MouseEvent) {
    e.stopPropagation();
    isExpanded = !isExpanded;
  }

  function selectFolder() {
    selectedFolderId.set(folder.id);
  }
</script>

<li
  class:active={$selectedFolderId === folder.id}
  style="padding-left: {12 + level * 16}px"
  on:click={selectFolder}
>
  {#if hasChildren}
    <button class="expand-btn" on:click={toggleExpand}>
      {isExpanded ? '▼' : '▶'}
    </button>
  {:else}
    <span class="expand-placeholder"></span>
  {/if}
  <span class="folder-icon">📁</span>
  <span class="name">{folder.name}</span>
  <button class="delete-btn" on:click|stopPropagation={() => onDelete(folder.id)}>×</button>
</li>

{#if hasChildren && isExpanded}
  {#each folder.children || [] as child}
    <svelte:self
      folder={child}
      level={level + 1}
      {onDelete}
    />
  {/each}
{/if}

<style>
  li {
    display: flex;
    align-items: center;
    padding: 8px 12px;
    cursor: pointer;
    border-radius: 4px;
    margin-bottom: 2px;
    gap: 4px;
  }

  li:hover {
    background: #e0e0e0;
  }

  li.active {
    background: #bbdefb;
  }

  .expand-btn {
    width: 16px;
    height: 16px;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 10px;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .expand-placeholder {
    width: 16px;
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

  .delete-btn {
    opacity: 0;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 16px;
    color: #f44336;
  }

  li:hover .delete-btn {
    opacity: 1;
  }
</style>