// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { nextTick } from 'vue';
import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { USER_SETTINGS_SCHEMA_VERSION } from '@/stores/schemas/user-settings';
import {
  USER_SETTINGS_THEME_CHANGED_EVENT,
  useUserSettingsStore,
} from '@/stores/storage/user-settings';
import type { StartupStorageFileBootstrap } from '@/stores/storage/utils/startup-storage-bootstrap';
import type { InfusionPage, NavigatorFolderSettingsMap, Theme } from '@/types/user-settings';
import { normalizePathForComparison } from '@/utils/file-operation-paths';

type ThemeEventCallback = (event: { payload: { theme: Theme } }) => void;

const {
  emitMock,
  lazyStoreSaveMock,
  lazyStoreSetMock,
  lazyStoreGetMock,
  listenMock,
  themeEventCallbacks,
  webviewSetZoomMock,
} = vi.hoisted(() => ({
  emitMock: vi.fn(),
  lazyStoreSaveMock: vi.fn(),
  lazyStoreSetMock: vi.fn(),
  lazyStoreGetMock: vi.fn(),
  listenMock: vi.fn(),
  themeEventCallbacks: new Map<string, ThemeEventCallback>(),
  webviewSetZoomMock: vi.fn(),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    async save(): Promise<void> {
      await lazyStoreSaveMock();
    }

    async set(key: string, value: unknown): Promise<void> {
      await lazyStoreSetMock(key, value);
    }

    async get(key: string): Promise<unknown> {
      return lazyStoreGetMock(key);
    }

    async entries(): Promise<[string, unknown][]> {
      return [];
    }
  },
}));

// The startup path reads the system locale to pick a first-run language.
// Keep it inert here so these theme tests do not depend on the host locale,
// and report a non-Windows platform so the default-file-manager claim (which
// mutates shell registry state) is short-circuited away.
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

// FORK-MODIFICATION 2026-10-20：移植上游 8908e214 —— appDataDir 改从
// `@/utils/app-dirs` 取（后端统一解析目录），所以 mock 的目标也跟着换。
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

function createUserSettingsBootstrap(theme: Theme): StartupStorageFileBootstrap {
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

describe('user settings theme sync', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    document.documentElement.className = '';
    document.documentElement.style.cssText = '';
    emitMock.mockReset().mockResolvedValue(undefined);
    lazyStoreSaveMock.mockReset();
    lazyStoreSetMock.mockReset();
    // Default: no stored keys, so the startup path takes its first-run branch.
    lazyStoreGetMock.mockReset().mockResolvedValue(undefined);
    webviewSetZoomMock.mockReset();
    themeEventCallbacks.clear();
    listenMock.mockReset().mockImplementation(async (
      eventName: string,
      callback: ThemeEventCallback,
    ) => {
      themeEventCallbacks.set(eventName, callback);

      return vi.fn();
    });

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
      })),
    });
  });

  it('broadcasts theme changes to secondary windows', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));
    emitMock.mockClear();

    await userSettingsStore.set('theme', 'light');

    expect(emitMock).toHaveBeenCalledWith(USER_SETTINGS_THEME_CHANGED_EVENT, { theme: 'light' });
  });

  it('keeps folders-first sorting enabled for stored settings that predate the key', async () => {
    const userSettingsStore = useUserSettingsStore();

    // createUserSettingsBootstrap 只带 __schemaVersion 与 theme，
    // 也就是模拟「升级前就已存在、但没有 navigator.foldersFirst」的老用户。
    // 老用户必须读到 true：一旦这里变成 false，就等于把既有用户的默认
    // 行为从「文件夹置顶」静默翻转成「混合排序」。
    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    expect(userSettingsStore.userSettings.navigator.foldersFirst).toBe(true);
  });

  it('reads a persisted folders-first preference instead of the default', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));
    await userSettingsStore.set('navigator.foldersFirst', false);

    expect(userSettingsStore.userSettings.navigator.foldersFirst).toBe(false);
  });

  it('applies theme changes from other windows without writing them back', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));
    emitMock.mockClear();
    lazyStoreSaveMock.mockClear();
    lazyStoreSetMock.mockClear();

    themeEventCallbacks.get(USER_SETTINGS_THEME_CHANGED_EVENT)?.({
      payload: {
        theme: 'light',
      },
    });
    await nextTick();

    expect(userSettingsStore.userSettings.theme).toBe('light');
    expect(lazyStoreSetMock).not.toHaveBeenCalled();
    expect(lazyStoreSaveMock).not.toHaveBeenCalled();
    expect(emitMock).not.toHaveBeenCalled();
  });
});

