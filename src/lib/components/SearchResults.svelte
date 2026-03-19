<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  async function handleContentResultClick(result: SearchResult) {
    selectedPdfId.set(result.pdf_id);
    jumpToPage.set(result.page_number);
    try {
      const detail = await getPdfDetail(result.pdf_id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
    }
  }

  async function handleFilenameResultClick(pdf: PdfInfo) {
    selectedPdfId.set(pdf.id);
    jumpToPage.set(null);
    try {
      const detail = await getPdfDetail(pdf.id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
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
              <div class="filename">{result.filename}</div>
              <div class="page">第 {result.page_number} 页</div>
              <div class="snippet">
                {@html renderSnippet(result.snippet)}
              </div>
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
              <div class="filename">{pdf.filename}</div>
              <div class="meta">{pdf.page_count} 页</div>
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

  .page, .meta {
    font-size: 12px;
    color: #2196f3;
    margin-bottom: 4px;
  }

  .snippet {
    font-size: 13px;
    color: #666;
    line-height: 1.4;
    word-break: break-word;
    white-space: normal;
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