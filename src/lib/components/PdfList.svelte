<script lang="ts">
  import { pdfList, selectedPdfId, selectedFolderId, isLoading, selectedPdfPath, selectedPdfPageCount, ocrProgress, folders, ocrModelStatus, ocrQueue, searchResults, filenameSearchResults, pageHighlight, revealPdfId } from '../stores';
  import { getPdfList, addPdf, deletePdf, getPdfDetail, startOcr, getOcrStatus, getOcrQueueStatus, cancelOcrTask, movePdfs, getFolders, importPdfFolder } from '../api';
  import type { FolderImportProgress } from '../api';
  import { onMount, tick } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { open, message } from '@tauri-apps/plugin-dialog';
  import { safeConfirm } from '../dialog';
  import type { OcrProgress } from '../stores';
  import OcrModelSetup from './OcrModelSetup.svelte';
  import PdfListItem from './PdfListItem.svelte';

  // ===== 批量选择状态 =====
  let selectionMode = false;
  let selectedIds: number[] = [];
  let showMoveDialog = false;
  let isBatchWorking = false;

  // ===== 搜索结果定位 =====
  let listElement: HTMLUListElement | null = null;
  let revealFlashTimer: ReturnType<typeof setTimeout> | null = null;

  $: if ($revealPdfId !== null) {
    const targetId = $revealPdfId;
    revealPdfId.set(null);
    void scrollToPdf(targetId);
  }

  async function scrollToPdf(id: number) {
    await tick();
    if (!listElement) return;

    let item = listElement.querySelector<HTMLElement>(`[data-pdf-id="${id}"]`);
    if (!item) {
      // 切目录后列表可能还需一帧才能渲染出目标文件
      await tick();
      item = listElement.querySelector<HTMLElement>(`[data-pdf-id="${id}"]`);
    }
    if (!item) return;

    item.scrollIntoView({ behavior: 'smooth', block: 'center' });
    item.classList.add('reveal-flash');
    if (revealFlashTimer) clearTimeout(revealFlashTimer);
    revealFlashTimer = setTimeout(() => {
      item?.classList.remove('reveal-flash');
    }, 1400);
  }

  // ===== 导入本地文件夹状态 =====
  let isImporting = false;
  let importProgress: FolderImportProgress | null = null;

  /**
   * 删除成功后同步清理视图状态：
   * 后端已把 pdfs/pdf_pages(OCR内容)/索引/文件全删，但前端的搜索结果面板、
   * 文件名结果、预览高亮仍缓存着已删文档的命中 → 看起来「没删干净」。
   */
  function purgeDeletedFromViews(ids: number[]) {
    if (ids.length === 0) return;
    const idSet = new Set(ids);
    searchResults.update(rs => rs.filter(r => !idSet.has(r.pdf_id)));
    filenameSearchResults.update(rs => rs.filter(r => !idSet.has(r.id)));
    pageHighlight.update(h => (h && idSet.has(h.pdfId) ? null : h));
    if ($selectedPdfId !== null && idSet.has($selectedPdfId)) {
      selectedPdfId.set(null);
      selectedPdfPath.set(null);
      selectedPdfPageCount.set(0);
    }
  }

  onMount(() => {
    loadPdfs();
    refreshQueueStatus();

    // 检查 OCR 模型状态
    getOcrStatus()
      .then(status => ocrModelStatus.set(status))
      .catch(e => console.error('Failed to get OCR status:', e));

    // 监听 OCR 进度（事件驱动，无轮询）
    let unlistenProgress: (() => void) | null = null;
    let unlistenQueued: (() => void) | null = null;
    let unlistenImport: (() => void) | null = null;

    // 每个 pdf 上一次已知状态，用于检测转换以触发列表/队列刷新
    const lastStatus = new Map<number, string>();

    listen<OcrProgress>('ocr-progress', (event) => {
      const progress = event.payload;
      const prev = lastStatus.get(progress.pdf_id);

      // 最小更新：内容无变化则不触碰 store
      ocrProgress.update(map => {
        const old = map.get(progress.pdf_id);
        if (
          old &&
          old.current === progress.current &&
          old.total === progress.total &&
          old.status === progress.status
        ) {
          return map;
        }
        const next = new Map(map);
        next.set(progress.pdf_id, progress);
        return next;
      });

      if (prev !== progress.status) {
        lastStatus.set(progress.pdf_id, progress.status);

        if (progress.status === 'done' || progress.status === 'error') {
          loadPdfs();
          refreshQueueStatus();
        } else if (progress.status === 'processing') {
          // 任务真正开跑时同步一次队列（替代原轮询）
          refreshQueueStatus();
        }
      }
    }).then(unlisten => unlistenProgress = unlisten);

    // 监听排队事件
    listen<{ pdf_id: number; position: number }>('ocr-queued', (event) => {
      console.log('OCR 任务已排队:', event.payload);
      refreshQueueStatus();
    }).then(unlisten => unlistenQueued = unlisten);

    // 监听文件夹导入进度
    listen<FolderImportProgress>('folder-import-progress', (event) => {
      importProgress = event.payload;
    }).then(unlisten => unlistenImport = unlisten);

    return () => {
      unlistenProgress?.();
      unlistenQueued?.();
      unlistenImport?.();
    };
  });

  async function refreshQueueStatus() {
    try {
      const status = await getOcrQueueStatus();
      ocrQueue.set(status);
    } catch (e) {
      console.error('Failed to get queue status:', e);
    }
  }

  async function loadPdfs() {
    isLoading.set(true);
    try {
      pdfList.set(await getPdfList());
    } catch (e) {
      console.error('Failed to load PDFs:', e);
    } finally {
      isLoading.set(false);
    }
  }

  async function handleAddPdf() {
    if ($folders.length === 0) {
      await message('请先创建一个文件夹，然后选中该文件夹再导入PDF文件。', {
        title: '提示',
        kind: 'info',
      });
      return;
    }

    if ($selectedFolderId === null) {
      await message('请先在左侧选择一个文件夹，然后再添加PDF文件。', {
        title: '提示',
        kind: 'info',
      });
      return;
    }

    const selected = await open({
      multiple: true,
      filters: [{ name: 'PDF', extensions: ['pdf'] }],
      title: '选择 PDF 文件',
    });

    if (!selected) {
      return;
    }

    const files = Array.isArray(selected) ? selected : [selected];

    let added = 0;
    const skipped: string[] = []; // 文件名已存在 / 已添加过
    const failed: string[] = [];  // 其它错误

    for (const filePath of files) {
      try {
        await addPdf(filePath, $selectedFolderId ?? undefined);
        added++;
      } catch (err) {
        const msg = String(err);
        const base = filePath.split(/[\\/]/).pop() || filePath;
        if (msg.includes('文件名已存在') || msg.includes('已添加过')) {
          skipped.push(base);
        } else {
          failed.push(`${base}: ${msg}`);
        }
      }
    }
    await loadPdfs();

    // 汇总提示（重名跳过不中断导入，结束后一条消息）
    if (skipped.length > 0 || failed.length > 0) {
      let text = `导入完成：成功 ${added} 个`;
      if (skipped.length > 0) {
        text += `，跳过 ${skipped.length} 个（文件名已存在）：\n${skipped.join('\n')}`;
      }
      if (failed.length > 0) {
        text += `\n失败 ${failed.length} 个：\n${failed.join('\n')}`;
      }
      await message(text, { title: '导入结果', kind: skipped.length > 0 || failed.length > 0 ? 'warning' : 'info' });
    }
  }

  /**
   * 导入本地已有 PDF 文件夹：
   * 顶级目录以所选文件夹命名，递归导入并镜像子目录结构。
   */
  async function handleImportFolder() {
    if (isImporting) return;

    const selected = await open({
      directory: true,
      multiple: false,
      title: '选择包含 PDF 的文件夹',
    });

    if (!selected) {
      return;
    }

    isImporting = true;
    importProgress = null;

    try {
      // 导入到当前选中的分类下；未选中分类（全部文件）时放在根级
      const targetFolderId = $selectedFolderId;
      const targetFolderName = targetFolderId === null
        ? null
        : ($folders.find(f => f.id === targetFolderId)?.name ?? null);
      const report = await importPdfFolder(selected as string, targetFolderId ?? undefined);

      // 刷新文件夹树并定位到新导入的文件夹
      folders.set(await getFolders());
      selectedFolderId.set(report.folder_id);
      await loadPdfs();

      let text = targetFolderName
        ? `已导入到「${targetFolderName}」下：新建分类「${report.folder_name}」，成功 ${report.imported} / ${report.total_pdfs} 个`
        : `已导入文件夹「${report.folder_name}」：成功 ${report.imported} / ${report.total_pdfs} 个`;
      if (report.skipped.length > 0) {
        text += `，跳过 ${report.skipped.length} 个（已存在）`;
      }
      if (report.failed.length > 0) {
        text += `，失败 ${report.failed.length} 个`;
        const details = report.failed
          .slice(0, 8)
          .map(f => `${f.path}: ${f.reason}`)
          .join('\n');
        text += `\n\n${details}`;
        if (report.failed.length > 8) text += `\n…`;
      }

      await message(text, {
        title: '导入完成',
        kind: report.failed.length > 0 ? 'warning' : 'info',
      });
    } catch (e) {
      await message('导入失败: ' + e, { title: '导入文件夹', kind: 'error' });
    } finally {
      isImporting = false;
      importProgress = null;
    }
  }

  // ===== 批量操作 =====

  function toggleSelect(id: number) {
    if (selectedIds.includes(id)) {
      selectedIds = selectedIds.filter(x => x !== id);
    } else {
      selectedIds = [...selectedIds, id];
    }
  }

  function selectAll() {
    selectedIds = filteredPdfs.map(p => p.id);
  }

  function clearSelection() {
    selectedIds = [];
  }

  function exitSelectionMode() {
    selectionMode = false;
    selectedIds = [];
  }

  function toggleSelectionMode() {
    if (selectionMode) {
      exitSelectionMode();
    } else {
      selectionMode = true;
      selectedIds = [];
    }
  }

  // 批量 OCR：pending → 普通识别；done/error → 强制重识别；processing/排队中 → 跳过
  async function handleBatchOcr() {
    if (selectedIds.length === 0) return;
    isBatchWorking = true;
    let queued = 0;
    let skipped = 0;
    const failList: string[] = [];

    for (const id of [...selectedIds]) {
      const pdf = $pdfList.find(p => p.id === id);
      if (!pdf) continue;
      if (pdf.status === 'processing' || getQueuePosition(id) !== null) {
        skipped++;
        continue;
      }
      try {
        await startOcr(id, pdf.status !== 'pending');
        queued++;
      } catch (e) {
        failList.push(`${pdf.filename}: ${e}`);
      }
    }

    await refreshQueueStatus();
    isBatchWorking = false;

    let text = `批量 OCR：已加入队列 ${queued} 个`;
    if (skipped > 0) text += `，跳过 ${skipped} 个（处理中/已排队）`;
    if (failList.length > 0) text += `\n失败：\n${failList.join('\n')}`;
    await message(text, { title: '批量 OCR', kind: 'info' });
  }

  // 批量删除：一次确认，循环删除，汇总结果
  async function handleBatchDelete() {
    if (selectedIds.length === 0) return;
    const confirmed = await safeConfirm(
      `确定要删除选中的 ${selectedIds.length} 个 PDF 吗？文件与 OCR 结果将一并删除。`,
      { title: '确认批量删除', kind: 'warning' }
    );
    if (!confirmed) return;

    isBatchWorking = true;
    let deleted = 0;
    const deletedIds: number[] = [];
    const failList: string[] = [];
    for (const id of [...selectedIds]) {
      const pdf = $pdfList.find(p => p.id === id);
      try {
        await deletePdf(id);
        deleted++;
        deletedIds.push(id);
      } catch (e) {
        failList.push(`${pdf?.filename || id}: ${e}`);
      }
    }
    isBatchWorking = false;
    clearSelection();
    purgeDeletedFromViews(deletedIds);
    await loadPdfs();

    let text = `删除完成：成功 ${deleted} 个`;
    if (failList.length > 0) text += `\n失败：\n${failList.join('\n')}`;
    await message(text, { title: '批量删除', kind: failList.length > 0 ? 'warning' : 'info' });
  }

  // 批量移动：选择目标文件夹 → 调 move_pdfs
  async function handleBatchMoveConfirm(targetFolderId: number | null) {
    if (selectedIds.length === 0) return;
    showMoveDialog = false;
    isBatchWorking = true;
    try {
      const report = await movePdfs([...selectedIds], targetFolderId ?? undefined);
      let text = `移动完成：成功 ${report.moved.length} 个`;
      if (report.skipped.length > 0) {
        const names = report.skipped.map(([pid, reason]) => {
          const pdf = $pdfList.find(p => p.id === pid);
          return `${pdf?.filename || pid}（${reason}）`;
        });
        text += `，跳过 ${report.skipped.length} 个：\n${names.join('\n')}`;
      }
      await message(text, { title: '批量移动', kind: report.skipped.length > 0 ? 'warning' : 'info' });
      clearSelection();
      await loadPdfs();
    } catch (e) {
      await message('移动失败: ' + e, { title: '批量移动', kind: 'error' });
    } finally {
      isBatchWorking = false;
    }
  }

  async function handleDelete(id: number) {
    const pdf = $pdfList.find(p => p.id === id);
    const filename = pdf?.filename || '此PDF';

    const confirmed = await safeConfirm(`确定要删除 "${filename}" 吗？`, {
      title: '确认删除',
      kind: 'warning',
    });

    if (confirmed) {
      try {
        await deletePdf(id);
        purgeDeletedFromViews([id]);
        await loadPdfs();
      } catch (e) {
        alert('删除失败: ' + e);
      }
    }
  }

  async function selectPdf(id: number) {
    selectedPdfId.set(id);
    try {
      const detail = await getPdfDetail(id);
      selectedPdfPath.set(detail.storage_path);
      selectedPdfPageCount.set(detail.page_count);
    } catch (e) {
      console.error('Failed to get PDF detail:', e);
      selectedPdfPath.set(null);
      selectedPdfPageCount.set(0);
    }
  }

  async function handleStartOcr(id: number, force: boolean = false) {
    try {
      await startOcr(id, force);
      await refreshQueueStatus();
    } catch (e) {
      alert('启动OCR失败: ' + e);
    }
  }

  async function handleCancelTask(id: number) {
    try {
      const cancelled = await cancelOcrTask(id);
      if (cancelled) {
        await refreshQueueStatus();
      } else {
        alert('无法取消正在运行的任务');
      }
    } catch (e) {
      alert('取消任务失败: ' + e);
    }
  }

  function getQueuePosition(pdfId: number): number | null {
    if ($ocrQueue.current === pdfId) return 0;
    const idx = $ocrQueue.pending.findIndex(t => t.pdf_id === pdfId);
    return idx >= 0 ? idx + 1 : null;
  }

  // 递归获取文件夹及其所有子文件夹的 ID
  function getAllFolderIds(folderId: number): number[] {
    const ids = [folderId];
    const children = $folders.filter(f => f.parent_id === folderId);
    for (const child of children) {
      ids.push(...getAllFolderIds(child.id));
    }
    return ids;
  }

  $: filteredPdfs = $selectedFolderId === null
    ? $pdfList
    : $pdfList.filter(p => getAllFolderIds($selectedFolderId!).includes(p.folder_id ?? 0));
