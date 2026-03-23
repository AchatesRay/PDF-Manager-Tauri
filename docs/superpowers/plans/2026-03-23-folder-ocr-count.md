# 文件树OCR数量显示实现计划

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在文件夹树中显示OCR处理进度，格式为 `已OCR数/总数`，全部完成时显示绿色徽章。

**Architecture:** 在现有 `getPdfCount` 函数基础上，新增 `getFolderOcrStats` 函数返回OCR统计对象。修改 `FolderNode.svelte` 接收新的统计函数并显示徽章。

**Tech Stack:** Svelte, TypeScript, Tauri

---

## Chunk 1: 修改 FolderTree.svelte

### Task 1: 添加 getFolderOcrStats 函数

**Files:**
- Modify: `src/lib/components/FolderTree.svelte:137-141`

- [ ] **Step 1: 添加 getFolderOcrStats 函数**

在 `getPdfCount` 函数后添加新函数（约第142行）：

```typescript
  // 获取文件夹的OCR统计（包括子文件夹）
  function getFolderOcrStats(folderId: number | null): { ocrCount: number; totalCount: number } {
    const ids = getAllFolderIds(folderId);
    const pdfs = $pdfList.filter(p => ids.includes(p.folder_id ?? 0));
    return {
      ocrCount: pdfs.filter(p => p.status === 'done').length,
      totalCount: pdfs.length
    };
  }
```

- [ ] **Step 2: 修改"全部文件"显示**

修改模板中的"全部文件"项（第291-302行），替换为：

```svelte
<script>
  // 在 script 标签内的响应式语句区域添加（约第240行前）
  $: allStats = getFolderOcrStats(null);
</script>

<!-- 修改模板 -->
<li
  class="folder-item"
  class:active={$selectedFolderId === null}
  on:click={() => selectFolder(null)}
>
  <svg class="folder-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
  </svg>
  <span class="folder-name">全部文件</span>
  <span class="folder-count" class:completed={allStats.ocrCount === allStats.totalCount && allStats.totalCount > 0}>
    {allStats.ocrCount}/{allStats.totalCount}
  </span>
</li>
```

- [ ] **Step 3: 修改 FolderNode 传递 getFolderOcrStats**

修改 FolderNode 组件调用（第304-318行），添加新prop：

```svelte
{#each treeNodes as node}
  <FolderNode
    {node}
    level={0}
    onDelete={handleDelete}
    onAddSubfolder={handleAddSubfolder}
    {getPdfCount}
    {getFolderOcrStats}
    newFolderParentId={newFolderParentId}
    newFolderName={newFolderName}
    {selectedStoragePath}
    onCreateFolder={handleCreate}
    onCancelFolder={handleCancel}
    onSelectDirectory={selectDirectory}
    onNewFolderNameChange={(v) => newFolderName = v}
  />
{/each}
```

- [ ] **Step 4: 添加 .completed 样式**

在 `<style>` 标签内，`.folder-count` 样式后添加（约第479行后）：

```css
  .folder-count.completed {
    background: #dcfce7;
    color: #16a34a;
  }
```

---

## Chunk 2: 修改 FolderNode.svelte

### Task 2: 修改 FolderNode 组件

**Files:**
- Modify: `src/lib/components/FolderNode.svelte`

- [ ] **Step 1: 添加 getFolderOcrStats prop**

在 script 标签内的 props 定义区域（约第13行后）添加：

```typescript
  export let getFolderOcrStats: (folderId: number | null) => { ocrCount: number; totalCount: number };
```

- [ ] **Step 2: 添加响应式统计计算**

在响应式语句区域（约第24行后）添加：

```svelte
  $: stats = getFolderOcrStats(node.id);
```

- [ ] **Step 3: 修改 folder-count 显示**

修改模板中的 folder-count（第73行），替换为：

```svelte
  <span class="folder-count" class:completed={stats.ocrCount === stats.totalCount && stats.totalCount > 0}>
    {stats.ocrCount}/{stats.totalCount}
  </span>
```

- [ ] **Step 4: 添加 .completed 样式**

在 `<style>` 标签内，`.folder-count` 样式后（约第226行后）添加：

```css
  .folder-count.completed {
    background: #dcfce7;
    color: #16a34a;
  }
```

- [ ] **Step 5: 递归传递 getFolderOcrStats**

修改 `<svelte:self>` 调用（第93-106行），添加新prop：

```svelte
    <svelte:self
      node={child}
      level={level + 1}
      {onDelete}
      {onAddSubfolder}
      {getPdfCount}
      {getFolderOcrStats}
      {newFolderParentId}
      {newFolderName}
      {selectedStoragePath}
      {onCreateFolder}
      {onCancelFolder}
      {onSelectDirectory}
      {onNewFolderNameChange}
    />
```

---

## Chunk 3: 提交和验证

### Task 3: 验证并提交

- [ ] **Step 1: 验证 TypeScript 编译**

```bash
npm run check
```
预期：无错误

- [ ] **Step 2: 提交更改**

```bash
git add src/lib/components/FolderTree.svelte src/lib/components/FolderNode.svelte
git commit -m "feat: 文件树显示OCR处理进度 (已OCR数/总数)"
```

---

## 测试要点

1. 父文件夹数量正确累加子文件夹
2. OCR完成后数字实时更新
3. 全部完成时徽章变绿
4. 空文件夹显示 `0/0`
5. "全部文件"显示所有PDF的总OCR进度