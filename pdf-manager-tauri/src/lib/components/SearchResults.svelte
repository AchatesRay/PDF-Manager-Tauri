<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults } from '../stores';
  import type { SearchResult } from '../api';

  function handleClick(result: SearchResult) {
    selectedPdfId.set(result.pdf_id);
    showSearchResults.set(false);
  }
</script>

{#if $showSearchResults && $searchResults.length > 0}
  <div class="search-results">
    <h4>搜索结果 ({$searchResults.length})</h4>
    <ul>
      {#each $searchResults as result}
        <li on:click={() => handleClick(result)}>
          <div class="filename">{result.filename}</div>
          <div class="page">第 {result.page_number} 页</div>
          <div class="snippet">{result.snippet}</div>
        </li>
      {/each}
    </ul>
  </div>
{/if}

{#if $showSearchResults && $searchResults.length === 0}
  <div class="no-results">
    未找到匹配结果
  </div>
{/if}

<style>
  .search-results {
    padding: 10px;
    background: #fff;
    border-top: 1px solid #ddd;
    max-height: 300px;
    overflow-y: auto;
  }

  h4 {
    margin: 0 0 10px 0;
    font-size: 14px;
    color: #333;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  li {
    padding: 10px;
    cursor: pointer;
    border-radius: 4px;
    margin-bottom: 5px;
    background: #f9f9f9;
  }

  li:hover {
    background: #f0f0f0;
  }

  .filename {
    font-weight: 500;
    margin-bottom: 4px;
  }

  .page {
    font-size: 12px;
    color: #2196f3;
    margin-bottom: 4px;
  }

  .snippet {
    font-size: 13px;
    color: #666;
    line-height: 1.4;
  }

  .no-results {
    padding: 20px;
    text-align: center;
    color: #666;
    background: #fff;
    border-top: 1px solid #ddd;
  }
</style>