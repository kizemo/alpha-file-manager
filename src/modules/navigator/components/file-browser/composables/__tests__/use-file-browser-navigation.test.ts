// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { defineComponent, isReactive, nextTick, ref } from 'vue';
import { flushPromises, mount } from '@vue/test-utils';
import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import type { DirContents, DirEntry } from '@/types/dir-entry';
import type { Tab } from '@/types/workspaces';

const {
  invokeMock,
  listenMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: listenMock,
}));

vi.mock('@tauri-apps/api/path', () => ({
  homeDir: vi.fn().mockResolvedValue('C:/Users/aleks'),
}));

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({
    close: vi.fn(),
  }),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    async save(): Promise<void> {}

    async get(): Promise<undefined> {
      return undefined;
    }

    async set(): Promise<void> {}
  },
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/router', () => ({
  default: {
    push: vi.fn(),
  },
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    custom: vi.fn(),
  },
  ToastStatic: {},
}));

vi.mock('@/stores/runtime/navigator', () => ({
  useNavigatorStore: () => ({
    updateInfoPanel: vi.fn(),
  }),
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    userPaths: {
      homeDir: 'C:/Users/aleks',
    },
    customPaths: {
      appUserDataWorkspacesPath: 'mock/user-workspaces.json',
    },
  }),
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => ({
    userSettings: {
      navigator: {},
    },
  }),
}));

