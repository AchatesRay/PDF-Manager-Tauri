<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  let currentResultIndex = 0;

  // 计算匹配关键字总数
  $: totalMatchCount = $searchResults.reduce((sum, r) => sum + (r.match_count || 0), 0);

  // 当搜索结果变化时重置索引
  $: if ($searchResults.length > 0) {
    currentResultIndex = Math.min(currentResultIndex, $searchResults.length - 1);
  }

  async function handleContentResultClick(result: SearchResult, index: number) {
    currentResultIndex = index;
    try {
      const detail = await getPdfDetail(result.pdf_id);
      selectedPdfId.set(result.pdf_id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
      jumpToPage.set(result.page_number);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
      alert('该PDF文件可能已被删除，请重新搜索');
    }
  }

  async function navigateResult(direction: 'prev' | 'next') {
    const results = $searchResults;
    if (results.length === 0) return;

    if (direction === 'prev') {
      currentResultIndex = currentResultIndex > 0 ? currentResultIndex - 1 : results.length - 1;
    } else {
      currentResultIndex = currentResultIndex < results.length - 1 ? currentResultIndex + 1 : 0;
    }

    const result = results[currentResultIndex];
    await handleContentResultClick(result, currentResultIndex);
  }

  async function handleFilenameResultClick(pdf: PdfInfo) {
    try {
      const detail = await getPdfDetail(pdf.id);
      selectedPdfId.set(pdf.id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
      jumpToPage.set(null);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
      alert('该PDF文件可能已被删除，请重新搜索');
    }
  }

  // 解析高亮的 snippet
  function renderSnippet(snippet: string): string {
    // 将 **text** 转换为 <mark>text</mark>
    return snippet.replace(/\*\*(.+?)\*\*/g, '<mark>$1</mark>');
  }
</script>

{#if $showSearchResults}
  {#if $searchMode === 'content'}
    {#if $searchResults.length > 0}
      <div class="search-results">
        <div class="header-row">
          <h4>内容搜索结果 ({$searchResults.length}条, {totalMatchCount}处匹配)</h4>
          <div class="nav-buttons">
            <button on:click={() => navigateResult('prev')} title="上一个">↑ 上一个</button>
            <span class="index-info">{currentResultIndex + 1}/{$searchResults.length}</span>
            <button on:click={() => navigateResult('next')} title="下一个">下一个 ↓</button>
          </div>
        </div>
        <ul>
          {#each $searchResults as result, index}
            <li
              class:active={index === currentResultIndex}
              on:click={() => handleContentResultClick(result, index)}
            >
              <span class="filename">{result.filename}</span>
              <span class="page">P{result.page_number}</span>
              <span class="match-count">{result.match_count}处</span>
              <span class="snippet">
                {@html renderSnippet(result.snippet)}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-results">
        未找到匹配的内容
      </div>
    {/if}
  {:else}
    {#if $filenameSearchResults.length > 0}
      <div class="search-results">
        <h4>文件名搜索结果 ({$filenameSearchResults.length})</h4>
        <ul>
          {#each $filenameSearchResults as pdf}
            <li on:click={() => handleFilenameResultClick(pdf)}>
              <span class="filename">{pdf.filename}</span>
              <span class="meta">{pdf.page_count} 页</span>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-results">
        未找到匹配的文件名
      </div>
    {/if}
  {/if}
{/if}

<style>
  .search-results {
    padding: 10px;
    background: #fff;
    border-top: 1px solid #ddd;
    max-height: 300px;
    overflow-y: auto;
    flex-shrink: 0;
  }

  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 10px;
  }

  h4 {
    margin: 0;
    font-size: 14px;
    color: #333;
  }

  .nav-buttons {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .nav-buttons button {
    padding: 4px 10px;
    background: #e0e0e0;
    border: none;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }

  .nav-buttons button:hover {
    background: #bddef7;
  }

  .index-info {
    font-size: 12px;
    color: #666;
    min-width: 40px;
    text-align: center;
  }

  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }

  li {
    padding: 8px 10px;
    cursor: pointer;
    border-radius: 4px;
    margin-bottom: 5px;
    background: #f9f9f9;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    border: 2px solid transparent;
  }

  li:hover {
    background: #f0f0f0;
  }

  li.active {
    border-color: #2196f3;
    background: #e3f2fd;
  }

  .filename {
    font-weight: 500;
    flex-shrink: 0;
    max-width: 120px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page {
    font-size: 12px;
    color: #2196f3;
    flex-shrink: 0;
    background: #e3f2fd;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .match-count {
    font-size: 12px;
    color: #ff9800;
    flex-shrink: 0;
    background: #fff3e0;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .meta {
    font-size: 12px;
    color: #666;
    flex-shrink: 0;
  }

  .snippet {
    flex: 1;
    color: #666;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .snippet :global(mark) {
    background-color: #fff176;
    padding: 0 2px;
    border-radius: 2px;
  }

  .no-results {
    padding: 20px;
    text-align: center;
    color: #666;
    background: #fff;
    border-top: 1px solid #ddd;
    flex-shrink: 0;
  }
</style>