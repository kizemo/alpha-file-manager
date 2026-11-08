// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-08：回归测试。
// 背景：设置页「远程」是整页分类（settings/ui/view.vue 的 categoryComponentMap 渲染），
// 在 allSections 里没有任何 category === 'remote' 的可搜索小节。
// 旧实现的 init() 兜底用 sections 判定「当前页签是否可用」，
// 于是 currentTab === 'remote' 会被误回落成 'general'，
// 导致首页「网盘 / 远程连接」入口跳转后落在「常规」页。
// 本测试锁定正确判据：导航页签列表（tabs）。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';

const { userSettingsMock } = vi.hoisted(() => ({
  userSettingsMock: {
    userSettings: { settingsCurrentTab: 'general' } as { settingsCurrentTab: string },
    setUserSettingsStorage: vi.fn(),
  },
}));

vi.mock('@/localization', () => ({
  i18n: {
    global: {
      t: (key: string) => key,
      locale: { value: 'en' },
    },
  },
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => userSettingsMock,
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({ isWindows: true }),
}));

vi.mock('@/stores/runtime/shortcuts', () => ({
  useShortcutsStore: () => ({}),
  formatShortcutKeys: () => '',
}));

vi.mock('@/stores/runtime/global-shortcuts', () => ({
  useGlobalShortcutsStore: () => ({}),
}));

vi.mock('@/stores/runtime/extensions', () => ({
  useExtensionsStore: () => ({}),
}));

import {
  resolveSettingsTabAfterInit,
  useSettingsStore,
} from '@/stores/runtime/settings';

describe('settings store 页签可用性判定', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    userSettingsMock.userSettings.settingsCurrentTab = 'general';
    userSettingsMock.setUserSettingsStorage.mockReset();
  });

  it('「远程」是导航页签之一，且不会被 init() 兜底回落成「常规」', () => {
    const store = useSettingsStore();
    const tabNames = store.tabs.map(tab => tab.name);

    expect(tabNames).toContain('remote');
    expect(resolveSettingsTabAfterInit('remote', tabNames)).toBe('remote');
  });

  it('非法页签仍然回落成「常规」', () => {
    const store = useSettingsStore();
    const tabNames = store.tabs.map(tab => tab.name);

    expect(resolveSettingsTabAfterInit('not-a-tab', tabNames)).toBe('general');
  });

  it('init() 之后当前页签仍是「远程」（首页入口深链依赖这一点）', async () => {
    userSettingsMock.userSettings.settingsCurrentTab = 'remote';
    const store = useSettingsStore();

    await store.init();

    expect(store.currentTab).toBe('remote');
    expect(userSettingsMock.userSettings.settingsCurrentTab).toBe('remote');
  });
});
