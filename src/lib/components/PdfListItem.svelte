<script lang="ts">
  import type { PdfInfo } from '../api';
  import type { OcrProgress } from '../stores';

  export let pdf: PdfInfo;
  export let active: boolean = false;
  export let queuePosition: number | null = null;
  export let progress: OcrProgress | undefined = undefined;
  export let onSelect: (id: number) => void;
  export let onStartOcr: (id: number, force?: boolean) => void;
  export let onCancel: (id: number) => void;
  export let onDelete: (id: number) => void;

  function getStatusText(status: string): string {
    switch (status) {
      case 'pending': return '待处理';
      case 'processing': return '处理中';
      case 'done': return '已OCR';
      case 'error': return '出错';
      default: return status;
    }
  }

  function getTypeText(type: string): string {
    switch (type) {
      case 'text': return '文字';
      case 'scanned': return '扫描';
      case 'mixed': return '混合';
      default: return type;
    }
  }
</script>

<li
  class="pdf-item"
  class:active={active}
  on:click={() => onSelect(pdf.id)}
>
  <span class="filename" title={pdf.filename}>{pdf.filename}</span>
  <span class="meta">{pdf.page_count} 页</span>
  <span class="meta">{getTypeText(pdf.pdf_type)}</span>
  <div class="status-area">
    <span class="status-indicator status-{pdf.status}">
      <span class="status-dot"></span>
      <span class="status-text">
        {getStatusText(pdf.status)}
        {#if progress && progress.status === 'processing'}
          ({progress.current}/{progress.total})
        {/if}
      </span>
    </span>
  </div>
  <div class="actions">
    <!-- pending 且不在队列中：显示 OCR 按钮 -->
    {#if pdf.status === 'pending' && queuePosition === null}
      <button class="ocr-btn" on:click|stopPropagation={() => onStartOcr(pdf.id)}>OCR</button>
    {/if}

    <!-- pending 在队列中：显示取消按钮 -->
    {#if pdf.status === 'pending' && queuePosition !== null}
      <span class="queue-position">排队中 (#{queuePosition})</span>
      <button class="cancel-btn" on:click|stopPropagation={() => onCancel(pdf.id)} title="取消排队">取消</button>
    {/if}

    <!-- processing：显示加载动画 -->
    {#if pdf.status === 'processing'}
      <span class="processing-indicator">
        <svg class="spinner-small" viewBox="0 0 24 24">
          <circle cx="12" cy="12" r="10" stroke="currentColor" stroke-width="3" fill="none" stroke-dasharray="31.4 31.4"/>
        </svg>
      </span>
    {/if}

    <!-- done 或 error：显示 OCR 按钮重新识别 -->
    {#if pdf.status === 'done' || pdf.status === 'error'}
      <button class="ocr-btn re-ocr" on:click|stopPropagation={() => onStartOcr(pdf.id, true)}>OCR</button>
    {/if}

    <button class="delete-btn" on:click|stopPropagation={() => onDelete(pdf.id)} title="删除">
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="3 6 5 6 21 6"/>
        <path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>
      </svg>
    </button>
  </div>
</li>

<style>
  .pdf-item {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 10px;
    background: var(--bg-secondary, #ffffff);
    border: 1px solid var(--border-light, #f3f4f6);
    border-radius: 6px;
    margin-bottom: 4px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .pdf-item:hover {
    border-color: var(--border, #e5e7eb);
    box-shadow: 0 1px 2px rgba(0,0,0,0.04);
  }

  .pdf-item.active {
    border-color: var(--accent, #3b82f6);
    background: var(--accent-soft, #eff6ff);
  }

  .filename {
    flex: 1;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-primary, #1f2937);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }

  .meta {
    font-size: 11px;
    color: var(--text-muted, #9ca3af);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .status-area {
    flex-shrink: 0;
  }

  .status-indicator {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
  }

  .status-dot {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .status-pending .status-dot { background: var(--text-muted, #9ca3af); }
  .status-processing .status-dot { background: var(--warning, #f59e0b); }
  .status-done .status-dot { background: var(--success, #10b981); }
  .status-error .status-dot { background: var(--error, #ef4444); }

  .status-text {
    color: var(--text-secondary, #6b7280);
  }

  .actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
    margin-left: 6px;
  }

  .ocr-btn {
    padding: 4px 8px;
    background: var(--success, #10b981);
    color: white;
    border: none;
    border-radius: 4px;
    font-size: 10px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
  }

  .ocr-btn:hover {
    background: #059669;
  }

  .ocr-btn.re-ocr {
    background: var(--accent, #3b82f6);
  }

  .ocr-btn.re-ocr:hover {
    background: var(--accent-dark, #2563eb);
  }

  .queue-position {
    padding: 4px 6px;
    background: var(--bg-tertiary, #f3f4f6);
    color: var(--text-secondary, #6b7280);
    border-radius: 4px;
    font-size: 10px;
    white-space: nowrap;
  }

  .cancel-btn {
    padding: 4px 6px;
    background: transparent;
    color: var(--error, #ef4444);
    border: 1px solid var(--error, #ef4444);
    border-radius: 4px;
    font-size: 10px;
    cursor: pointer;
    transition: all 0.15s;
  }

  .cancel-btn:hover {
    background: var(--error, #ef4444);
    color: white;
  }

  .processing-indicator {
    display: flex;
    align-items: center;
  }

  .spinner-small {
    width: 14px;
    height: 14px;
    color: var(--warning, #f59e0b);
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .delete-btn {
    width: 24px;
    height: 24px;
    background: none;
    border: 1px solid var(--border, #e5e7eb);
    border-radius: 4px;
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted, #9ca3af);
    transition: all 0.15s;
  }

  .delete-btn:hover {
    border-color: var(--error, #ef4444);
    background: var(--error, #ef4444);
    color: white;
  }

  .delete-btn svg {
    width: 12px;
    height: 12px;
  }
</style>
