// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (2026-11-05)：全局搜索结果行的右键菜单回归测试。
//
// 背景：`perf: 收敛渲染与索引的常驻开销（资源优化批次 C）`（bb003f97）把搜索结果
// 从「导航器 FileBrowser（external 模式）」换成了轻量列表，右键菜单与
// 「打开所在目录」一起消失（用户反馈：搜索结果右键没有菜单弹出）。
//
// 这组测试锁住三件事：
//   1. 行上有 `@contextmenu` 通道（点了行能定位到「光标下的那一个条目」）；
//   2. 菜单打开时带的是那一行的条目，并且处于「搜索结果视图」语义；
//   3. 空白处右键不弹菜单（避免菜单带着上一行的条目弹出）。

import {
  describe, expect, it, vi, beforeEach,
} from 'vitest';
import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import type { DirEntry } from '@/types/dir-entry';

const {
  handleContextMenuActionMock,
  openNavigatorRevealEntryMock,
  revealEntriesContainingDirectoryMock,
  toastErrorMock,
} = vi.hoisted(() => ({
  handleContextMenuActionMock: vi.fn(),
  openNavigatorRevealEntryMock: vi.fn(),
  revealEntriesContainingDirectoryMock: vi.fn(),
  toastErrorMock: vi.fn(),
}));

// 只替换 `useI18n`，`createI18n` 必须保留 —— `src/localization/index.ts` 会用它。
vi.mock('vue-i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-i18n')>();

  return {
    ...actual,
    useI18n: () => ({
      t: (key: string) => key,
      locale: { value: 'ch' },
    }),
  };
});

vi.mock('vue-router', () => ({
  useRouter: () => ({
    push: vi.fn(),
    replace: vi.fn(),
  }),
}));

vi.mock('@/composables/use-dir-entry-actions', () => ({
  useDirEntryActions: () => ({ handleContextMenuAction: handleContextMenuActionMock }),
}));

vi.mock('@/utils/open-navigator-directory', () => ({
  openNavigatorRevealEntry: openNavigatorRevealEntryMock,
}));

vi.mock('@/utils/reveal-containing-directory', () => ({
  revealEntriesContainingDirectory: revealEntriesContainingDirectoryMock,
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: toastErrorMock,
    success: vi.fn(),
  },
}));

// FORK-MODIFICATION (2026-11-06)：结果行会走 `useNativePath()`，
// 它在 setup 里读 platform store —— 这里直接替换掉，省掉整套 Pinia 依赖。
vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({
    currentPlatform: { value: 'windows' },
    isWindows: true,
    isMacOS: false,
    isLinux: false,
    isUnix: false,
  }),
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => ({
    userSettings: {
      navigator: {
        listColumnFlexWeights: {},
        listColumnWidths: {},
        thumbnailSize: 'medium',
      },
      dateTime: {
        dateStyle: 'short',
        timeStyle: 'short',
      },
    },
    fontSizeOffset: 0,
  }),
}));

// reka-ui 的 ContextMenu 会 Portal 到 body，测试里换成普通 div。
vi.mock('@/components/ui/context-menu', () => ({
  ContextMenu: {
    name: 'ContextMenu',
    props: ['open'],
    emits: ['update:open'],
    template: '<div class="stub-context-menu"><slot /></div>',
  },
  ContextMenuTrigger: {
    name: 'ContextMenuTrigger',
    template: '<div class="stub-context-menu-trigger"><slot /></div>',
  },
  ContextMenuContent: {
    name: 'ContextMenuContent',
    template: '<div class="stub-context-menu-content"><slot /></div>',
  },
  ContextMenuItem: {
    name: 'ContextMenuItem',
    template: '<div class="stub-context-menu-item"><slot /></div>',
  },
  ContextMenuSeparator: {
    name: 'ContextMenuSeparator',
    template: '<hr class="stub-context-menu-separator" />',
  },
}));

vi.mock('@/modules/navigator/components/file-browser/file-browser-entry-icon.vue', () => ({
  default: {
    name: 'FileBrowserEntryIcon',
    props: ['entry', 'size'],
    template: '<span class="stub-entry-icon" />',
  },
}));

// 动作菜单本体由另一组测试覆盖；这里只需要「菜单有没有出现、带的是哪一行」。
vi.mock('@/modules/navigator/components/file-browser/file-browser-actions-menu.vue', () => ({
  default: {
    name: 'FileBrowserActionsMenu',
    props: [
      'selectedEntries',
      'menuItemComponent',
      'menuSeparatorComponent',
      'isSearchResultsView',
      'excludedActions',
    ],
    emits: ['action'],
    template: '<div class="stub-actions-menu" />',
  },
}));

