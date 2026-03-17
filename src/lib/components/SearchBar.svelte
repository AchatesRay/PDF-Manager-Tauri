<script lang="ts">
  import { searchQuery, searchResults, isLoading, showSearchResults } from '../stores';
  import { search } from '../api';

  async function handleSearch() {
    if ($searchQuery.trim()) {
      isLoading.set(true);
      try {
        searchResults.set(await search($searchQuery.trim()));
        showSearchResults.set(true);
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
    showSearchResults.set(false);
  }
</script>

<div class="search-bar">
  <input
    type="text"
    bind:value={$searchQuery}
    placeholder="搜索 PDF 内容..."
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
    gap: 10px;
    padding: 10px;
    background: #f5f5f5;
    border-bottom: 1px solid #ddd;
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
  }

  .clear-btn {
    background: #757575;
  }
</style>