vi.mock('@/stores/storage/user-stats', () => ({
  useUserStatsStore: () => ({
    recordItemOpen: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/dir-sizes', () => ({
  useDirSizesStore: () => ({
    requestSizesBatch: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/navigator-icons', () => ({
  useNavigatorIconsStore: () => ({
    prefetchForDirectoryEntries: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/link-metadata', () => ({
  useLinkMetadataStore: () => ({
    primeMetadata: vi.fn(),
    invalidate: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/item-counts', () => ({
  useItemCountsStore: () => ({
    primeItemCounts: vi.fn(),
    invalidate: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/status-center', () => ({
  useStatusCenterStore: () => ({
    activeOperations: [],
  }),
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({
    currentPlatform: 'windows',
  }),
}));

vi.mock('@/stores/storage/remote-connections', () => ({
  useRemoteConnectionsStore: () => ({
    getConnection: vi.fn(),
  }),
}));

vi.mock('@/utils/open-path-default', () => ({
  openPathDefault: vi.fn(),
}));

vi.mock('@/utils/resolve-navigable-item-target', () => ({
  resolveNavigableItemTarget: vi.fn(),
}));

vi.mock('@/modules/home/composables/use-drives', async () => {
  const { ref: vueRef } = await import('vue');

  return {
    sharedDrives: vueRef([]),
  };
});

vi.mock('@/modules/navigator/utils/resolve-navigator-folder-settings', () => ({
  resolveNavigatorFolderSettings: () => ({
    layout: {
      type: {
        name: 'grid',
      },
    },
  }),
}));

vi.mock('@/modules/navigator/components/file-browser/utils/file-browser-sort-columns', () => ({
  getFileBrowserSortReadDirOptions: () => ({
    includeShortcutTargets: false,
    includeHardLinkCounts: false,
    includeItemCounts: false,
    includeHiddenItemCounts: false,
  }),
}));

import { useWorkspacesStore } from '@/stores/storage/workspaces';
import { useFileBrowserNavigation } from '../use-file-browser-navigation';
import { useFileBrowserLifecycle } from '../use-file-browser-lifecycle';

type NavigationApi = ReturnType<typeof useFileBrowserNavigation>;

function createTab(id: string, path: string): Tab {
  return {
    id,
    name: path,
    path,
    type: 'directory',
    paneWidth: 100,
    filterQuery: '',
    dirEntries: [],
    selectedDirEntries: [],
  };
}

function createDirContents(path: string, entryNames: string[]): DirContents {
  return {
    path,
    entries: entryNames.map(name => ({
      name,
      ext: 'txt',
      path: `${path}/${name}`,
      size: 0,
      item_count: null,
      modified_time: 0,
      accessed_time: 0,
      created_time: 0,
      mime: 'text/plain',
      is_file: true,
      is_dir: false,
      is_symlink: false,
      is_hidden: false,
    })),
    total_count: entryNames.length,
    dir_count: 0,
    file_count: entryNames.length,
    opened_directory_times: {
      modified_time: 0,
      accessed_time: 0,
      created_time: 0,
    },
  };
}

/**
 * 把「导航 composable」与「生命周期 composable」装进同一个组件：
 * 生命周期盯着 {id, path}，标签页一变就按新路径重读 —— 这正是切标签页的读盘入口。
 */
function createNavigationHarness(initialTab: Tab) {
  const tabRef = ref<Tab | undefined>(initialTab);
  const holder: { api: NavigationApi | null } = { api: null };

  mount(defineComponent({
    setup() {
      const navigation = useFileBrowserNavigation(() => tabRef.value);

      useFileBrowserLifecycle({
        tabRef,
        currentPath: navigation.currentPath,
        readDir: (path: string, addToHistory: boolean) => navigation.readDir(path, addToHistory),
        init: () => {
          void navigation.init();
        },
      });

      holder.api = navigation;

      return {};
    },
    template: '<div />',
  }));

  return {
    tabRef,
    navigation: () => holder.api as NavigationApi,
  };
}

describe('useFileBrowserNavigation directory data', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    listenMock.mockReset();
    listenMock.mockResolvedValue(() => {});
    invokeMock.mockImplementation((command: string, payload?: { path?: string }) => {
      if (command === 'read_dir') {
        return Promise.resolve(createDirContents(payload?.path ?? '', ['file.txt']));
      }

      if (command === 'read_dir_with_timeout') {
        return Promise.resolve(createDirContents(payload?.path ?? '', []));
      }

      if (command === 'get_dir_entry_with_timeout') {
        return Promise.resolve({ path: payload?.path ?? '' });
      }

      return Promise.resolve(null);
    });
  });

  it('reloads the directory by path when the active tab changes', async () => {
    const firstTab = createTab('tab-a', 'C:/Users/aleks/Projects');
    const secondTab = createTab('tab-b', 'D:/Documents');
    const { tabRef, navigation } = createNavigationHarness(firstTab);

    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('read_dir', expect.objectContaining({
      path: 'C:/Users/aleks/Projects',
    }));

    tabRef.value = secondTab;
    await nextTick();
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('read_dir', expect.objectContaining({
      path: 'D:/Documents',
    }));
    expect(navigation().currentPath.value).toBe('D:/Documents');
    expect(secondTab.dirEntries).toEqual([]);
  });

  it('reloads the previous and the next directory from the history stack', async () => {
    const { navigation } = createNavigationHarness(createTab('tab-a', 'C:/Users/aleks/Projects'));
    await flushPromises();

    await navigation().navigateToPath('D:/Documents');
    await flushPromises();

    expect(navigation().canGoBack.value).toBe(true);

    await navigation().goBack();
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('read_dir', expect.objectContaining({
      path: 'C:/Users/aleks/Projects',
    }));

    await navigation().goForward();
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('read_dir', expect.objectContaining({
      path: 'D:/Documents',
    }));
  });

  it('keeps the directory entries out of the deep reactive proxy tree', async () => {
    const { navigation } = createNavigationHarness(createTab('tab-a', 'C:/Users/aleks/Projects'));
    await flushPromises();

    const contents = navigation().dirContents.value;

    expect(contents).not.toBeNull();
    expect(contents?.entries).toHaveLength(1);
    expect(isReactive(contents)).toBe(false);
    expect(isReactive(contents?.entries)).toBe(false);
    expect(isReactive(contents?.entries[0])).toBe(false);
  });

  it('caches the read directory by path instead of keeping it on the tab', async () => {
    const tab = createTab('tab-a', 'C:/Users/aleks/Projects');
    const { navigation } = createNavigationHarness(tab);
    await flushPromises();

    const workspacesStore = useWorkspacesStore();

    expect(tab.dirEntries).toEqual([]);
    expect(tab.selectedDirEntries).toEqual([]);
    expect(workspacesStore.getCachedDirectoryEntries('C:/Users/aleks/Projects')).toHaveLength(1);
    expect(navigation().dirContents.value?.entries).toHaveLength(1);
  });

  it('reports the read failure and drops the stale cache entry', async () => {
    const { navigation } = createNavigationHarness(createTab('tab-a', 'C:/Users/aleks/Projects'));
    await flushPromises();

    const workspacesStore = useWorkspacesStore();

    workspacesStore.cacheDirectoryEntries('E:/Missing', [
      { name: 'stale.txt' } as DirEntry,
    ]);

    invokeMock.mockImplementation((command: string) => {
      if (command === 'read_dir') {
        return Promise.reject(new Error('Directory is gone'));
      }

      return Promise.resolve(null);
    });

    const consoleErrorSpy = vi.spyOn(console, 'error').mockImplementation(() => {});

    await navigation().readDir('E:/Missing', false);
    await flushPromises();

    expect(navigation().error.value).toContain('Directory is gone');
    expect(workspacesStore.getCachedDirectoryEntries('E:/Missing')).toBeNull();

    consoleErrorSpy.mockRestore();
  });
});
