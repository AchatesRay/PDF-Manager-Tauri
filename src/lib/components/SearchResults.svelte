<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  // 当前全局匹配索引（从0开始）
  let currentMatchIndex = 0;

  // 计算总匹配数（用于导航和显示）
  $: totalMatchCount = $searchResults.reduce((sum, r) => sum + (r.match_count || 0), 0);

  // 计算每个结果的累计匹配数量
  $: cumulativeMatches = (() => {
    const arr: number[] = [];
    let cum = 0;
    for (const r of $searchResults) {
      arr.push(cum);
      cum += r.match_count || 0;
    }
    return arr;
  })();

  // 根据全局匹配索引找到对应的结果索引和页内匹配索引
  function findMatchPosition(globalIndex: number): { resultIndex: number; inPageIndex: number } {
    for (let i = 0; i < $searchResults.length; i++) {
      const cum = cumulativeMatches[i];
      const count = $searchResults[i].match_count || 0;
      if (count > 0 && globalIndex >= cum && globalIndex < cum + count) {
        return { resultIndex: i, inPageIndex: globalIndex - cum };
      }
    }
    return { resultIndex: 0, inPageIndex: 0 };
  }

  // 当搜索结果变化时重置索引并跳转
  let lastSearchResultsLength = 0;
  $: if ($searchResults.length > 0 && $searchResults.length !== lastSearchResultsLength) {
    lastSearchResultsLength = $searchResults.length;
    currentMatchIndex = 0;
    // 自动跳转到第一个结果
    navigateToMatch(0);
  }

  // 当前显示的PDF ID和页码（用于判断是否需要切换页面）
  let currentPdfId: number | null = null;
  let currentPageNumber: number | null = null;

  async function navigateToMatch(globalIndex: number) {
    const { resultIndex, inPageIndex } = findMatchPosition(globalIndex);
    const result = $searchResults[resultIndex];
    if (!result) return;

    currentMatchIndex = globalIndex;

    // 只有当PDF或页码变化时才重新加载
    if (currentPdfId !== result.pdf_id || currentPageNumber !== result.page_number) {
      currentPdfId = result.pdf_id;
      currentPageNumber = result.page_number;
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
  }

  async function handleResultClick(index: number) {
    // 点击结果时跳转到该结果的第一个匹配
    const globalIndex = cumulativeMatches[index];
    await navigateToMatch(globalIndex);
  }

  async function navigatePrev() {
    if (totalMatchCount === 0) return;
    const newIndex = currentMatchIndex > 0 ? currentMatchIndex - 1 : totalMatchCount - 1;
    await navigateToMatch(newIndex);
  }

  async function navigateNext() {
    if (totalMatchCount === 0) return;
    const newIndex = currentMatchIndex < totalMatchCount - 1 ? currentMatchIndex + 1 : 0;
    await navigateToMatch(newIndex);
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
  // isCurrentResult: 当前结果是否被选中
  // inPageIndex: 当前选中的是第几个匹配（从0开始）
  function renderSnippet(snippet: string, isCurrentResult: boolean, inPageIndex: number): string {
    // 将 **text** 转换为高亮标记
    if (!isCurrentResult) {
      // 非当前页：所有高亮用黄色
      return snippet.replace(/\*\*(.+?)\*\*/g, '<mark>$1</mark>');
    }

    // 当前页：第 inPageIndex 个高亮用橙色，其他用黄色
    let highlightIndex = 0;
    return snippet.replace(/\*\*(.+?)\*\*/g, (match, text) => {
      const idx = highlightIndex;
      highlightIndex++;
      if (idx === inPageIndex) {
        return `<mark class="current-match" data-index="${idx}">${text}</mark>`;
      }
      return `<mark data-index="${idx}">${text}</mark>`;
    });
  }

  // 获取当前匹配所在的结果索引和页内索引
  $: currentMatchPosition = findMatchPosition(currentMatchIndex);
  $: currentResultIndex = currentMatchPosition.resultIndex;
  $: currentInPageIndex = currentMatchPosition.inPageIndex;

  // 结果列表元素引用（用于滚动）
  let resultListElement: HTMLUListElement | null = null;

  // 滚动到当前选中的结果
  function scrollToCurrentResult() {
    if (!resultListElement) return;
    const activeItem = resultListElement.querySelector('li.active');
    if (activeItem) {
      activeItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' });
    }
  }

  // 滚动 snippet 到当前高亮的关键字
  function scrollSnippetToCurrentMatch() {
    if (!resultListElement) return;
    const activeItem = resultListElement.querySelector('li.active');
    if (!activeItem) return;

    const snippetEl = activeItem.querySelector('.snippet') as HTMLElement;
    const currentMatch = activeItem.querySelector(`mark[data-index="${currentInPageIndex}"]`) as HTMLElement;

    if (snippetEl && currentMatch) {
      // 计算滚动位置，让当前关键字居中显示
      const snippetWidth = snippetEl.clientWidth;
      const matchLeft = currentMatch.offsetLeft;
      const matchWidth = currentMatch.offsetWidth;
      const scrollLeft = matchLeft - (snippetWidth / 2) + (matchWidth / 2);

      snippetEl.scrollTo({
        left: Math.max(0, scrollLeft),
        behavior: 'smooth'
      });
    }
  }

  // 当结果索引或页内索引变化时滚动
  $: if (currentResultIndex >= 0) {
    setTimeout(scrollToCurrentResult, 50);
    setTimeout(scrollSnippetToCurrentMatch, 50);
  }
</script>

{#if $showSearchResults}
  {#if $searchMode === 'content'}
    {#if $searchResults.length > 0}
      <div class="search-results">
        <div class="header-row">
          <h4>内容搜索结果 ({totalMatchCount}处匹配)</h4>
          <div class="nav-buttons">
            <button on:click={navigatePrev} title="上一个">上一个</button>
            <span class="index-info">{currentMatchIndex + 1}/{totalMatchCount}</span>
            <button on:click={navigateNext} title="下一个">下一个</button>
          </div>
        </div>
        <ul class="result-list" bind:this={resultListElement}>
          {#each $searchResults as result, index}
            <li
              class:active={index === currentResultIndex}
              on:click={() => handleResultClick(index)}
            >
              <span class="filename" title={result.filename}>{result.filename}</span>
              <span class="page">P{result.page_number}</span>
              <span class="match-count">{result.match_count}处</span>
              <span class="snippet">
                {@html renderSnippet(result.snippet, index === currentResultIndex, currentInPageIndex)}
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
        <div class="header-row">
          <h4>文件名搜索结果 ({$filenameSearchResults.length})</h4>
        </div>
        <ul class="result-list">
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
    max-height: 250px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }

  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 10px;
    background: #f5f5f5;
    border-bottom: 1px solid #ddd;
    position: sticky;
    top: 0;
    z-index: 10;
    flex-shrink: 0;
  }

  h4 {
    margin: 0;
    font-size: 13px;
    color: #333;
  }

  .nav-buttons {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .nav-buttons button {
    padding: 3px 8px;
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
    min-width: 35px;
    text-align: center;
  }

  .result-list {
    list-style: none;
    padding: 0;
    margin: 0;
    overflow-y: auto;
    max-height: 200px;
  }

  li {
    padding: 6px 10px;
    cursor: pointer;
    border-radius: 4px;
    margin: 4px 6px;
    background: #f9f9f9;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
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
    max-width: 100px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .page {
    font-size: 11px;
    color: #2196f3;
    flex-shrink: 0;
    background: #e3f2fd;
    padding: 1px 5px;
    border-radius: 3px;
  }

  .match-count {
    font-size: 11px;
    color: #ff9800;
    flex-shrink: 0;
    background: #fff3e0;
    padding: 1px 5px;
    border-radius: 3px;
  }

  .meta {
    font-size: 11px;
    color: #666;
    flex-shrink: 0;
  }

  .snippet {
    flex: 1;
    color: #666;
    overflow-x: auto;
    white-space: nowrap;
    min-width: 0;
    scrollbar-width: none;
  }

  .snippet::-webkit-scrollbar {
    display: none;
  }

  .snippet :global(mark) {
    background-color: #fff176;
    padding: 0 2px;
    border-radius: 2px;
  }

  .snippet :global(mark.current-match) {
    background-color: #ff9800;
    color: white;
  }

  .no-results {
    padding: 15px;
    text-align: center;
    color: #666;
    background: #fff;
    border-top: 1px solid #ddd;
    flex-shrink: 0;
    font-size: 13px;
  }
</style>