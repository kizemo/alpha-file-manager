// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (2026-11-26)：全局搜索结果列表「方向键逐行移动」的 DOM 级回归测试（W-19 残留①）。
//
// 背景：批次 56 第一轮在真机上用 CDP 判 `focusMovesThroughList`，判据只比较两行的
// className 字符串 —— 而每一行渲染的类名本来就相同，于是「按了方向键」与「焦点确实
// 换了一行」在判据上无法区分。该证据没有判别力，W-19 的这一半一直空着。
//
// 这里把链条拆成两段钉死，任何一段断开都会红：
//   A. listbox 的 keydown → `emit('navigate', direction, extend)`，并且 `preventDefault()`；
//   B. 父级回传 `focusedPath` → 目标行挂上 `--focused`、该行 `scrollIntoView`、listbox 拿回焦点。
// 视图侧把 A 接到 B 的那几行绑定由文件末尾的源码守卫兜住
// （同一手法见 `src/modules/navigator/pages/__tests__/navigator-arrow-ownership-global-search.test.ts`）。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import type { DirEntry } from '@/types/dir-entry';

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

// 三行：够区分「移动到下一行」与「移动到首行/末行」，也够让「不在本组」的路径有意义。
const ENTRIES: DirEntry[] = [
  createEntry('C:\\Users\\tester\\Pictures\\alpha.png'),
  createEntry('C:\\Users\\tester\\Pictures\\beta.png'),
  createEntry('C:\\Users\\tester\\Pictures\\gamma.png'),
];

async function mountList(overrides: { focusedPath?: string | null } = {}) {
  const { default: GlobalSearchResultList } = await import('../global-search-result-list.vue');

  return mount(GlobalSearchResultList, {
    attachTo: document.body,
    props: {
      entries: ENTRIES,
      selectedPaths: new Set<string>(),
      focusedPath: null,
      ...overrides,
    },
  });
}

/**
 * 派发一个真实的 KeyboardEvent 并把它拿回来 —— `trigger()` 不暴露事件对象，
 * 而「有没有 preventDefault」正是这组测试要判的东西。
 */
function pressKey(element: Element, init: KeyboardEventInit): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    bubbles: true,
    cancelable: true,
    ...init,
  });

  element.dispatchEvent(event);

  return event;
}

/** 给某一行装 `scrollIntoView` —— jsdom 没有这个 API，组件里也有 `typeof` 守卫。 */
function stubRowScroll(row: { element: Element }) {
  const target = row.element as Element & { scrollIntoView?: (options?: ScrollIntoViewOptions) => void };
  const scrollIntoView = vi.fn();

  target.scrollIntoView = scrollIntoView;

  return scrollIntoView;
}

const FOCUSED_CLASS = 'global-search-result-list__row--focused';

