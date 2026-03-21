<script lang="ts">
  import { searchResults, selectedPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail } from '../api';

  let currentMatchIndex = 0;

  $: totalMatchCount = $searchResults.reduce((sum, r) => sum + (r.match_count || 0), 0);

  $: cumulativeMatches = (() => {
    const arr: number[] = [];
    let cum = 0;
    for (const r of $searchResults) {
      arr.push(cum);
      cum += r.match_count || 0;
    }
    return arr;
  })();

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

  let lastSearchResultsLength = 0;
  $: if ($searchResults.length > 0 && $searchResults.length !== lastSearchResultsLength) {
    lastSearchResultsLength = $searchResults.length;
    currentMatchIndex = 0;
    navigateToMatch(0);
  }

  let currentPdfId: number | null = null;
  let currentPageNumber: number | null = null;

  async function navigateToMatch(globalIndex: number) {
    const { resultIndex, inPageIndex } = findMatchPosition(globalIndex);
    const result = $searchResults[resultIndex];
    if (!result) return;

    currentMatchIndex = globalIndex;

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

  function renderSnippet(snippet: string, isCurrentResult: boolean, inPageIndex: number): string {
    if (!isCurrentResult) {
      return snippet.replace(/\*\*(.+?)\*\*/g, '<mark>$1</mark>');
    }

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

  $: currentMatchPosition = findMatchPosition(currentMatchIndex);
  $: currentResultIndex = currentMatchPosition.resultIndex;
  $: currentInPageIndex = currentMatchPosition.inPageIndex;

  let resultListElement: HTMLUListElement | null = null;

  function scrollToCurrent() {
    if (!resultListElement) return;
    const activeItem = resultListElement.querySelector('li.active');
    if (!activeItem) return;

    activeItem.scrollIntoView({ behavior: 'smooth', block: 'nearest' });

    const snippetEl = activeItem.querySelector('.snippet') as HTMLElement;
    const currentMatch = activeItem.querySelector(`mark[data-index="${currentInPageIndex}"]`) as HTMLElement;

    if (snippetEl && currentMatch) {
      currentMatch.scrollIntoView({
        behavior: 'smooth',
        inline: 'center',
        block: 'nearest'
      });
    }
  }

  $: if (currentResultIndex >= 0 && resultListElement) {
    void currentInPageIndex;
    setTimeout(scrollToCurrent, 100);
  }
</script>

{#if $showSearchResults}
  {#if $searchMode === 'content'}
    {#if $searchResults.length > 0}
      <div class="search-results">
        <div class="header-row">
          <div class="header-info">
            <h4>内容搜索结果</h4>
            <span class="match-count">{totalMatchCount} 处匹配</span>
          </div>
          <div class="nav-buttons">
            <button class="nav-btn" on:click={navigatePrev} title="上一个">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="15 18 9 12 15 6"/>
              </svg>
            </button>
            <span class="index-info">{currentMatchIndex + 1} / {totalMatchCount}</span>
            <button class="nav-btn" on:click={navigateNext} title="下一个">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="9 18 15 12 9 6"/>
              </svg>
            </button>
          </div>
        </div>
        <ul class="result-list" bind:this={resultListElement}>
          {#each $searchResults as result, index}
            <li
              class:active={index === currentResultIndex}
              on:click={() => handleResultClick(index)}
            >
              <span class="filename" title={result.filename}>{result.filename}</span>
              <span class="page-badge">P{result.page_number}</span>
              <span class="match-badge">{result.match_count}处</span>
              <span class="snippet">
                {@html renderSnippet(result.snippet, index === currentResultIndex, currentInPageIndex)}
              </span>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-results">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <span>未找到匹配的内容</span>
      </div>
    {/if}
  {:else}
    {#if $filenameSearchResults.length > 0}
      <div class="search-results">
        <div class="header-row">
          <div class="header-info">
            <h4>文件名搜索结果</h4>
            <span class="match-count">{$filenameSearchResults.length} 个文件</span>
          </div>
        </div>
        <ul class="result-list filename-list">
          {#each $filenameSearchResults as pdf}
            <li on:click={() => handleFilenameResultClick(pdf)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
                <polyline points="14 2 14 8 20 8"/>
              </svg>
              <span class="filename">{pdf.filename}</span>
              <span class="meta">{pdf.page_count} 页</span>
            </li>
          {/each}
        </ul>
      </div>
    {:else}
      <div class="no-results">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <circle cx="11" cy="11" r="8"/>
          <line x1="21" y1="21" x2="16.65" y2="16.65"/>
        </svg>
        <span>未找到匹配的文件名</span>
      </div>
    {/if}
  {/if}
{/if}

<style>
  .search-results {
    background: var(--bg-secondary, #ffffff);
    border-top: 1px solid var(--border, #e5e7eb);
    max-height: 240px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }

  .header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: 8px 12px;
    background: var(--bg-tertiary, #f5f7f9);
    border-bottom: 1px solid var(--border, #e5e7eb);
    flex-shrink: 0;
  }

  .header-info {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  h4 {
    margin: 0;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
  }

  .match-count {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    background: var(--bg-secondary, #ffffff);
    padding: 1px 6px;
    border-radius: 3px;
  }

  .nav-buttons {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .nav-btn {
    width: 24px;
    height: 24px;
    padding: 0;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 5px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s;
  }

  .nav-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .nav-btn svg {
    width: 12px;
    height: 12px;
  }

  .index-info {
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    min-width: 40px;
    text-align: center;
    font-weight: 500;
  }

  .result-list {
    list-style: none;
    padding: 0;
    margin: 0;
    overflow-y: auto;
    max-height: 180px;
  }

  li {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    cursor: pointer;
    border-bottom: 1px solid var(--border-light, #f3f4f6);
    transition: background 0.15s;
  }

  li:hover {
    background: var(--bg-tertiary, #f5f7f9);
  }

  li.active {
    background: var(--accent-soft, #eff6ff);
    border-left: 3px solid var(--accent, #3b82f6);
    padding-left: 9px;
  }

  .filename {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 120px;
    flex-shrink: 0;
  }

  .page-badge {
    font-size: 10px;
    color: var(--accent, #3b82f6);
    background: var(--accent-soft, #eff6ff);
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .match-badge {
    font-size: 10px;
    color: var(--warning, #f59e0b);
    background: #fffbeb;
    padding: 1px 5px;
    border-radius: 3px;
    flex-shrink: 0;
  }

  .snippet {
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    flex: 1;
    min-width: 0;
  }

  .snippet :global(mark) {
    background-color: #fef08a;
    padding: 0 2px;
    border-radius: 2px;
    color: #1f2937 !important;
  }

  .snippet :global(mark.current-match) {
    background-color: #f59e0b !important;
    color: #ffffff !important;
    font-weight: 600;
    text-shadow: none;
  }

  .filename-list li {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .filename-list svg {
    width: 16px;
    height: 16px;
    color: var(--text-muted, #9ca3af);
    flex-shrink: 0;
  }

  .meta {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    flex-shrink: 0;
    margin-left: auto;
  }

  .no-results {
    padding: 16px;
    text-align: center;
    color: var(--text-muted, #9ca3af);
    background: var(--bg-secondary, #ffffff);
    border-top: 1px solid var(--border, #e5e7eb);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .no-results svg {
    width: 28px;
    height: 28px;
    opacity: 0.5;
  }

  .no-results span {
    font-size: 12px;
  }
</style>