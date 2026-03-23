# 文件树OCR数量显示设计

## 概述

在文件夹树中显示每个文件夹的OCR处理进度，格式为 `已OCR数/总数`，全部完成时显示绿色徽章。

## 功能需求

1. 文件夹后显示 `已OCR数/总数` 格式（如 `3/5`）
2. 数量徽章靠右对齐
3. 父文件夹数量包含所有子文件夹的文件
4. "全部文件"显示程序内PDF总数
5. 全部完成时（OCR数=总数且总数>0）显示绿色徽章

## 技术设计

### 数据模型

PDF状态字段（已存在）：
```typescript
status: 'pending' | 'processing' | 'done' | 'error'
```

`status === 'done'` 表示OCR已完成。

### 组件修改

#### FolderTree.svelte

新增函数：
```javascript
// 获取文件夹的OCR统计（递归）
function getFolderOcrStats(folderId: number | null): { ocrCount: number; totalCount: number } {
  const ids = getAllFolderIds(folderId);
  const pdfs = $pdfList.filter(p => ids.includes(p.folder_id ?? 0));
  return {
    ocrCount: pdfs.filter(p => p.status === 'done').length,
    totalCount: pdfs.length
  };
}
```

模板修改（"全部文件"项）：
```svelte
<script>
  $: allStats = getFolderOcrStats(null);
</script>

<li class="folder-item" class:active={$selectedFolderId === null} on:click={() => selectFolder(null)}>
  <!-- ... -->
  <span class="folder-count" class:completed={allStats.ocrCount === allStats.totalCount && allStats.totalCount > 0}>
    {allStats.ocrCount}/{allStats.totalCount}
  </span>
</li>
```

#### FolderNode.svelte

Props 新增：
```typescript
export let getFolderOcrStats: (folderId: number | null) => { ocrCount: number; totalCount: number };
```

模板修改：
```svelte
<script>
  $: stats = getFolderOcrStats(node.id);
</script>

<span class="folder-count" class:completed={stats.ocrCount === stats.totalCount && stats.totalCount > 0}>
  {stats.ocrCount}/{stats.totalCount}
</span>
```

递归传递props：
```svelte
<svelte:self
  node={child}
  level={level + 1}
  {onDelete}
  {onAddSubfolder}
  {getFolderOcrStats}
  ...
/>
```

### 样式

```css
.folder-count {
  font-size: 10px;
  color: var(--text-muted, #9ca3af);
  background: var(--bg-tertiary, #f5f7f9);
  padding: 1px 5px;
  border-radius: 3px;
  flex-shrink: 0;
  margin-left: auto;
  font-variant-numeric: tabular-nums;
  min-width: 32px;
  text-align: center;
}

.folder-count.completed {
  background: #dcfce7;
  color: #16a34a;
}
```

## 涉及文件

| 文件 | 修改内容 |
|------|----------|
| `src/lib/components/FolderTree.svelte` | 新增 `getFolderOcrStats` 函数，修改模板 |
| `src/lib/components/FolderNode.svelte` | 新增 prop，修改模板和样式 |

## 测试要点

1. 父文件夹数量正确累加子文件夹
2. OCR完成后数字实时更新
3. 全部完成时徽章变绿
4. 空文件夹显示 `0/0`