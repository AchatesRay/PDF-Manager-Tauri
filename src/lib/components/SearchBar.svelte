<script lang="ts">
  import { searchQuery, searchResults, isLoading, showSearchResults, searchMode, filenameSearchResults } from '../stores';
  import { search, searchFilename } from '../api';

  async function handleSearch() {
    if ($searchQuery.trim()) {
      isLoading.set(true);
      try {
        if ($searchMode === 'content') {
          searchResults.set(await search($searchQuery.trim()));
          filenameSearchResults.set([]);
        } else {
          filenameSearchResults.set(await searchFilename($searchQuery.trim()));
          searchResults.set([]);
        }
        showSearchResults.set(true);
      } catch (e) {
        console.error('Search failed:', e);
        alert('搜索失败: ' + e);
      } finally {
        isLoading.set(false);
      }
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') {
      handleSearch();
    }
  }

  function clearSearch() {
    searchQuery.set('');
    searchResults.set([]);
    filenameSearchResults.set([]);
    showSearchResults.set(false);
  }
</script>

<div class="search-bar">
  <select bind:value={$searchMode} class="mode-select">
    <option value="content">内容</option>
    <option value="filename">文件名</option>
  </select>
  <input
    type="text"
    bind:value={$searchQuery}
    placeholder="搜索..."
    on:keydown={handleKeydown}
  />
  <button on:click={handleSearch} disabled={$isLoading}>
    {$isLoading ? '搜索中...' : '搜索'}
  </button>
  {#if $showSearchResults}
    <button class="clear-btn" on:click={clearSearch}>清除</button>
  {/if}
</div>

<style>
  .search-bar {
    display: flex;
    gap: 8px;
    padding: 10px;
    background: #f5f5f5;
    border-bottom: 1px solid #ddd;
    flex-shrink: 0;
  }

  .mode-select {
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 4px;
    background: #fff;
    cursor: pointer;
  }

  input {
    flex: 1;
    padding: 10px;
    border: 1px solid #ddd;
    border-radius: 4px;
    font-size: 14px;
  }

  input:focus {
    outline: none;
    border-color: #2196f3;
  }

  button {
    padding: 10px 20px;
    background: #2196f3;
    color: white;
    border: none;
    border-radius: 4px;
    cursor: pointer;
  }

  button:disabled {
    background: #ccc;
    cursor: not-allowed;
  }

  .clear-btn {
    background: #757575;
  }
</style>