describe('user settings 视觉效果 / 样式设置默认值（附图规格锁定）', () => {
  const infusionPages: InfusionPage[] = ['', 'home', 'navigator', 'dashboard', 'settings', 'extensions'];

  beforeEach(() => {
    setActivePinia(createPinia());
    emitMock.mockReset().mockResolvedValue(undefined);
    lazyStoreSaveMock.mockReset();
    lazyStoreSetMock.mockReset();
    lazyStoreGetMock.mockReset().mockResolvedValue(undefined);
    webviewSetZoomMock.mockReset();
    themeEventCallbacks.clear();
    listenMock.mockReset().mockImplementation(async (
      eventName: string,
      callback: ThemeEventCallback,
    ) => {
      themeEventCallbacks.set(eventName, callback);

      return vi.fn();
    });
  });

  // 规格来源：附图「视觉效果」页（2026-11-07）。
  // 叠加模糊 0px / 叠加不透明度 0% / 叠加噪点 0% —— 这三项是本次从 64|32 / 5 / 5 改过来的，
  // 其余各项本来就与附图一致，一并断言是为了防漂移。
  it('所有页面的叠加层默认参数与附图一致', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    const pages = userSettingsStore.userSettings.infusion.pages;

    for (const page of infusionPages) {
      expect(pages[page].blur).toBe(0);
      expect(pages[page].opacity).toBe(0);
      expect(pages[page].noise).toBe(0);
      expect(pages[page].mediaContrast).toBe(100);
      expect(pages[page].mediaBrightness).toBe(100);
      expect(pages[page].noiseScale).toBe(0.5);
      expect(pages[page].mixBlendMode).toBe('normal');
      expect(pages[page].blendTarget).toBe('ui');
    }
  });

  it('视觉效果的开关类默认值与附图一致', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    expect(userSettingsStore.userSettings.infusion.enabled).toBe(true);
    expect(userSettingsStore.userSettings.infusion.sameSettingsForAllPages).toBe(true);
    expect(userSettingsStore.userSettings.infusion.pauseVideoWhenIdle).toBe(true);
    expect(userSettingsStore.userSettings.homeBannerPauseVideoWhenIdle).toBe(true);
  });

  // 「重置为默认」读的是 userSettingsDefault 快照（设置页 resetVisualEffectsSection 直接克隆它回写），
  // 所以这份快照必须跟着默认值一起变 —— 否则重置按钮会把旧的 64/5/5 又写回去。
  it('默认快照带同样的叠加层参数，保证「重置为默认」回到这组值', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    const defaults = userSettingsStore.userSettingsDefault;

    expect(defaults).not.toBeNull();

    for (const page of infusionPages) {
      expect(defaults?.infusion.pages[page].blur).toBe(0);
      expect(defaults?.infusion.pages[page].opacity).toBe(0);
      expect(defaults?.infusion.pages[page].noise).toBe(0);
    }
  });

  it('样式设置（亮度 / 对比度 / 对话框背景模糊）默认值与附图一致', async () => {
    const userSettingsStore = useUserSettingsStore();

    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    expect(userSettingsStore.userSettings.visualFilters.brightness).toBe(100);
    expect(userSettingsStore.userSettings.visualFilters.contrast).toBe(100);
    expect(userSettingsStore.userSettings.visualFilters.dialogOverlayBlur).toBe(8);
  });
});

