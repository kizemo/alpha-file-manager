// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-01：目录树开关（navigator.showFolderTree）跨重启失效。
//
// 缺陷现象：关掉目录树后重启应用，目录树又回来了。
// 根因：允许键集合（buildAllowedUserSettingsStorageKeys）是从**默认对象**自动派生的，
// 而默认对象里没有 `navigator.showFolderTree` ⇒ 读侧 applyUserSettingsEntries
// 把磁盘上真实的 false 静默丢弃 ⇒ 每次启动都回到默认（显示目录树）。
//
// 断言三件事：
//   1. 默认对象里带 `navigator.showFolderTree`（默认 true，与 navigator.vue 的 `?? true` 一致）；
//   2. 允许键集合从默认对象派生 ⇒ 含 'navigator.showFolderTree'；
//   3. **存储里存着 false 时，加载进内存仍然是 false** —— 这一条才有判别力：
//      只断言「默认值是 true」抓不到本 bug（恰恰是默认值缺键导致的）。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import {
  buildAllowedUserSettingsStorageKeys,
  USER_SETTINGS_SCHEMA_VERSION,
  USER_SETTINGS_SCHEMA_VERSION_KEY,
} from '@/stores/schemas/user-settings';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import type { StartupStorageFileBootstrap } from '@/stores/storage/utils/startup-storage-bootstrap';
import type { UserSettings } from '@/types/user-settings';

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

/** 启动存储快路径：data 就是 user-settings.json 的扁平键值对。 */
function createBootstrap(data: Record<string, unknown>): StartupStorageFileBootstrap {
  return {
    path: '/tmp/user-data/user-settings.json',
    status: 'ready',
    data: {
      [USER_SETTINGS_SCHEMA_VERSION_KEY]: USER_SETTINGS_SCHEMA_VERSION,
      ...data,
    },
    schemaVersion: USER_SETTINGS_SCHEMA_VERSION,
    error: null,
  };
}

/**
 * 启动存储不可用 ⇒ 走慢路径：`migrateUserSettingsStorage` + `loadUserSettings()`，
 * 后者读的是 `LazyStore.entries()`，也就是真正的 user-settings.json 文件内容。
 */
function createUnavailableBootstrap(): StartupStorageFileBootstrap {
  return {
    path: '/tmp/user-data/user-settings.json',
    status: 'error',
    data: null,
    schemaVersion: null,
    error: 'unavailable',
  };
}

describe('user settings folder tree toggle', () => {
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

  it('默认显示目录树（与 navigator.vue 的 ?? true 兜底一致）', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap({}));

    expect(userSettingsStore.userSettings.navigator.showFolderTree).toBe(true);
  });

  it('允许键集合从默认对象派生，含 navigator.showFolderTree', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap({}));

    const defaultSettings = userSettingsStore.userSettingsDefault as UserSettings;

    expect(defaultSettings).not.toBeNull();
    expect(buildAllowedUserSettingsStorageKeys(defaultSettings).has('navigator.showFolderTree')).toBe(true);
  });

  it('启动存储里存着 false 时，加载进内存仍然是 false', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap({ 'navigator.showFolderTree': false }));

    expect(userSettingsStore.userSettings.navigator.showFolderTree).toBe(false);
  });

  it('重启时从 user-settings.json 的条目读回 false', async () => {
    // 模拟「上一次运行把目录树关掉并落盘」，这次启动走慢路径读文件。
    storedValues.set(USER_SETTINGS_SCHEMA_VERSION_KEY, USER_SETTINGS_SCHEMA_VERSION);
    storedValues.set('navigator.showFolderTree', false);

    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUnavailableBootstrap());

    expect(userSettingsStore.userSettings.navigator.showFolderTree).toBe(false);
  });

  it('关闭目录树会落盘为 navigator.showFolderTree=false', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createBootstrap({}));
    await userSettingsStore.set('navigator.showFolderTree', false);

    expect(storedValues.get('navigator.showFolderTree')).toBe(false);
  });
});
