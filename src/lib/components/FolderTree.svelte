<script lang="ts">
  import { folders, selectedFolderId, isLoading } from '../stores';
  import { getFolders, createFolder, deleteFolder } from '../api';
  import { onMount } from 'svelte';
  import FolderNode from './FolderNode.svelte';
  import type { Folder } from '../api';

  interface TreeNode extends Folder {
    children: TreeNode[];
  }

  let newFolderName = '';
  let showNewFolder = false;
  let newFolderParentId: number | null = null;

  $: treeNodes = buildTree($folders);

  onMount(async () => {
    try {
      folders.set(await getFolders());
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

  async function handleCreate() {
    if (newFolderName.trim()) {
      try {
        await createFolder(newFolderName.trim(), newFolderParentId ?? undefined);
        folders.set(await getFolders());
        newFolderName = '';
        showNewFolder = false;
        newFolderParentId = null;
      } catch (e) {
        alert('创建失败: ' + e);
      }
    }
  }

  async function handleDelete(id: number) {
    if (confirm('确定删除此文件夹？')) {
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
    }
  }
</script>

<div class="folder-tree">
  <div class="header">
    <h3>文件夹</h3>
    <button on:click={() => showNewFolder = !showNewFolder}>+</button>
  </div>

  {#if showNewFolder}
    <div class="new-folder">
      <input
        type="text"
        bind:value={newFolderName}
        placeholder="文件夹名称"
        on:keydown={handleKeydown}
      />
      <button on:click={handleCreate}>确定</button>
    </div>
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
      <FolderNode {node} level={0} onDelete={handleDelete} />
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
</style>