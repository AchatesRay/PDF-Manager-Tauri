import type { Config } from '@tauri-apps/api/config';
import type { InvokeArgs } from '@tauri-apps/api/core';

declare global {
  interface Window {
    __TAURI__: {
      invoke: <T>(cmd: string, args?: InvokeArgs) => Promise<T>;
    };
  }
}

export {};