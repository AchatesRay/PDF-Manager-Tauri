<script lang="ts">
  import { selectedFolderId, pdfList } from '../stores';
  import type { Folder } from '../api';

  export let node: Folder & { children: (Folder & { children: Folder[] })[] };
  export let level = 0;
  export let onDelete: (id: number) => void;
  export let onAddSubfolder: (parentId: number) => void;

  let isExpanded = false;
  $: hasChildren = node.children && node.children.length > 0;

  // 递归收集所有子文件夹ID（包括当前文件夹）
  function getAllFolderIds(folderNode: typeof node): number[] {
    const ids = [folderNode.id];
    if (folderNode.children) {
      for (const child of folderNode.children) {
        ids.push(...getAllFolderIds(child));
      }
    }
    return ids;
  }

  // 计算文件夹及其子文件夹的文件总数
  $: fileCount = (() => {
    const folderIds = getAllFolderIds(node);
    return $pdfList.filter(pdf => folderIds.includes(pdf.folder_id ?? 0)).length;
  })();

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
  class:active={$selectedFolderId === node.id}
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
  <span class="name">{node.name}</span>
  <span class="file-count">({fileCount})</span>
  <button class="add-btn" on:click={handleAddSubfolder} title="添加子文件夹">+</button>
  <button class="delete-btn" on:click|stopPropagation={() => onDelete(node.id)} title="删除">×</button>
</li>

{#if hasChildren && isExpanded}
  {#each node.children as child}
    <svelte:self
      node={child}
      level={level + 1}
      {onDelete}
      {onAddSubfolder}
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

  .file-count {
    font-size: 11px;
    color: #888;
    flex-shrink: 0;
  }

  .add-btn {
    opacity: 0;
    width: 18px;
    height: 18px;
    border: none;
    background: #4caf50;
    color: white;
    border-radius: 50%;
    cursor: pointer;
    font-size: 12px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .delete-btn {
    opacity: 0;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 16px;
    color: #f44336;
  }

  li:hover .add-btn,
  li:hover .delete-btn {
    opacity: 1;
  }
</style>