describe('global-search-result-list — 方向键把焦点落到结果行上（W-19）', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it.each([
    ['ArrowDown', 'next'],
    ['ArrowUp', 'previous'],
    ['Home', 'first'],
    ['End', 'last'],
  ])('%s 发出 navigate("%s", false) 并吃掉默认行为', async (key, direction) => {
    const wrapper = await mountList();
    const listbox = wrapper.find('.global-search-result-list');

    const event = pressKey(listbox.element, { key });

    expect(wrapper.emitted('navigate')).toEqual([[direction, false]]);
    expect(event.defaultPrevented).toBe(true);

    wrapper.unmount();
  });

  it('Shift+方向键把 extend 标成 true（整段选中）', async () => {
    const wrapper = await mountList();
    const listbox = wrapper.find('.global-search-result-list');

    const event = pressKey(listbox.element, {
      key: 'ArrowDown',
      shiftKey: true,
    });

    expect(wrapper.emitted('navigate')).toEqual([['next', true]]);
    expect(event.defaultPrevented).toBe(true);

    wrapper.unmount();
  });

  it('Enter 发 activate 并吃掉默认行为', async () => {
    const wrapper = await mountList();
    const listbox = wrapper.find('.global-search-result-list');

    const event = pressKey(listbox.element, { key: 'Enter' });

    expect(wrapper.emitted('activate')).toHaveLength(1);
    expect(wrapper.emitted('navigate')).toBeUndefined();
    expect(event.defaultPrevented).toBe(true);

    wrapper.unmount();
  });

  it('无关按键既不 emit 也不 preventDefault（不吞掉别人的键）', async () => {
    const wrapper = await mountList();
    const listbox = wrapper.find('.global-search-result-list');

    const event = pressKey(listbox.element, { key: 'a' });

    // 只看组件自定义事件：VTU 会把原生 keydown 也记进 emitted()。
    expect(wrapper.emitted('navigate')).toBeUndefined();
    expect(wrapper.emitted('activate')).toBeUndefined();
    expect(event.defaultPrevented).toBe(false);

    wrapper.unmount();
  });

  it('focusedPath 回传后只有那一行挂 --focused，并滚动回可见区、listbox 拿回焦点', async () => {
    const wrapper = await mountList();
    const rows = wrapper.findAll('.global-search-result-list__row');
    const firstRowScroll = stubRowScroll(rows[0]);
    const secondRowScroll = stubRowScroll(rows[1]);

    await wrapper.setProps({ focusedPath: ENTRIES[1].path });
    await flushPromises();

    expect(rows[1].classes()).toContain(FOCUSED_CLASS);
    expect(rows[0].classes()).not.toContain(FOCUSED_CLASS);
    expect(rows[2].classes()).not.toContain(FOCUSED_CLASS);

    expect(secondRowScroll).toHaveBeenCalledTimes(1);
    expect(secondRowScroll).toHaveBeenCalledWith({ block: 'nearest' });
    expect(firstRowScroll).not.toHaveBeenCalled();

    expect(document.activeElement).toBe(wrapper.find('.global-search-result-list').element);

    wrapper.unmount();
  });

  it('focusedPath 落在别的盘符分组时不抢焦点、不滚动', async () => {
    const wrapper = await mountList();
    const rows = wrapper.findAll('.global-search-result-list__row');
    const rowScroll = stubRowScroll(rows[1]);
    const outside = document.createElement('button');

    document.body.appendChild(outside);
    outside.focus();

    await wrapper.setProps({ focusedPath: 'D:\\Other\\delta.png' });
    await flushPromises();

    expect(rowScroll).not.toHaveBeenCalled();
    expect(document.activeElement).toBe(outside);
    expect(wrapper.find(`.${FOCUSED_CLASS}`).exists()).toBe(false);

    wrapper.unmount();
  });

  it('focusedPath 为空时什么都不做', async () => {
    const wrapper = await mountList();
    const rowScroll = stubRowScroll(wrapper.findAll('.global-search-result-list__row')[0]);

    await wrapper.setProps({ focusedPath: null });
    await flushPromises();

    expect(rowScroll).not.toHaveBeenCalled();
    expect(wrapper.find(`.${FOCUSED_CLASS}`).exists()).toBe(false);

    wrapper.unmount();
  });
});

// A→B 的接缝：组件 emit 出去之后，由视图把方向折算成新的 focusedPath 再传回来。
// 挂整个 `global-search-view.vue` 代价过高（面板 + 路由 + 索引 invoke），
// 这里只守「绑定还在、回写还在」——与批次 56 的归属守卫同一档。
describe('global-search-view — navigate 与 focusedPath 的接缝', () => {
  // 本项目 vitest 环境里 import.meta.url 不是 file: scheme，只能走 process.cwd()。
  const viewSource = readFileSync(
    resolve(process.cwd(), 'src/modules/global-search/global-search-view.vue'),
    'utf8',
  );

  it('listbox 的 navigate/activate 与 focusedPath 双向绑在视图里', () => {
    expect(viewSource).toContain(':focused-path="focusedPath"');
    expect(viewSource).toContain('@navigate="handleResultNavigate"');
    expect(viewSource).toContain('@activate="handleResultActivate"');
  });

  it('handleResultNavigate 把方向交给 navigateResults', () => {
    expect(viewSource).toMatch(/function handleResultNavigate\(direction: GlobalSearchNavigateDirection, extend: boolean\) \{\s*navigateResults\(direction, extend\);/);
  });

  it('focusEntryAt / extendSelectionTo 会把 focusedPath 回写成目标条目', () => {
    const assignments = viewSource.match(/focusedPath\.value = entry\.path;/g) ?? [];

    // 两条导航路径各一处：点选一条、Shift 扩选一条。
    expect(assignments.length).toBeGreaterThanOrEqual(2);
  });
});