function createEntry(path: string, overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    path,
    name: path.split(/[\\/]/).pop() ?? path,
    is_dir: false,
    is_file: true,
    size: 1024,
    modified_time: 1700000000,
    ...overrides,
  } as unknown as DirEntry;
}

const ENTRIES: DirEntry[] = [
  createEntry('C:\\Users\\tester\\Pictures\\alpha.png'),
  createEntry('C:\\Users\\tester\\Pictures\\beta.png'),
];

async function mountList() {
  const { default: GlobalSearchResultList } = await import('../global-search-result-list.vue');

  return mount(GlobalSearchResultList, {
    props: {
      entries: ENTRIES,
      selectedPaths: new Set<string>(),
      focusedPath: null,
    },
  });
}

/** 模拟 Radix 的 ContextMenuRoot 在右键后把 open 置为 true。 */
async function openContextMenu(wrapper: Awaited<ReturnType<typeof mountList>>) {
  await wrapper.findComponent({ name: 'ContextMenu' }).vm.$emit('update:open', true);
  await nextTick();
}

describe('global-search-result-list — context menu on search result rows', () => {
  beforeEach(() => {
    handleContextMenuActionMock.mockReset();
    openNavigatorRevealEntryMock.mockReset();
    revealEntriesContainingDirectoryMock.mockReset();
    toastErrorMock.mockReset();
  });

  it('opens a context menu carrying the right-clicked row entry', async () => {
    const wrapper = await mountList();

    expect(wrapper.find('.stub-actions-menu').exists()).toBe(false);

    const rows = wrapper.findAll('.global-search-result-list__row');
    expect(rows.length).toBe(2);

    await rows[1].trigger('contextmenu');
    await openContextMenu(wrapper);

    const actionsMenu = wrapper.findComponent({ name: 'FileBrowserActionsMenu' });
    expect(actionsMenu.exists()).toBe(true);
    expect(actionsMenu.props('selectedEntries')).toEqual([ENTRIES[1]]);
    expect(actionsMenu.props('isSearchResultsView')).toBe(true);

    wrapper.unmount();
  });

  it('does not open a menu when the right click lands on empty space', async () => {
    const wrapper = await mountList();

    await wrapper.find('.global-search-result-list').trigger('contextmenu');
    await openContextMenu(wrapper);

    expect(wrapper.find('.stub-actions-menu').exists()).toBe(false);

    wrapper.unmount();
  });

  it('routes "open containing directory" to the shared reveal helper', async () => {
    const wrapper = await mountList();

    await wrapper.findAll('.global-search-result-list__row')[0].trigger('contextmenu');
    await openContextMenu(wrapper);

    await wrapper.findComponent({ name: 'FileBrowserActionsMenu' }).vm.$emit('action', 'open-containing-directory');
    await nextTick();

    expect(revealEntriesContainingDirectoryMock).toHaveBeenCalledTimes(1);
    expect(revealEntriesContainingDirectoryMock.mock.calls[0][0]).toEqual([ENTRIES[0]]);

    wrapper.unmount();
  });

  it('routes "open containing directory in app" to the navigator reveal helper', async () => {
    const wrapper = await mountList();

    await wrapper.findAll('.global-search-result-list__row')[0].trigger('contextmenu');
    await openContextMenu(wrapper);

    await wrapper.findComponent({ name: 'FileBrowserActionsMenu' }).vm.$emit('action', 'open-containing-directory-in-app');
    await nextTick();

    expect(openNavigatorRevealEntryMock).toHaveBeenCalledTimes(1);
    expect(openNavigatorRevealEntryMock.mock.calls[0][1]).toBe(ENTRIES[0].path);

    wrapper.unmount();
  });

  it('routes "rename" to the panel-level rename dialog', async () => {
    const wrapper = await mountList();

    await wrapper.findAll('.global-search-result-list__row')[0].trigger('contextmenu');
    await openContextMenu(wrapper);

    await wrapper.findComponent({ name: 'FileBrowserActionsMenu' }).vm.$emit('action', 'rename');
    await nextTick();

    expect(wrapper.emitted('rename')).toEqual([[ENTRIES[0]]]);
    expect(handleContextMenuActionMock).not.toHaveBeenCalled();

    wrapper.unmount();
  });

  it('delegates every other action to the shared entry action executor', async () => {
    const wrapper = await mountList();

    await wrapper.findAll('.global-search-result-list__row')[0].trigger('contextmenu');
    await openContextMenu(wrapper);

    await wrapper.findComponent({ name: 'FileBrowserActionsMenu' }).vm.$emit('action', 'copy');
    await nextTick();

    expect(handleContextMenuActionMock).toHaveBeenCalledTimes(1);
    expect(handleContextMenuActionMock.mock.calls[0][0]).toBe('copy');
    expect(handleContextMenuActionMock.mock.calls[0][1]).toEqual([ENTRIES[0]]);

    wrapper.unmount();
  });
});
