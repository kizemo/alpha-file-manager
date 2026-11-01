// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 字体大小三档（需求 3，2026-10-31）。
//
// 断言两件事：
//   1. 切档位后 `--font-size-offset` 真的写进了根元素的内联 style；
//   2. 档位**真的落盘**（走一遍 mock store 的 set/entries 往返）。
//      只断言内存状态抓不到「界面变了但重启即丢」这类问题。

import { nextTick } from 'vue';
import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { USER_SETTINGS_SCHEMA_VERSION } from '@/stores/schemas/user-settings';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import type { StartupStorageFileBootstrap } from '@/stores/storage/utils/startup-storage-bootstrap';
import type { Theme } from '@/types/user-settings';

const {
  emitMock,
  lazyStoreSaveMock,
  lazyStoreSetMock,
  lazyStoreGetMock,
  listenMock,
  webviewSetZoomMock,
} = vi.hoisted(() => ({
  emitMock: vi.fn(),
  lazyStoreSaveMock: vi.fn(),
  lazyStoreSetMock: vi.fn(),
  lazyStoreGetMock: vi.fn(),
  listenMock: vi.fn(),
  webviewSetZoomMock: vi.fn(),
}));

// 落盘用的真值表：`set` 写进去，`entries()` 原样吐回来。
const storedValues = new Map<string, unknown>();

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    async save(): Promise<void> {
      await lazyStoreSaveMock();
    }

    async set(key: string, value: unknown): Promise<void> {
      await lazyStoreSetMock(key, value);
      storedValues.set(key, value);
    }

    async get(key: string): Promise<unknown> {
      await lazyStoreGetMock(key);

      return storedValues.get(key);
    }

    async entries(): Promise<[string, unknown][]> {
      return [...storedValues.entries()];
    }
  },
}));

vi.mock('@tauri-apps/plugin-os', () => ({
  locale: vi.fn(async () => null),
  platform: vi.fn(() => 'linux'),
}));

vi.mock('@tauri-apps/api/event', () => ({
  emit: emitMock,
  listen: listenMock,
}));

vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({
    setZoom: webviewSetZoomMock,
  }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@/utils/app-dirs', () => ({
  appDataDir: vi.fn(),
  isIsolatedDataDir: vi.fn(async () => false),
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataSettingsPath: '/tmp/user-data/user-settings.json',
    },
  }),
}));

function createBootstrap(theme: Theme): StartupStorageFileBootstrap {
  return {
    path: '/tmp/user-data/user-settings.json',
    status: 'ready',
    data: {
      __schemaVersion: USER_SETTINGS_SCHEMA_VERSION,
      theme,
    },
    schemaVersion: USER_SETTINGS_SCHEMA_VERSION,
    error: null,
  };
}

function readOffsetVariable(): string {
  return document.documentElement.style.getPropertyValue('--font-size-offset');
}

describe('user settings font size tier', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    storedValues.clear();
    document.documentElement.className = '';
    document.documentElement.style.cssText = '';
    emitMock.mockReset().mockResolvedValue(undefined);
    lazyStoreSaveMock.mockReset();
    lazyStoreSetMock.mockReset();
    lazyStoreGetMock.mockReset().mockResolvedValue(undefined);
    webviewSetZoomMock.mockReset();
    listenMock.mockReset().mockResolvedValue(vi.fn());

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
      })),
    });
  });

  it('默认档位注入 0px 偏移', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap('dark'));

    expect(userSettingsStore.userSettings.text.fontSize).toBe('default');
    expect(userSettingsStore.fontSizeOffset).toBe(0);
    expect(readOffsetVariable()).toBe('0px');
  });

  it('切到「大」后注入 2px 并落盘', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap('dark'));
    await userSettingsStore.set('text.fontSize', 'large');
    await nextTick();

    expect(userSettingsStore.fontSizeOffset).toBe(2);
    expect(readOffsetVariable()).toBe('2px');
    // 真往返：写进 mock store 的键值对，不是只在内存里飘着。
    expect(storedValues.get('text.fontSize')).toBe('large');
  });

  it('切到「特大」后注入 4px 并落盘', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap('dark'));
    await userSettingsStore.set('text.fontSize', 'extra-large');
    await nextTick();

    expect(userSettingsStore.fontSizeOffset).toBe(4);
    expect(readOffsetVariable()).toBe('4px');
    expect(storedValues.get('text.fontSize')).toBe('extra-large');
  });

  it('非法档位回落到默认偏移 0px', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap('dark'));
    await userSettingsStore.set('text.fontSize', 'xlarge' as never);
    await nextTick();

    expect(userSettingsStore.fontSizeOffset).toBe(0);
    expect(readOffsetVariable()).toBe('0px');
  });
});
