<script lang="ts">
  import { folders, selectedFolderId, isLoading } from '../stores';
  import { getFolders, createFolder, deleteFolder } from '../api';
  import { onMount } from 'svelte';

  let newFolderName = '';
  let showNewFolder = false;

  onMount(async () => {
    try {
      folders.set(await getFolders());
    } catch (e) {
      console.error('Failed to load folders:', e);
    }
  });

  async function handleCreate() {
    if (newFolderName.trim()) {
      try {
        await createFolder(newFolderName.trim());
        folders.set(await getFolders());
        newFolderName = '';
        showNewFolder = false;
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

  $: $folders, $selectedFolderId, $isLoading;
</script>

<div class="folder-tree">
  <div class="header">
    <h3>文件夹</h3>
    <button on:click={() => showNewFolder = !showNewFolder}>+</button>
  </div>

  {#if showNewFolder}
    <div class="new-folder">
      <input type="text" bind:value={newFolderName} placeholder="文件夹名称" />
      <button on:click={handleCreate}>确定</button>
    </div>
  {/if}

  <ul class="folder-list">
    <li class:active={$selectedFolderId === null} on:click={() => selectFolder(null)}>
      全部文件
    </li>
    {#each $folders as folder}
      <li class:active={$selectedFolderId === folder.id} on:click={() => selectFolder(folder.id)}>
        <span>{folder.name}</span>
        <button class="delete-btn" on:click|stopPropagation={() => handleDelete(folder.id)}>×</button>
      </li>
    {/each}
  </ul>
</div>

<style>
  .folder-tree {
    width: 200px;
    border-right: 1px solid #ddd;
    padding: 10px;
    height: 100%;
    overflow-y: auto;
    background: #fafafa;
  }

  .header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  .header h3 {
    font-size: 14px;
    color: #333;
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
  }

  .folder-list li {
    padding: 8px 12px;
    cursor: pointer;
    border-radius: 4px;
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 2px;
  }

  .folder-list li:hover {
    background: #e0e0e0;
  }

  .folder-list li.active {
    background: #bbdefb;
  }

  .delete-btn {
    opacity: 0;
    border: none;
    background: none;
    cursor: pointer;
    font-size: 16px;
    color: #f44336;
  }

  .folder-list li:hover .delete-btn {
    opacity: 1;
  }

  .new-folder {
    display: flex;
    gap: 5px;
    margin-bottom: 10px;
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