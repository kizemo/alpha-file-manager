// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 「打开文件所在目录」入口的回归测试。
//
// 背景（2026-11-02 真机验收发现）：
// 这个动作的**可见性规则**（utils/context-menu-action-visibility.ts：
// 仅 Windows + 仅 file）和**后端处理**（composables/use-file-browser-actions.ts
// → revealInExplorer → 后端 reveal_in_explorer）一直都在，
// 但文件列表右键菜单**从来没有渲染过这一项** ⇒ 原生 Explorer 定位通道在界面上不可达。
// 而目录树自 3.9.0 起只显示目录（use-file-tree.ts 过滤 is_dir === true），
// 树菜单里的同名项因此永远不显示（showOpenContainingDirectory 恒为 false）。
//
// 这条测试守住「菜单里真的有这一项、点了真的会 emit」，
// 避免以后再次退化成「规则和处理都在、界面里点不到」。

import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { ref } from 'vue';
import { createPinia } from 'pinia';
import FileBrowserActionsMenu from '../file-browser-actions-menu.vue';
import type { DirEntry } from '@/types/dir-entry';

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/localization', () => ({
  i18n: {
    global: {
      locale: { value: 'ch' },
      t: (k: string) => k,
    },
  },
  messages: {},
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({ currentPlatform: 'windows' }),
}));

vi.mock('@/stores/runtime/clipboard', () => ({
  useClipboardStore: () => ({
    hasItems: false,
    canPasteTo: () => false,
    setClipboard: vi.fn(),
  }),
}));

vi.mock('@/stores/storage/user-stats', () => ({
  useUserStatsStore: () => ({ isFavorite: () => false }),
}));

vi.mock('@/stores/runtime/shortcuts', () => ({
  useShortcutsStore: () => ({ getShortcutLabel: () => '' }),
}));

vi.mock('@/stores/storage/remote-connections', () => ({
  useRemoteConnectionsStore: () => ({
    connections: [],
    getConnection: () => undefined,
  }),
}));

vi.mock('@/modules/remote/utils/remote-transfer-targets', () => ({
  useRemoteTransferTargets: () => ({
    remotePanePath: ref(null),
    localPanePath: ref(null),
    transferPanePath: ref(null),
  }),
}));

vi.mock('@/composables/use-text-direction', () => ({
  useTextDirection: () => ({ inlineEndSide: 'left' }),
}));

vi.mock('../composables/use-file-browser-tags', () => ({
  useFileBrowserTags: () => ({
    availableTags: ref([]),
    getEntriesSharedTagIds: () => [],
    toggleTagForEntries: vi.fn(),
    createTagForEntries: vi.fn(),
    renameTag: vi.fn(),
    updateTagColor: vi.fn(),
    setTags: vi.fn(),
  }),
}));

const OPEN_CONTAINING_DIRECTORY_LABEL = 'fileBrowser.actions.openContainingDirectory';

const MenuItemStub = {
  name: 'MenuItemStub',
  template: '<div class="menu-item-stub" @click="$emit(\'select\')"><slot /></div>',
  emits: ['select'],
};

const MenuSeparatorStub = {
  name: 'MenuSeparatorStub',
  template: '<hr class="menu-separator-stub" />',
};

const PassThroughStub = {
  name: 'PassThroughStub',
  template: '<div><slot /></div>',
};

function createEntry(path: string, isDir: boolean): DirEntry {
  return {
    name: path.split(/[\\/]/).pop() ?? path,
    path,
    is_file: !isDir,
    is_dir: isDir,
    is_hidden: false,
    is_symlink: false,
    size: 0,
    item_count: null,
    created_time: 0,
    modified_time: 0,
    accessed_time: 0,
    ext: null,
    mime: null,
  };
}

function mountMenu(selectedEntries: DirEntry[]) {
  return mount(FileBrowserActionsMenu, {
    props: {
      selectedEntries,
      menuItemComponent: MenuItemStub,
      menuSeparatorComponent: MenuSeparatorStub,
    },
    global: {
      plugins: [createPinia()],
      stubs: {
        Button: PassThroughStub,
        Tooltip: PassThroughStub,
        TooltipTrigger: PassThroughStub,
        TooltipContent: PassThroughStub,
        TagSelector: PassThroughStub,
        DropdownMenu: PassThroughStub,
        DropdownMenuTrigger: PassThroughStub,
        DropdownMenuContent: PassThroughStub,
        DropdownMenuItem: PassThroughStub,
        DropdownMenuShortcut: PassThroughStub,
        FileBrowserNewSubmenu: PassThroughStub,
        FileBrowserOpenWithSubmenu: PassThroughStub,
        FileBrowserMoreOptionsSubmenu: PassThroughStub,
        FileBrowserArchiveSubmenu: PassThroughStub,
        FileBrowserTerminalSubmenu: PassThroughStub,
      },
    },
  });
}

function menuItemLabels(wrapper: ReturnType<typeof mountMenu>): string[] {
  return wrapper.findAll('span').map(node => node.text());
}

describe('file-browser-actions-menu — 打开文件所在目录', () => {
  it('renders the native reveal item for a selected file', () => {
    const wrapper = mountMenu([createEntry('C:/Users/test/link.lnk', false)]);

    expect(menuItemLabels(wrapper)).toContain(OPEN_CONTAINING_DIRECTORY_LABEL);
  });

  it('does not render the native reveal item for a selected directory', () => {
    const wrapper = mountMenu([createEntry('C:/Users/test/folder', true)]);

    expect(menuItemLabels(wrapper)).not.toContain(OPEN_CONTAINING_DIRECTORY_LABEL);
  });

  it('emits open-containing-directory when the item is selected', async () => {
    const wrapper = mountMenu([createEntry('C:/Users/test/link.lnk', false)]);

    const item = wrapper
      .findAllComponents(MenuItemStub)
      .find(node => node.text().includes(OPEN_CONTAINING_DIRECTORY_LABEL));

    expect(item).toBeDefined();
    await item!.trigger('click');

    expect(wrapper.emitted('action')).toEqual([['open-containing-directory']]);
  });
});
