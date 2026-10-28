// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：WebView2 崩溃兜底的前端单测。
//
// 覆盖两件事：
//
// 1. 后端推来的状态种类 → 提示文案 / 语气 的映射；
// 2. 心跳的节奏与事件订阅（以及后端不可用时不炸）。

import {
  afterEach, beforeEach, describe, expect, it, vi,
} from 'vitest';

const {
  invokeMock,
  listenMock,
  unlistenMock,
  toastErrorMock,
  toastSuccessMock,
  toastWarningMock,
  toastInfoMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  unlistenMock: vi.fn(),
  toastErrorMock: vi.fn(),
  toastSuccessMock: vi.fn(),
  toastWarningMock: vi.fn(),
  toastInfoMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: toastErrorMock,
    success: toastSuccessMock,
    warning: toastWarningMock,
    info: toastInfoMock,
  },
}));

import {
  describeWebviewGuardNotice,
  startWebviewGuard,
  WEBVIEW_GUARD_EVENT,
  WEBVIEW_GUARD_HEARTBEAT_INTERVAL_MS,
} from '@/composables/use-webview-guard';
import type { WebviewGuardMessages, WebviewGuardNotice } from '@/composables/use-webview-guard';

const MESSAGES: WebviewGuardMessages = {
  detected: 'DETECTED',
  recovered: 'RECOVERED',
  recoveryFailed: 'RECOVERY_FAILED',
};

function getMessages(): WebviewGuardMessages {
  return MESSAGES;
}

function emitNotice(kind: WebviewGuardNotice['kind']) {
  const listener = listenMock.mock.calls[0]?.[1] as
    ((event: { payload: WebviewGuardNotice }) => void) | undefined;

  listener?.({
    payload: {
      kind,
      label: 'main',
      detail: '',
      recoverable: true,
    },
  });
}

describe('describeWebviewGuardNotice', () => {
  it('maps every notice kind to a tone and a message', () => {
    expect(describeWebviewGuardNotice('recovered', MESSAGES)).toEqual({
      tone: 'success',
      message: 'RECOVERED',
    });
    expect(describeWebviewGuardNotice('recovery-failed', MESSAGES)).toEqual({
      tone: 'error',
      message: 'RECOVERY_FAILED',
    });
    expect(describeWebviewGuardNotice('renderer-crashed', MESSAGES).tone).toBe('warning');
    expect(describeWebviewGuardNotice('window-hung', MESSAGES).tone).toBe('warning');
    expect(describeWebviewGuardNotice('browser-gone', MESSAGES).tone).toBe('warning');
  });
});

describe('startWebviewGuard', () => {
  beforeEach(() => {
    vi.useFakeTimers({
      toFake: ['setTimeout', 'clearTimeout', 'setInterval', 'clearInterval'],
    });
    invokeMock.mockReset();
    listenMock.mockReset();
    unlistenMock.mockReset();
    toastErrorMock.mockReset();
    toastSuccessMock.mockReset();
    toastWarningMock.mockReset();
    toastInfoMock.mockReset();

    invokeMock.mockResolvedValue(undefined);
    listenMock.mockResolvedValue(unlistenMock);
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it('sends a heartbeat right away and then on every interval', async () => {
    const handle = await startWebviewGuard(getMessages);

    expect(invokeMock).toHaveBeenCalledWith('webview_guard_heartbeat');
    expect(listenMock).toHaveBeenCalledWith(WEBVIEW_GUARD_EVENT, expect.any(Function));

    await vi.advanceTimersByTimeAsync(WEBVIEW_GUARD_HEARTBEAT_INTERVAL_MS);
    expect(invokeMock).toHaveBeenCalledTimes(2);

    handle.stop();
  });

  it('stops the heartbeat and the subscription on stop', async () => {
    const handle = await startWebviewGuard(getMessages);
    handle.stop();

    expect(unlistenMock).toHaveBeenCalledTimes(1);

    const callsBefore = invokeMock.mock.calls.length;
    await vi.advanceTimersByTimeAsync(WEBVIEW_GUARD_HEARTBEAT_INTERVAL_MS * 3);
    expect(invokeMock).toHaveBeenCalledTimes(callsBefore);
  });

  it('shows a toast for the notice kinds the backend reports', async () => {
    const handle = await startWebviewGuard(getMessages);

    emitNotice('renderer-crashed');
    expect(toastWarningMock).toHaveBeenCalledWith('DETECTED');

    emitNotice('recovered');
    expect(toastSuccessMock).toHaveBeenCalledWith('RECOVERED');

    emitNotice('recovery-failed');
    expect(toastErrorMock).toHaveBeenCalledWith('RECOVERY_FAILED');

    handle.stop();
  });

  it('survives a failing heartbeat and a failing subscription', async () => {
    invokeMock.mockRejectedValue(new Error('backend is gone'));
    listenMock.mockRejectedValue(new Error('no events here'));

    const handle = await startWebviewGuard(getMessages);

    expect(invokeMock).toHaveBeenCalledWith('webview_guard_heartbeat');
    handle.stop();
  });
});
