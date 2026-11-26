// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (批次 55)：云盘条目在全局搜索界面上的「可见性」回归测试。
//
// 背景：批次 49–54 的证据链停在「引擎返回了云盘靶子」（MCP 六档实测，
// limit=50 的窗口是 24 本地 + 26 云盘，靶子在第 27 位），还差最后一段 ——
// 引擎的结果**真的进了界面**吗？这组测试把最后一段锁死：
//
//   1. 客户端二次过滤（显示文件 / 显示目录）只按条目类型判断，不按路径判断
//      ⇒ 云盘条目（`sfm://remote/...`）不会被前端悄悄丢掉；
//   2. 按根目录分组不会把云盘条目分丢 ⇒ 它仍然落在某一组里；
//   3. 结果列表组件真的把云盘条目的**文件名渲染进 DOM**。
//
// 靶子沿用批次 50–54 的同一条真实云盘文件：
// `Midjourney 使用心得+关键词分析.mp4`（阿里云盘连接 646dacdb…）。
//
// 本组**不覆盖**：全局搜索 store 内部的合并层（priority + indexed 去重排序）。
// 那段逻辑不按路径过滤（见 stores/runtime/global-search.ts 的
// mergeAndDeduplicateResults），引擎侧的合并窗口由 Rust 单测锁住
// （src-tauri/src/global_search/query.rs 的 w15 / w16 组）。

import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import type { DirEntry } from '@/types/dir-entry';
import {
  filterGlobalSearchResultsByKind,
  groupGlobalSearchResults,
} from '../global-search-result-rows';

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
  useDirEntryActions: () => ({ handleContextMenuAction: vi.fn() }),
}));

vi.mock('@/utils/open-navigator-directory', () => ({
  openNavigatorRevealEntry: vi.fn(),
}));

vi.mock('@/utils/reveal-containing-directory', () => ({
  revealEntriesContainingDirectory: vi.fn(),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: vi.fn(),
    success: vi.fn(),
  },
}));

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

/** 批次 50–54 的真实云盘靶子（阿里云盘连接 646dacdb…）。 */
const CLOUD_FILE_PATH = 'sfm://remote/646dacdb6ff34af4b26f88dd802baf02/Midjourney 使用心得+关键词分析.mp4';
const CLOUD_FILE_NAME = 'Midjourney 使用心得+关键词分析.mp4';

function createEntry(path: string, overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    path,
    name: path.split(/[\\/]/).pop() ?? path,
    is_dir: false,
    is_file: true,
    size: 1024,
    modified_time: 1680686620768,
    ...overrides,
  } as unknown as DirEntry;
}

const CLOUD_FILE: DirEntry = createEntry(CLOUD_FILE_PATH, { name: CLOUD_FILE_NAME });
const CLOUD_DIR: DirEntry = createEntry('sfm://remote/646dacdb6ff34af4b26f88dd802baf02/视频', {
  name: '视频',
  is_dir: true,
  is_file: false,
});
const LOCAL_FILE: DirEntry = createEntry('F:\\资料\\分析报告.pdf');

async function mountList(entries: DirEntry[]) {
  const { default: GlobalSearchResultList } = await import('../global-search-result-list.vue');

  return mount(GlobalSearchResultList, {
    props: {
      entries,
      selectedPaths: new Set<string>(),
      focusedPath: null,
    },
  });
}

describe('global-search — 云盘条目的界面可见性（批次 55）', () => {
  it('客户端二次过滤只按条目类型判断，不按路径判断（云盘条目不被丢掉）', () => {
    const entries = [LOCAL_FILE, CLOUD_FILE, CLOUD_DIR];

    const kept = filterGlobalSearchResultsByKind(entries, {
      includeFiles: true,
      includeDirectories: true,
    });

    expect(kept).toHaveLength(3);
    expect(kept.map(entry => entry.path)).toContain(CLOUD_FILE_PATH);

    const withoutFiles = filterGlobalSearchResultsByKind(entries, {
      includeFiles: false,
      includeDirectories: true,
    });

    expect(withoutFiles.map(entry => entry.path)).not.toContain(CLOUD_FILE_PATH);
    expect(withoutFiles.map(entry => entry.path)).toContain(CLOUD_DIR.path);

    const withoutDirs = filterGlobalSearchResultsByKind(entries, {
      includeFiles: true,
      includeDirectories: false,
    });

    expect(withoutDirs.map(entry => entry.path)).toContain(CLOUD_FILE_PATH);
    expect(withoutDirs.map(entry => entry.path)).not.toContain(CLOUD_DIR.path);
  });

  it('按根目录分组时云盘条目仍然留在结果里', () => {
    const groups = groupGlobalSearchResults([LOCAL_FILE, CLOUD_FILE]);

    const cloudGroup = groups.find(group => group.entries.some(entry => entry.path === CLOUD_FILE_PATH));

    expect(cloudGroup).toBeDefined();
    expect(cloudGroup?.driveRoot).toBe('/sfm:');
    expect(groups).toHaveLength(2);
  });

  it('结果列表组件把云盘条目的文件名渲染进 DOM', async () => {
    const wrapper = await mountList([LOCAL_FILE, CLOUD_FILE]);

    const rows = wrapper.findAll('.global-search-result-list__row');
    expect(rows).toHaveLength(2);
    expect(wrapper.text()).toContain(CLOUD_FILE_NAME);

    wrapper.unmount();
  });

  it('只挂云盘条目时同样渲染出来（不会被当成「无结果」挡掉）', async () => {
    const wrapper = await mountList([CLOUD_FILE]);

    expect(wrapper.findAll('.global-search-result-list__row')).toHaveLength(1);
    expect(wrapper.text()).toContain(CLOUD_FILE_NAME);

    wrapper.unmount();
  });
});
