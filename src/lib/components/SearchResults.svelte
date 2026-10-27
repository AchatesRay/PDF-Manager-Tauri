<script lang="ts">
  import { searchResults, selectedPdfId, selectedFolderId, revealPdfId, showSearchResults, searchMode, filenameSearchResults, selectedPdfPath, jumpToPage, selectedPdfPageCount, folders, searchQuery, pageHighlight, searchRevision } from '../stores';
  import type { SearchResult, PdfInfo } from '../api';
  import { getPdfDetail, getPageMatches } from '../api';
  import { onMount } from 'svelte';

  interface PdfSearchGroup {
    pdfId: number;
    filename: string;
    folderId: number | null;
    pages: SearchResult[];
    totalMatches: number;
    firstFlatIndex: number;
  }

  let currentMatchIndex = 0;

  // ===== 文件名列宽（可拖动，localStorage 持久化） =====
  const FILENAME_WIDTH_KEY = 'contentSearchFilenameWidth';
  const FILENAME_MIN_WIDTH = 80;
  const FILENAME_MAX_WIDTH = 420;
  let filenameWidth = 150;
  let resizing = false;
  let resizeStartX = 0;
  let resizeStartWidth = 0;

  function clampFilenameWidth(width: number): number {
    return Math.max(FILENAME_MIN_WIDTH, Math.min(FILENAME_MAX_WIDTH, width));
  }

  function startResize(e: MouseEvent) {
    e.preventDefault();
    e.stopPropagation();
    resizing = true;
    resizeStartX = e.clientX;
    resizeStartWidth = filenameWidth;
    document.body.style.cursor = 'col-resize';
    document.body.style.userSelect = 'none';
  }

  function handleResizeMove(e: MouseEvent) {
    if (!resizing) return;
    filenameWidth = clampFilenameWidth(resizeStartWidth + (e.clientX - resizeStartX));
  }

  function stopResize() {
    if (!resizing) return;
    resizing = false;
    document.body.style.cursor = '';
    document.body.style.userSelect = '';
    try {
      localStorage.setItem(FILENAME_WIDTH_KEY, String(filenameWidth));
    } catch (e) {
      console.error('Failed to save filename width:', e);
    }
  }

  onMount(() => {
    try {
      const saved = Number(localStorage.getItem(FILENAME_WIDTH_KEY));
      if (Number.isFinite(saved) && saved > 0) {
        filenameWidth = clampFilenameWidth(saved);
      }
    } catch (e) {
      console.error('Failed to load filename width:', e);
    }
    window.addEventListener('mousemove', handleResizeMove);
    window.addEventListener('mouseup', stopResize);
    return () => {
      window.removeEventListener('mousemove', handleResizeMove);
      window.removeEventListener('mouseup', stopResize);
    };
  });

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

  // 原始结果按页返回，按 pdf_id 聚合成「一个 PDF 一行」；保留首次出现的扁平索引用于导航。
  function groupSearchResults(results: SearchResult[]): PdfSearchGroup[] {
    const groups = new Map<number, PdfSearchGroup>();
    results.forEach((result, flatIndex) => {
      let group = groups.get(result.pdf_id);
      if (!group) {
        group = {
          pdfId: result.pdf_id,
          filename: result.filename,
          folderId: result.folder_id,
          pages: [],
          totalMatches: 0,
          firstFlatIndex: flatIndex,
        };
        groups.set(result.pdf_id, group);
      }
      group.pages.push(result);
      group.totalMatches += result.match_count || 0;
    });
    return [...groups.values()];
  }

  $: groupedResults = groupSearchResults($searchResults);

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

  let lastSearchRevision = 0;
  // 按「搜索提交次数」触发（旧逻辑按结果条数变化触发：同条数/零结果的新搜索
  // 不会重置导航、旧高亮会残留在预览上）
  $: if ($searchResults.length > 0 && $searchRevision !== lastSearchRevision) {
    lastSearchRevision = $searchRevision;
    currentMatchIndex = 0;
    currentPdfId = null;
    currentPageNumber = null;
    navigateToMatch(0);
  }

  let currentPdfId: number | null = null;
  let currentPageNumber: number | null = null;

  async function navigateToMatch(globalIndex: number): Promise<boolean> {
    const { resultIndex, inPageIndex } = findMatchPosition(globalIndex);
    const result = $searchResults[resultIndex];
    if (!result) return false;

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
        return false;
      }
    }

    // 刷新预览高亮（必须在页变更 if 之外：同页翻「下一个」也要切换焦点）。
    // 旧数据无坐标 → rects=[] → 预览不高亮，导航不受影响（方案甲：force 重识别产出坐标）。
    try {
      const rects = await getPageMatches(result.pdf_id, result.page_number, $searchQuery.trim());
      pageHighlight.set({
        pdfId: result.pdf_id,
        page: result.page_number,
        query: $searchQuery.trim(),
        rects,
        activeIndex: inPageIndex,
      });
    } catch (e) {
      console.error('Failed to get page highlights:', e);
      pageHighlight.set(null);
    }

    return true;
  }

  // 判断 folderId 是否在 ancestorId 目录子树内（含自身）。
  function folderContains(ancestorId: number, folderId: number | null): boolean {
    if (folderId === null) return false;
    let currentId: number | null = folderId;
    const seen = new Set<number>();
    while (currentId !== null && !seen.has(currentId)) {
      if (currentId === ancestorId) return true;
      seen.add(currentId);
      const folder = $folders.find(f => f.id === currentId);
      if (!folder) return false;
      currentId = folder.parent_id;
    }
    return false;
  }

  // 通知中间 PDF 文件列表定位到目标文件；目标不在当前目录时先切换目录。
  function revealInPdfList(pdfId: number, folderId: number | null) {
    const currentFolder = $selectedFolderId;
    const alreadyVisible = currentFolder === null || folderContains(currentFolder, folderId);
    if (!alreadyVisible) {
      selectedFolderId.set(folderId);
    }
    revealPdfId.set(pdfId);
  }

  async function handleResultClick(groupIndex: number) {
    const group = groupedResults[groupIndex];
    if (!group) return;
    const navigated = await navigateToMatch(group.firstFlatIndex);
    if (navigated) {
      revealInPdfList(group.pdfId, group.folderId);
    }
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
      // 文件名跳转不带内容命中：清残留高亮（防上一次内容搜索的框串页）
      pageHighlight.set(null);
      revealInPdfList(pdf.id, pdf.folder_id);
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
  let currentResult: SearchResult | undefined;
  $: currentResult = $searchResults[currentResultIndex];
  $: currentGroupIndex = currentResult
    ? groupedResults.findIndex(group => group.pdfId === currentResult!.pdf_id)
    : -1;

  let resultListElement: HTMLUListElement | null = null;

  // 根据 folder_id 获取文件夹路径
  function getFolderPath(folderId: number | null): string {
    if (folderId === null) return '/';

    const folderMap = new Map<number, { name: string; parent_id: number | null }>();
    $folders.forEach(f => folderMap.set(f.id, { name: f.name, parent_id: f.parent_id }));

    const path: string[] = [];
    let currentId: number | null = folderId;

    while (currentId !== null) {
      const folder = folderMap.get(currentId);
      if (folder) {
        path.unshift(folder.name);
        currentId = folder.parent_id;
      } else {
        break;
      }
    }

    return '/' + path.join('/');
  }

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

  $: if (currentGroupIndex >= 0 && resultListElement) {
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
            <span class="match-count">{groupedResults.length} 个文件 · {totalMatchCount} 处匹配</span>
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
          {#each groupedResults as group, groupIndex}
            {@const isActive = group.pdfId === currentResult?.pdf_id}
            {@const displayResult = isActive && currentResult ? currentResult : group.pages[0]}
            {@const displayInPageIndex = isActive ? currentInPageIndex : 0}
            <li
              class:active={isActive}
              on:click={() => handleResultClick(groupIndex)}
            >
              <span class="filename" style="width: {filenameWidth}px" title={group.filename}>{group.filename}</span>
              <div
                class="col-resizer"
                role="separator"
                aria-orientation="vertical"
                title="拖动调整文件名列宽"
                on:mousedown={startResize}
                on:click|stopPropagation={() => {}}
              ></div>
              <span class="snippet">
                {@html renderSnippet(displayResult.snippet, isActive, displayInPageIndex)}
              </span>
              <span class="match-badge">{group.totalMatches}处</span>
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
              <div class="file-info">
                <span class="filename" title={pdf.filename}>{pdf.filename}</span>
                <span class="folder-path">{getFolderPath(pdf.folder_id)}</span>
              </div>
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
    min-width: 0;
    flex-shrink: 0;
  }

  .col-resizer {
    width: 7px;
    align-self: stretch;
    margin: -6px 0;
    cursor: col-resize;
    position: relative;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .col-resizer::before {
    content: '';
    width: 1px;
    height: 14px;
    background: var(--border, #e5e7eb);
    border-radius: 1px;
  }

  .col-resizer:hover::before,
  .col-resizer:active::before {
    background: var(--accent, #3b82f6);
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
    overflow-x: auto;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
    scrollbar-width: none;
  }

  .snippet::-webkit-scrollbar {
    display: none;
  }

  .snippet :global(mark) {
    background-color: #fef08a;
    padding: 0 2px;
    border-radius: 2px;
    color: inherit;
  }

  .snippet :global(mark.current-match) {
    background-color: var(--warning, #f59e0b);
    color: white;
  }

  .filename-list li {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .filename-list .filename {
    max-width: none;
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

  .file-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
    min-width: 0;
  }

  .folder-path {
    font-size: 10px;
    color: var(--text-muted, #9ca3af);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>