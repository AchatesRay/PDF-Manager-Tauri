<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  async function handleContentResultClick(result: SearchResult) {
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
        <h4>内容搜索结果 ({$searchResults.length})</h4>
        <ul>
          {#each $searchResults as result}
            <li on:click={() => handleContentResultClick(result)}>
              <span class="filename">{result.filename}</span>
              <span class="page">P{result.page_number}</span>
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
    padding: 8px 10px;
    cursor: pointer;
    border-radius: 4px;
    margin-bottom: 5px;
    background: #f9f9f9;
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
  }

  li:hover {
    background: #f0f0f0;
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
    background: #e3f2fd;
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