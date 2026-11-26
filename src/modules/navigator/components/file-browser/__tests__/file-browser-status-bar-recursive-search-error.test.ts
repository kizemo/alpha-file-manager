// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 65（W-18）—— 「递归搜索不可用」必须真的出现在界面上。
//
// 这条测试是**界面级**的：它挂载状态栏组件本身，断言那条提示节点确实渲染、
// 且带着后端原文 —— 不是去 grep 源码里有没有那个字符串。
// 项目当前拿不到 webview console（W-19），DOM 级挂载是能站住的次强证据。

import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import FileBrowserStatusBar from '../file-browser-status-bar.vue';

// 部分 mock：只换掉 useI18n，保留 createI18n —— 状态栏的依赖链里
// 有人（stores/storage/user-settings）会 import createI18n，整模块替换会直接炸。
vi.mock('vue-i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-i18n')>();
  return {
    ...actual,
    useI18n: () => ({
      t: (key: string) => key,
    }),
  };
});

vi.mock('@/stores/runtime/dir-sizes', () => ({
  useDirSizesStore: () => ({
    getSize: () => null,
  }),
}));

vi.mock('@/stores/runtime/mcp', () => ({
  useMcpStore: () => ({
    isEnabled: false,
    status: { activeClients: 0 },
    refresh: vi.fn(),
  }),
}));

vi.mock('../composables/use-file-browser-context', () => ({
  useFileBrowserContext: () => ({
    contextMenu: { value: null },
    openNewItemDialog: vi.fn(),
    onContextMenuAction: vi.fn(),
    createLinksForEntries: vi.fn(),
    currentPath: { value: '' },
  }),
}));

const BACKEND_ERROR = '云盘目录递归检索失败（sfm://remote/conn-a/docs）：该连接当前没有勾选「加入全局检索」，索引层不可见。请先在设置的云盘连接里勾选它。';

const STUBS = {
  Button: true,
  Tooltip: true,
  TooltipTrigger: true,
  TooltipContent: true,
  Input: true,
  ScrollArea: true,
  Popover: true,
  PopoverAnchor: true,
  PopoverContent: true,
  DropdownMenu: true,
  DropdownMenuContent: true,
  DropdownMenuItem: true,
  DropdownMenuLabel: true,
  DropdownMenuSeparator: true,
  DropdownMenuTrigger: true,
  FileBrowserActionsMenu: true,
  FileBrowserExtensionMenuItems: true,
};

function mountStatusBar(recursiveSearchError?: string | null, hiddenByUser?: boolean) {
  return mount(FileBrowserStatusBar, {
    props: {
      dirContents: null,
      filteredCount: 0,
      ...(recursiveSearchError === undefined ? {} : { recursiveSearchError }),
      // 批次 67：只在调用方显式给出时才传，默认路径仍走组件自己的默认值。
      ...(hiddenByUser === undefined ? {} : { hiddenByUser }),
    },
    global: {
      stubs: STUBS,
      // 默认插槽要渲染出来，否则提示节点（挂在 TooltipTrigger 的插槽里）根本不会出现。
      renderStubDefaultSlot: true,
    },
  });
}

describe('file-browser-status-bar · 批次 65（W-18）递归搜索失败提示', () => {
  it('后端报错时状态栏出现可见提示，并带着后端原文', () => {
    const wrapper = mountStatusBar(BACKEND_ERROR);
    const notice = wrapper.find('[data-file-browser-recursive-search-error]');

    expect(notice.exists()).toBe(true);
    expect(notice.text()).toContain('fileBrowser.recursiveSearchUnavailableTitle');
    expect(notice.text()).toContain(BACKEND_ERROR);
  });

  it('没有失败时不出现任何提示节点', () => {
    const wrapper = mountStatusBar(null);

    expect(wrapper.find('[data-file-browser-recursive-search-error]').exists()).toBe(false);
  });

  it('不传该 prop 时同样不出现提示（默认值为空）', () => {
    const wrapper = mountStatusBar();

    expect(wrapper.find('[data-file-browser-recursive-search-error]').exists()).toBe(false);
  });
});

// FORK-MODIFICATION: 批次 67 —— 「隐藏状态栏」不能把递归检索失败一起隐藏。
//
// 批次 65 把提示安在状态栏里，于是 `hideStatusBar` 为真的 pane 又回到静默降级，
// 那条修复对这类用户完全失效。规则收在状态栏自己身上：
// 隐藏只对「无事发生」生效；有错误就必须露面。
//
// 为什么测这里：真正会踩坑的是「隐藏 + 有错」的组合，而 file-browser.vue 没有
// 现成的挂载夹具（依赖 fb / stores / DataSource）。规则搬进状态栏后，
// 这个组合可以在 DOM 层直接验证，而不是去 grep 源码字符串。
describe('file-browser-status-bar · 批次 67 隐藏状态栏时错误仍然可见', () => {
  it('隐藏 + 有错误：整条状态栏出现，错误节点带着后端原文', () => {
    const wrapper = mountStatusBar(BACKEND_ERROR, true);

    expect(wrapper.find('.file-browser-status-bar').exists()).toBe(true);
    const notice = wrapper.find('[data-file-browser-recursive-search-error]');
    expect(notice.exists()).toBe(true);
    expect(notice.text()).toContain(BACKEND_ERROR);
  });

  it('隐藏 + 无错误：整条状态栏不渲染（隐藏依然生效）', () => {
    const wrapper = mountStatusBar(null, true);

    expect(wrapper.find('.file-browser-status-bar').exists()).toBe(false);
    expect(wrapper.find('[data-file-browser-recursive-search-error]').exists()).toBe(false);
  });

  it('不隐藏 + 有错误：行为与批次 65 一致', () => {
    const wrapper = mountStatusBar(BACKEND_ERROR, false);

    expect(wrapper.find('.file-browser-status-bar').exists()).toBe(true);
    expect(wrapper.find('[data-file-browser-recursive-search-error]').exists()).toBe(true);
  });
});