describe('user settings folder settings path lifecycle', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    emitMock.mockReset().mockResolvedValue(undefined);
    lazyStoreSaveMock.mockReset();
    lazyStoreSetMock.mockReset();
    // Default: no stored keys, so the startup path takes its first-run branch.
    lazyStoreGetMock.mockReset().mockResolvedValue(undefined);
    webviewSetZoomMock.mockReset();
    themeEventCallbacks.clear();
    listenMock.mockReset().mockImplementation(async (
      eventName: string,
      callback: ThemeEventCallback,
    ) => {
      themeEventCallbacks.set(eventName, callback);

      return vi.fn();
    });

    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
      })),
    });
  });

  it('remaps folder settings when a directory is renamed', async () => {
    const userSettingsStore = useUserSettingsStore();
    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    const folderSettings = {
      layout: 'grid' as const,
      listSortColumn: 'modified' as const,
      listSortDirection: 'desc' as const,
      gridSortColumn: 'name' as const,
      gridSortDirection: 'asc' as const,
      showHiddenFiles: true,
    };

    await userSettingsStore.set('navigator.folderSettings', {
      'C:/Users/aleks/Old': folderSettings,
      'C:/Users/aleks/Old/Nested': folderSettings,
    });

    await userSettingsStore.handlePathRenamed('C:/Users/aleks/Old', 'C:/Users/aleks/New');

    expect(userSettingsStore.userSettings.navigator.folderSettings).toEqual({
      [normalizePathForComparison('C:/Users/aleks/New')]: folderSettings,
      [normalizePathForComparison('C:/Users/aleks/New/Nested')]: folderSettings,
    });
  });

  it('removes folder settings when a directory is deleted', async () => {
    const userSettingsStore = useUserSettingsStore();
    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    const folderSettings = {
      layout: 'list' as const,
      listSortColumn: null,
      listSortDirection: 'asc' as const,
      gridSortColumn: 'name' as const,
      gridSortDirection: 'asc' as const,
      showHiddenFiles: false,
    };

    await userSettingsStore.set('navigator.folderSettings', {
      'C:/Users/aleks/Deleted': folderSettings,
      'C:/Users/aleks/Keep': folderSettings,
    });

    await userSettingsStore.handlePathsDeleted(['C:/Users/aleks/Deleted']);

    expect(userSettingsStore.userSettings.navigator.folderSettings).toEqual({
      [normalizePathForComparison('C:/Users/aleks/Keep')]: folderSettings,
    });
  });

  it('does not fill other folder snapshots from current global values when remapping', async () => {
    const userSettingsStore = useUserSettingsStore();
    await userSettingsStore.init(createUserSettingsBootstrap('dark'));

    const partialSettings = {
      layout: 'list',
    };
    const fullSettings = {
      layout: 'grid' as const,
      listSortColumn: 'modified' as const,
      listSortDirection: 'desc' as const,
      gridSortColumn: 'name' as const,
      gridSortDirection: 'asc' as const,
      showHiddenFiles: true,
    };

    await userSettingsStore.set('navigator.folderSettings', {
      'C:/Users/aleks/Old': fullSettings,
      'C:/Users/aleks/Keep': partialSettings,
    } as unknown as NavigatorFolderSettingsMap);

    await userSettingsStore.handlePathRenamed('C:/Users/aleks/Old', 'C:/Users/aleks/New');

    expect(userSettingsStore.userSettings.navigator.folderSettings).toEqual({
      [normalizePathForComparison('C:/Users/aleks/New')]: fullSettings,
      [normalizePathForComparison('C:/Users/aleks/Keep')]: partialSettings,
    });
  });
});

describe('user settings infusion blend settings', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    emitMock.mockReset().mockResolvedValue(undefined);
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

  function createBootstrap(data: Record<string, unknown>): StartupStorageFileBootstrap {
    return {
      path: '/tmp/user-data/user-settings.json',
      status: 'ready',
      data: {
        __schemaVersion: USER_SETTINGS_SCHEMA_VERSION,
        ...data,
      },
      schemaVersion: USER_SETTINGS_SCHEMA_VERSION,
      error: null,
    };
  }

  it('loads old settings without blend target and fill color unchanged', async () => {
    const userSettingsStore = useUserSettingsStore();
    await userSettingsStore.init(createBootstrap({
      'infusion.pages.global.mixBlendMode': 'overlay',
      'infusion.pages.global.opacity': 25,
      'infusion.pages.global.blur': 12,
    }));

    const globalPage = userSettingsStore.userSettings.infusion.pages[''];

    expect(globalPage.mixBlendMode).toBe('overlay');
    expect(globalPage.opacity).toBe(25);
    expect(globalPage.blur).toBe(12);
    expect(globalPage.blendTarget).toBe('ui');
    expect(globalPage.blendTint).toBe('#3b82f6');
    expect(userSettingsStore.userSettings.infusion.pages.home.mixBlendMode).toBe('normal');
  });

  it('loads saved blend target and fill color', async () => {
    const userSettingsStore = useUserSettingsStore();
    await userSettingsStore.init(createBootstrap({
      'infusion.pages.home.mixBlendMode': 'plus-lighter',
      'infusion.pages.home.blendTarget': 'fill',
      'infusion.pages.home.blendTint': '#EC4899',
    }));

    expect(userSettingsStore.userSettings.infusion.pages.home).toMatchObject({
      mixBlendMode: 'plus-lighter',
      blendTarget: 'fill',
      blendTint: '#ec4899',
    });
  });

  it('replaces invalid blend values with safe ones', async () => {
    const userSettingsStore = useUserSettingsStore();
    await userSettingsStore.init(createBootstrap({
      'infusion.pages.global.mixBlendMode': 'hue',
      'infusion.pages.global.blendTarget': 'everything',
      'infusion.pages.global.blendTint': 'url(javascript:alert(1))',
      'infusion.pages.navigator.mixBlendMode': 'pigment',
      'infusion.pages.navigator.blendTint': '#fff',
    }));

    expect(userSettingsStore.userSettings.infusion.pages['']).toMatchObject({
      mixBlendMode: 'color',
      blendTarget: 'ui',
      blendTint: '#3b82f6',
    });
    expect(userSettingsStore.userSettings.infusion.pages.navigator).toMatchObject({
      mixBlendMode: 'normal',
      blendTarget: 'ui',
      blendTint: '#3b82f6',
    });
  });
});
