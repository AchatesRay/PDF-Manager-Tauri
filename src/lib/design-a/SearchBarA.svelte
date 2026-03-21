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
    placeholder="搜索文档内容或文件名..."
    on:keydown={handleKeydown}
  />
  <button class="search-btn" on:click={handleSearch} disabled={$isLoading}>
    {#if $isLoading}
      <svg class="spinner" viewBox="0 0 24 24">
        <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none" stroke-dasharray="31.4 31.4"/>
      </svg>
    {:else}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8"/>
        <line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
    {/if}
  </button>
  {#if $showSearchResults}
    <button class="clear-btn" on:click={clearSearch}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <line x1="18" y1="6" x2="6" y2="18"/>
        <line x1="6" y1="6" x2="18" y2="18"/>
      </svg>
    </button>
  {/if}
</div>

<style>
  .search-bar {
    display: flex;
    gap: 8px;
    padding: 10px;
    background: var(--bg-secondary, #ffffff);
    border-bottom: 1px solid var(--border, #e5e7eb);
    flex-shrink: 0;
  }

  .mode-select {
    padding: 6px 10px;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 6px;
    background: var(--bg-secondary, #ffffff);
    font-size: 12px;
    color: var(--text-primary, #1f2937);
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s;
  }

  .mode-select:focus {
    border-color: var(--accent, #3b82f6);
  }

  input {
    flex: 1;
    padding: 6px 10px;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 6px;
    font-size: 12px;
    outline: none;
    transition: all 0.15s;
    color: var(--text-primary, #1f2937);
  }

  input::placeholder {
    color: var(--text-muted, #9ca3af);
  }

  input:focus {
    border-color: var(--accent, #3b82f6);
    box-shadow: 0 0 0 2px var(--accent-soft, #eff6ff);
  }

  .search-btn, .clear-btn {
    width: 32px;
    height: 32px;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .search-btn {
    background: var(--accent, #3b82f6);
    color: white;
  }

  .search-btn:hover:not(:disabled) {
    background: #2563eb;
  }

  .search-btn:disabled {
    background: var(--text-muted, #9ca3af);
    cursor: not-allowed;
  }

  .search-btn svg, .clear-btn svg {
    width: 16px;
    height: 16px;
  }

  .clear-btn {
    background: var(--bg-tertiary, #f5f7f9);
    color: var(--text-secondary, #6b7280);
  }

  .clear-btn:hover {
    background: var(--border, #e5e7eb);
  }

  .spinner {
    width: 16px;
    height: 16px;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>