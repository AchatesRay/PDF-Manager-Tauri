import { confirm } from '@tauri-apps/plugin-dialog';

/**
 * 带兜底的确认框（删除修复 A2，2026-09-28）
 *
 * 背景：@tauri-apps/plugin-dialog 与 Rust 侧 tauri-plugin-dialog 版本错配时，
 * `confirm()` 会直接 reject（实测 "dialog.confirm not allowed. Command not found"），
 * 而各调用点的 await 在 try 之外 → 异常静默吞掉 → **删除等破坏性操作的确认框
 * 不出现、操作静默不执行且无任何提示**（2026-09-28 删除故障根因）。
 *
 * 策略：插件 confirm 失败时回退 `window.confirm`（WebView2 原生支持），
 * 并 console.error 留痕，保证确认框永远出现、操作永远不会静默丢失。
 */
export async function safeConfirm(
  message: string,
  options?: { title?: string; kind?: 'warning' | 'error' | 'info' }
): Promise<boolean> {
  try {
    return await confirm(message, options);
  } catch (e) {
    console.error('[safeConfirm] 插件确认框失败，回退 window.confirm:', e);
    try {
      return window.confirm(message);
    } catch (e2) {
      console.error('[safeConfirm] window.confirm 也失败，按取消处理:', e2);
      return false;
    }
  }
}
