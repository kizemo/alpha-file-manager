// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：WebView2 子进程崩溃兜底的前端接线。
//
// 后端看门狗（src-tauri/src/webview_guard.rs + webview_guard_runtime.rs）
// 靠两件事判断界面是不是死了：
//
// 1. 心跳 —— 这里每 5 秒 invoke 一次 `webview_guard_heartbeat`；
// 2. 进程旁证 —— 后端自己扫 `msedgewebview2.exe` 的进程树。
//
// 前端只负责两件：稳定发心跳；把后端推来的状态变成用户看得见的提示。
//
// ⚠️ 心跳不能因为窗口被隐藏就停发 —— Chromium 会限流后台定时器，
// 那样后端会把「限流」误判成「界面卡死」。发不停，
// 由后端按窗口可见性决定要不要判卡死。

import { onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { UnlistenFn } from '@tauri-apps/api/event';
import { useI18n } from 'vue-i18n';
import { toast } from '@/components/ui/toaster';

/** 后端推送看门狗状态的事件名。 */
export const WEBVIEW_GUARD_EVENT = 'webview-guard://status';

/** 心跳间隔（毫秒）。必须明显短于后端的 20 秒超时。 */
export const WEBVIEW_GUARD_HEARTBEAT_INTERVAL_MS = 5_000;

/** 后端会推来的状态种类。 */
export type WebviewGuardNoticeKind
  = | 'renderer-crashed'
    | 'window-hung'
    | 'browser-gone'
    | 'recovered'
    | 'recovery-failed';

export interface WebviewGuardNotice {
  kind: WebviewGuardNoticeKind;
  label: string;
  detail: string;
  recoverable: boolean;
}

export type WebviewGuardNoticeTone = 'info' | 'success' | 'error' | 'warning';

/** 提示文案。用 getter 取，这样切换语言之后提示也跟着变。 */
export interface WebviewGuardMessages {
  detected: string;
  recovered: string;
  recoveryFailed: string;
}

/**
 * 把后端的状态翻译成一条用户看得见的提示。
 *
 * 单独导出来是为了能直接单测，不用挂载组件。
 */
export function describeWebviewGuardNotice(
  kind: WebviewGuardNoticeKind,
  messages: WebviewGuardMessages,
): {
  tone: WebviewGuardNoticeTone;
  message: string;
} {
  switch (kind) {
    case 'recovered':
      return {
        tone: 'success',
        message: messages.recovered,
      };
    case 'recovery-failed':
      return {
        tone: 'error',
        message: messages.recoveryFailed,
      };
    case 'renderer-crashed':
    case 'window-hung':
    case 'browser-gone':
      return {
        tone: 'warning',
        message: messages.detected,
      };
    default:
      return {
        tone: 'info',
        message: messages.detected,
      };
  }
}

async function sendHeartbeat() {
  try {
    await invoke('webview_guard_heartbeat');
  }
  catch (error) {
    // 心跳发不出去本身就是一种信号，但不在这里升级 —— 判定归后端。
    console.error('Failed to send the webview guard heartbeat:', error);
  }
}

function showNotice(notice: WebviewGuardNotice, messages: WebviewGuardMessages) {
  const { tone, message } = describeWebviewGuardNotice(notice.kind, messages);

  if (tone === 'success') {
    toast.success(message);
  }
  else if (tone === 'error') {
    toast.error(message);
  }
  else if (tone === 'warning') {
    toast.warning(message);
  }
  else {
    toast.info(message);
  }
}

export interface WebviewGuardHandle {
  stop: () => void;
}

/**
 * 启动看门狗的前端部分：发心跳 + 订阅状态事件。
 *
 * 与组件生命周期解耦，导出成普通函数是为了能直接单测。
 */
export async function startWebviewGuard(
  getMessages: () => WebviewGuardMessages,
): Promise<WebviewGuardHandle> {
  await sendHeartbeat();

  const heartbeatTimer = setInterval(() => {
    void sendHeartbeat();
  }, WEBVIEW_GUARD_HEARTBEAT_INTERVAL_MS);

  let unlisten: UnlistenFn | null = null;

  try {
    unlisten = await listen<WebviewGuardNotice>(WEBVIEW_GUARD_EVENT, (event) => {
      showNotice(event.payload, getMessages());
    });
  }
  catch (error) {
    console.error('Failed to subscribe to the webview guard events:', error);
  }

  return {
    stop() {
      clearInterval(heartbeatTimer);
      unlisten?.();
      unlisten = null;
    },
  };
}

/**
 * 在组件里启用看门狗（主窗口 layout 里调用）。
 */
export function useWebviewGuard() {
  const { t } = useI18n();
  let handle: WebviewGuardHandle | null = null;
  let disposed = false;

  onMounted(async () => {
    const started = await startWebviewGuard(() => ({
      detected: t('webviewGuard.detected'),
      recovered: t('webviewGuard.recovered'),
      recoveryFailed: t('webviewGuard.recoveryFailed'),
    }));

    // 订阅还没回来组件就被卸载了 ⇒ 立刻收摊，别把定时器留在后台。
    if (disposed) {
      started.stop();
      return;
    }

    handle = started;
  });

  onUnmounted(() => {
    disposed = true;
    handle?.stop();
    handle = null;
  });
}
