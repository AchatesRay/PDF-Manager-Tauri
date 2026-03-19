<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  // 当前匹配项索引（在展开后的所有匹配项中）
  let currentMatchIndex = 0;

  // 展开后的匹配项列表：每个关键词匹配作为一个独立的导航目标
  interface MatchItem {
    result: SearchResult;
    matchIndex: number; // 在该页面中的第几个匹配 (1-based)
  }

  // 将搜索结果展开为所有匹配项
  $: expandedMatches = $searchResults.flatMap((result: SearchResult) => {
    const count = result.match_count || 1;
    return Array.from({ length: count }, (_, i) => ({
      result,
      matchIndex: i + 1
    }));
  }) as MatchItem[];

  // 当前显示的匹配项
  $: currentMatch = expandedMatches.length > 0 ? expandedMatches[currentMatchIndex] : null;

  // 当搜索结果变化时重置索引
  $: if (expandedMatches.length > 0) {
    currentMatchIndex = Math.min(currentMatchIndex, expandedMatches.length - 1);
  }

  async function navigateToMatch(index: number) {
    const matchItem = expandedMatches[index];
    if (!matchItem) return;

    currentMatchIndex = index;
    try {
      const detail = await getPdfDetail(matchItem.result.pdf_id);
      selectedPdfId.set(matchItem.result.pdf_id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
      jumpToPage.set(matchItem.result.page_number);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
      alert('该PDF文件可能已被删除，请重新搜索');
    }
  }

  async function navigateMatch(direction: 'prev' | 'next') {
    if (expandedMatches.length === 0) return;

    if (direction === 'prev') {
      currentMatchIndex = currentMatchIndex > 0 ? currentMatchIndex - 1 : expandedMatches.length - 1;
    } else {
      currentMatchIndex = currentMatchIndex < expandedMatches.length - 1 ? currentMatchIndex + 1 : 0;
    }

    await navigateToMatch(currentMatchIndex);
  }

  // 初始化：跳转到第一个匹配项
  $: if (expandedMatches.length > 0 && currentMatchIndex === 0) {
    navigateToMatch(0);
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
          <h4>内容搜索结果 ({$searchResults.length}页, {expandedMatches.length}处匹配)</h4>
          <div class="nav-buttons">
            <button on:click={() => navigateMatch('prev')} title="上一个匹配">↑ 上一个</button>
            <span class="index-info">{currentMatchIndex + 1}/{expandedMatches.length}</span>
            <button on:click={() => navigateMatch('next')} title="下一个匹配">下一个 ↓</button>
          </div>
        </div>
        <div class="match-list">
          {#if currentMatch}
            <div class="match-item active">
              <span class="filename" title={currentMatch.result.filename}>{currentMatch.result.filename}</span>
              <span class="page">P{currentMatch.result.page_number}</span>
              <span class="match-index">#{currentMatch.matchIndex}/{currentMatch.result.match_count}</span>
              <span class="snippet">
                {@html renderSnippet(currentMatch.result.snippet)}
              </span>
            </div>
          {/if}
        </div>
      </div>
    {:else}
      <div class="no-results">
        未找到匹配的内容
      </div>
    {/if}
  {:else}
    {#if $filenameSearchResults.length > 0}
      <div class="search-results">
        <div class="header-row">
          <h4>文件名搜索结果 ({$filenameSearchResults.length})</h4>
        </div>
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
    background: #fff;
    border-top: 1px solid #ddd;
    max-height: 300px;
    overflow-y: auto;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }

  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 10px;
    background: #f5f5f5;
    border-bottom: 1px solid #ddd;
    position: sticky;
    top: 0;
    z-index: 10;
    flex-shrink: 0;
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

  .match-list {
    padding: 10px;
    flex: 1;
  }

  .match-item {
    padding: 10px;
    cursor: pointer;
    border-radius: 4px;
    background: #f9f9f9;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    border: 2px solid #2196f3;
    background: #e3f2fd;
  }

  .filename {
    font-weight: 500;
    flex-shrink: 0;
    max-width: 150px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page {
    font-size: 12px;
    color: #2196f3;
    flex-shrink: 0;
    background: #bbdefb;
    padding: 2px 6px;
    border-radius: 3px;
  }

  .match-index {
    font-size: 12px;
    color: #9c27b0;
    flex-shrink: 0;
    background: #f3e5f5;
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

  .no-results {
    padding: 20px;
    text-align: center;
    color: #666;
    background: #fff;
    border-top: 1px solid #ddd;
    flex-shrink: 0;
  }
</style>