</script>

<div class="pdf-list">
  <div class="list-header">
    <span class="list-title">
      PDF 文件
      <span class="count">{filteredPdfs.length} 个文档</span>
    </span>
    <div class="header-actions">
      <!-- OCR 模型状态和重新检测按钮 -->
      <div class="model-status-area">
        <OcrModelSetup />
      </div>
      <button class="add-btn" on:click={handleAddPdf}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="12" y1="5" x2="12" y2="19"/>
          <line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
        添加
      </button>
      <button
        class="add-btn"
        on:click={handleImportFolder}
        disabled={isImporting}
        title="导入本地已有 PDF 文件夹：作为当前选中分类的子文件夹，目录名使用所选文件夹名"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
          <line x1="12" y1="10" x2="12" y2="16"/>
          <polyline points="9 13 12 16 15 13"/>
        </svg>
        {isImporting ? '导入中…' : '导入文件夹'}
      </button>
      <button
        class="add-btn"
        class:active-toggle={selectionMode}
        on:click={toggleSelectionMode}
        disabled={filteredPdfs.length === 0}
        title="批量管理"
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polyline points="9 11 12 14 22 4"/>
          <path d="M21 12v7a2 2 0 01-2 2H5a2 2 0 01-2-2V5a2 2 0 012-2h11"/>
        </svg>
        {selectionMode ? '退出选择' : '批量管理'}
      </button>
    </div>
  </div>

  {#if isImporting}
    <div class="import-banner">
      <div class="spinner"></div>
      <span class="import-text">
        {#if importProgress}
          正在导入 {importProgress.current}/{importProgress.total}：{importProgress.relative_dir ? importProgress.relative_dir + '/' : ''}{importProgress.filename}
        {:else}
          正在扫描文件夹并创建目录…
        {/if}
      </span>
    </div>
  {/if}

  <!-- 批量操作栏 -->
  {#if selectionMode}
    <div class="batch-bar">
      <button class="batch-btn" on:click={selectAll} disabled={isBatchWorking}>全选</button>
      <button class="batch-btn" on:click={clearSelection} disabled={isBatchWorking || selectedIds.length === 0}>清空</button>
      <span class="batch-count">已选 {selectedIds.length} 项</span>
      <div class="batch-spacer"></div>
      <button class="batch-btn primary" on:click={handleBatchOcr} disabled={isBatchWorking || selectedIds.length === 0}>批量OCR</button>
      <button class="batch-btn move" on:click={() => showMoveDialog = true} disabled={isBatchWorking || selectedIds.length === 0}>批量移动</button>
      <button class="batch-btn danger" on:click={handleBatchDelete} disabled={isBatchWorking || selectedIds.length === 0}>批量删除</button>
    </div>
  {/if}

  {#if $isLoading}
    <div class="loading-state">
      <div class="spinner"></div>
      <span>加载中...</span>
    </div>
  {:else if filteredPdfs.length === 0}
    <div class="empty-state">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
        <path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/>
        <polyline points="14 2 14 8 20 8"/>
      </svg>
      <p>暂无 PDF 文件</p>
      <span>选择文件夹后点击"添加"导入文档</span>
    </div>
  {:else}
    <ul class="list" bind:this={listElement}>
      {#each filteredPdfs as pdf}
        <PdfListItem
          {pdf}
          active={$selectedPdfId === pdf.id}
          queuePosition={getQueuePosition(pdf.id)}
          progress={$ocrProgress.get(pdf.id)}
          selectable={selectionMode}
          checked={selectedIds.includes(pdf.id)}
          onToggle={toggleSelect}
          onSelect={selectPdf}
          onStartOcr={handleStartOcr}
          onCancel={handleCancelTask}
          onDelete={handleDelete}
        />
      {/each}
    </ul>
  {/if}
</div>

<!-- 批量移动：目标文件夹选择 -->
{#if showMoveDialog}
  <div class="move-overlay" on:click={() => showMoveDialog = false}>
    <div class="move-dialog" on:click|stopPropagation>
      <h4>移动 {selectedIds.length} 个 PDF 到</h4>
      <ul class="folder-pick">
        {#each $folders as folder}
          <li>
            <button class="folder-item" on:click={() => handleBatchMoveConfirm(folder.id)}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M22 19a2 2 0 01-2 2H4a2 2 0 01-2-2V5a2 2 0 012-2h5l2 3h9a2 2 0 012 2z"/>
              </svg>
              {folder.name}
            </button>
          </li>
        {/each}
        <li>
          <button class="folder-item" on:click={() => handleBatchMoveConfirm(null)}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 7h18v12a2 2 0 01-2 2H5a2 2 0 01-2-2z"/>
            </svg>
            根目录（未分类）
          </button>
        </li>
      </ul>
      <button class="move-cancel" on:click={() => showMoveDialog = false}>取消</button>
    </div>
  </div>
{/if}

<style>
  .pdf-list {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .list-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px 8px;
    padding: 8px 12px;
    flex-shrink: 0;
  }

  .list-title {
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .count {
    font-size: 11px;
    font-weight: 400;
    color: var(--text-muted, #9ca3af);
  }

  .header-actions {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    justify-content: flex-end;
    gap: 8px;
  }

  .model-status-area {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .add-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 5px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    cursor: pointer;
    transition: all 0.15s;
  }

  .add-btn:hover {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .add-btn svg {
    width: 12px;
    height: 12px;
  }

  .add-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }

  .import-banner {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 12px 6px;
    padding: 6px 10px;
    border-radius: 6px;
    background: var(--accent-soft, #eff6ff);
    border: 1px solid #bfdbfe;
    flex-shrink: 0;
    min-width: 0;
  }

  .import-banner .spinner {
    width: 13px;
    height: 13px;
    flex-shrink: 0;
  }

  .import-text {
    font-size: 11px;
    color: var(--text-secondary, #4b5563);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .list {
    list-style: none;
    padding: 0 8px 8px;
    margin: 0;
    flex: 1;
    overflow-y: auto;
  }

  /* 搜索结果定位后的短暂描边提示（子组件根节点，需全局选择器） */
  :global(.pdf-item.reveal-flash) {
    outline: 2px solid var(--accent, #3b82f6);
    outline-offset: -1px;
    box-shadow: 0 0 0 3px var(--accent-soft, #eff6ff);
  }

  .loading-state, .empty-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #9ca3af);
    gap: 10px;
  }

  .loading-state {
    flex-direction: row;
  }

  .spinner {
    width: 18px;
    height: 18px;
    border: 2px solid var(--border, #e5e7eb);
    border-top-color: var(--accent, #3b82f6);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .empty-state svg {
    width: 40px;
    height: 40px;
    opacity: 0.4;
  }

  .empty-state p {
    margin: 0;
    font-size: 13px;
    color: var(--text-secondary, #6b7280);
  }

  .empty-state span {
    font-size: 11px;
  }

  /* ===== 批量操作 ===== */
  .add-btn.active-toggle {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
    background: var(--accent-soft, #eff6ff);
  }

  .batch-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--border-light, #f3f4f6);
    background: var(--bg-tertiary, #f9fafb);
  }

  .batch-count {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    white-space: nowrap;
  }

  .batch-spacer {
    flex: 1;
  }

  .batch-btn {
    padding: 4px 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    cursor: pointer;
    transition: all 0.15s;
    white-space: nowrap;
  }

  .batch-btn:hover:not(:disabled) {
    border-color: var(--accent, #3b82f6);
    color: var(--accent, #3b82f6);
  }

  .batch-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .batch-btn.primary {
    background: var(--success, #10b981);
    border-color: var(--success, #10b981);
    color: white;
  }

  .batch-btn.primary:hover:not(:disabled) {
    background: #059669;
    border-color: #059669;
    color: white;
  }

  .batch-btn.move {
    background: var(--accent, #3b82f6);
    border-color: var(--accent, #3b82f6);
    color: white;
  }

  .batch-btn.move:hover:not(:disabled) {
    background: var(--accent-dark, #2563eb);
    border-color: var(--accent-dark, #2563eb);
    color: white;
  }

  .batch-btn.danger {
    color: var(--error, #ef4444);
    border-color: var(--error, #ef4444);
    background: transparent;
  }

  .batch-btn.danger:hover:not(:disabled) {
    background: var(--error, #ef4444);
    color: white;
  }

  /* 移动对话框 */
  .move-overlay {
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
  }

  .move-dialog {
    background: var(--bg-secondary, #ffffff);
    border-radius: 8px;
    padding: 16px;
    max-width: 360px;
    width: 90%;
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.15);
  }

  .move-dialog h4 {
    margin: 0 0 10px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-primary, #1f2937);
  }

  .folder-pick {
    list-style: none;
    margin: 0 0 10px;
    padding: 0;
    max-height: 300px;
    overflow-y: auto;
  }

  .folder-item {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    padding: 7px 8px;
    background: none;
    border: none;
    border-radius: 4px;
    font-size: 12px;
    color: var(--text-primary, #1f2937);
    cursor: pointer;
    text-align: left;
  }

  .folder-item:hover {
    background: var(--accent-soft, #eff6ff);
    color: var(--accent, #3b82f6);
  }

  .folder-item svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }

  .move-cancel {
    width: 100%;
    padding: 6px;
    background: none;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    font-size: 11px;
    color: var(--text-secondary, #6b7280);
    cursor: pointer;
  }

  .move-cancel:hover {
    border-color: var(--text-muted, #9ca3af);
  }
</style>