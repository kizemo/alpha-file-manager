// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * 「设置 → 搜索」页的**云盘索引区块**（组件级回归网，批次 44 / 台账 O-3）。
 *
 * 这一块在批次 43 改过模板逻辑（按连接判文案、未勾选不渲染重建按钮），
 * 但一直没有组件级测试 —— 只有「整仓门禁 + 真机截图」两层证据。
 * 本文件把当时手工核对的那几条口径写成自动检查：
 *
 * 1. **分组**：按网盘类型分组渲染；没有连接的网盘类型也显示「暂无连接」；
 * 2. **逐行**：每条连接一行，复选框的勾选态**跟数据走**（`includeInGlobalSearch`）；
 * 3. **未勾选不渲染重建按钮**：对它点重建只会重建别的连接，摆着是误导；
 * 4. **「正在索引云盘」只给本轮集合里的连接**：
 *    批次 43 的装机反馈「勾一个百度，其它两个也显示正在索引云盘」就是这么来的；
 * 5. **取消勾选立刻生效**：点掉复选框 ⇒ 那一行的重建按钮**当场消失**，
 *    并且落库写成 `false`（不依赖「下次重建时顺手清掉」）。
 *
 * 桩的范围：
 * - Tauri 运行时（`invoke` / `LazyStore` / `listen`）—— 没有真机；
 * - **云盘索引控制器**（`useRemoteGlobalSearchIndex`）—— 它的行为另有专门单测
 *   （`src/composables/__tests__/use-remote-global-search-index.test.ts`），
 *   这里只桩掉「当前状态」，好把界面摆成想要的局面；
 * - `buildRemoteNetdiskGroups` **用真的** —— 分组规则本身就是被测对象之一。
 */

import {
  afterEach, beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { ref } from 'vue';
import GlobalSearchSettings from '@/modules/settings/ui/categories/search/global-search.vue';
import { useGlobalSearchStore } from '@/stores/runtime/global-search';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import type { RemoteConnection } from '@/types/remote';

const { invokeMock, lazyStoreSaveMock, storageContents } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  lazyStoreSaveMock: vi.fn(),
  storageContents: new Map<string, unknown>(),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    async save(): Promise<void> {
      await lazyStoreSaveMock();
    }

    async get<T>(key: string): Promise<T | undefined> {
      return storageContents.get(key) as T | undefined;
    }

    async set(key: string, value: unknown): Promise<void> {
      storageContents.set(key, value);
    }
  },
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {
    // 测试环境里没有 Tauri 运行时，退订函数返回空实现即可。
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: vi.fn(),
}));

// reka-ui 的 `TooltipRoot` 在测试环境里不渲染子树（只吐一条 warning），
// 换成「就地渲染」的替身，行内那颗「重建」按钮才查得到。
vi.mock('@/components/ui/tooltip', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/components/ui/tooltip')>();

  return {
    ...actual,
    Tooltip: { template: '<div><slot /></div>' },
    TooltipTrigger: { template: '<span><slot /></span>' },
    TooltipContent: { template: '<span><slot /></span>' },
  };
});

// 路由模块在测试环境里装载不了（`routes` 来自构建期生成的虚拟模块），
// 而设置页的依赖链（`@/modules/settings` → `open-settings`）会摸到它。
vi.mock('@/router', () => ({
  default: {
    push: vi.fn(),
    replace: vi.fn(),
    resolve: vi.fn(() => ({ href: '/' })),
    currentRoute: {
      value: {
        path: '/',
        name: 'home',
      },
    },
  },
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataDir: '/tmp/user-data',
      appUserDataRemoteConnectionsPath: '/tmp/user-data/user-remote-connections.json',
    },
  }),
}));

vi.mock('@/modules/home/composables/use-drives', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/modules/home/composables/use-drives')>();
  const { ref: vueRef } = await import('vue');

  return {
    ...actual,
    useDrives: () => ({ drives: vueRef([]) }),
    // 全局检索 store 在 watch 它 —— 给个空的，别让它去摸 Tauri。
    sharedDrives: vueRef([]),
  };
});

/**
 * 云盘索引控制器的「当前状态」。
 *
 * 每个用例自己摆；组件通过 composable 拿到的是这里的 ref，
 * 所以改完 `await nextTick()` 界面就会跟着变。
 */
const controller = {
  globalSearchConnectionIds: ref<string[]>([]),
  isRebuilding: ref(false),
  indexTargetConnectionIds: ref<string[]>([]),
  lastIndexedItemCount: ref<number | null>(null),
  triggerRebuild: vi.fn(),
  markReady: vi.fn(),
};

vi.mock('@/composables/use-remote-global-search-index', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/composables/use-remote-global-search-index')>();

  return {
    ...actual,
    // 只桩掉「控制器」，`buildRemoteNetdiskGroups` 用真的。
    useRemoteGlobalSearchIndex: () => ({
      globalSearchConnectionIds: controller.globalSearchConnectionIds,
      isRebuilding: controller.isRebuilding,
      indexTargetConnectionIds: controller.indexTargetConnectionIds,
      lastIndexedItemCount: controller.lastIndexedItemCount,
      triggerRebuild: controller.triggerRebuild,
      markReady: controller.markReady,
    }),
  };
});

const BAIDU_ID = 'baidu-conn-1';
const QUARK_ID = 'quark-conn-1';
const ALIYUN_ID = 'aliyun-conn-1';

// jsdom 没有 `matchMedia`，而用户设置 store 初始化主题时会摸它。
if (typeof window !== 'undefined' && typeof window.matchMedia !== 'function') {
  Object.defineProperty(window, 'matchMedia', {
    writable: true,
    value: (query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    }),
  });
}

function makeConnection(
  id: string,
  name: string,
  protocol: RemoteConnection['protocol'],
  includeInGlobalSearch: boolean,
): RemoteConnection {
  return {
    id,
    name,
    protocol,
    host: 'example.invalid',
    port: 443,
    username: '',
    initialPath: '/',
    proxy: { kind: 'direct' },
    includeInGlobalSearch,
  };
}

/** i18n 打回键名，断言直接对键，不受翻译影响。 */
function createI18nStubPlugin() {
  return createI18n({
    legacy: false,
    locale: 'en',
    fallbackWarn: false,
    missingWarn: false,
    messages: {},
  });
}

/**
 * 复选框当前是不是勾上了。
 *
 * 原生 `<input type="checkbox">` 看 `checked`；reka-ui 的复选框是个
 * `role="checkbox"` 的按钮，看 `aria-checked`。两种都判得动。
 */
function isChecked(element: Element): boolean {
  if (element instanceof HTMLInputElement) {
    return element.checked;
  }

  return element.getAttribute('aria-checked') === 'true';
}

async function mountGlobalSearch() {
  const wrapper = mount(GlobalSearchSettings, {
    global: {
      plugins: [createI18nStubPlugin()],
    },
  });

  // 等 onMounted 那条链走完（refreshStatus → init → markReady）。
  await flushPromises();

  return wrapper;
}

describe('设置 → 搜索：云盘索引区块', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    invokeMock.mockResolvedValue({});
    lazyStoreSaveMock.mockReset();
    storageContents.clear();

    controller.globalSearchConnectionIds.value = [BAIDU_ID, QUARK_ID];
    controller.isRebuilding.value = false;
    controller.indexTargetConnectionIds.value = [];
    controller.lastIndexedItemCount.value = null;
    controller.triggerRebuild.mockReset();
    controller.markReady.mockReset();

    // 连接表：百度 + 夸克已勾选，阿里未勾选。
    storageContents.set('connections', [
      makeConnection(BAIDU_ID, '百度网盘 A', 'baidu_pan', true),
      makeConnection(QUARK_ID, '夸克网盘 A', 'quark_pan', true),
      makeConnection(ALIYUN_ID, '阿里云盘 A', 'aliyun_pan', false),
    ]);
  });

  afterEach(() => {
    // 状态轮询有定时器，测试结束必须停掉，否则 vitest 挂着不退。
    useGlobalSearchStore().stopStatusPolling();
  });

  it('按网盘类型分组渲染；三条连接各自成行，复选框勾选态跟数据走', async () => {
    const wrapper = await mountGlobalSearch();

    expect(wrapper.find('[data-test="remote-index-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="remote-index-group-baidu_pan"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="remote-index-group-quark_pan"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="remote-index-group-aliyun_pan"]').exists()).toBe(true);

    expect(wrapper.find(`[data-test="remote-index-item-${BAIDU_ID}"]`).exists()).toBe(true);
    expect(wrapper.find(`[data-test="remote-index-item-${QUARK_ID}"]`).exists()).toBe(true);
    expect(wrapper.find(`[data-test="remote-index-item-${ALIYUN_ID}"]`).exists()).toBe(true);

    const baiduCheckbox = wrapper.find(`[data-test="remote-index-checkbox-${BAIDU_ID}"]`);
    const aliyunCheckbox = wrapper.find(`[data-test="remote-index-checkbox-${ALIYUN_ID}"]`);

    expect(baiduCheckbox.exists()).toBe(true);
    expect(isChecked(baiduCheckbox.element)).toBe(true);
    expect(isChecked(aliyunCheckbox.element)).toBe(false);
  });

  it('未勾选的那一行不渲染重建按钮（对它点重建只会重建别的连接）', async () => {
    const wrapper = await mountGlobalSearch();

    const baiduRow = wrapper.find(`[data-test="remote-index-item-${BAIDU_ID}"]`);
    const quarkRow = wrapper.find(`[data-test="remote-index-item-${QUARK_ID}"]`);
    const aliyunRow = wrapper.find(`[data-test="remote-index-item-${ALIYUN_ID}"]`);

    expect(baiduRow.find('[data-test="rebuild-remote-index-item"]').exists()).toBe(true);
    expect(quarkRow.find('[data-test="rebuild-remote-index-item"]').exists()).toBe(true);
    expect(aliyunRow.find('[data-test="rebuild-remote-index-item"]').exists()).toBe(false);
  });

  it('「正在索引云盘」只显示给本轮索引集合里的连接（不拿全局锁当每行状态）', async () => {
    controller.isRebuilding.value = true;
    controller.indexTargetConnectionIds.value = [BAIDU_ID];

    const wrapper = await mountGlobalSearch();

    const baiduRow = wrapper.find(`[data-test="remote-index-item-${BAIDU_ID}"]`);
    const quarkRow = wrapper.find(`[data-test="remote-index-item-${QUARK_ID}"]`);

    expect(baiduRow.text()).toContain('globalSearch.remoteIndexing');
    expect(quarkRow.text()).toContain('settings.globalSearch.rebuildCheckedRemoteIndex');
    expect(quarkRow.text()).not.toContain('globalSearch.remoteIndexing');
  });

  it('顶部「全部重建」按钮：只重建当前已勾选的连接集合', async () => {
    const wrapper = await mountGlobalSearch();

    const rebuildAll = wrapper.find('[data-test="rebuild-remote-index-all"]');
    expect(rebuildAll.exists()).toBe(true);

    await rebuildAll.trigger('click');

    expect(controller.triggerRebuild).toHaveBeenCalledTimes(1);
    expect(controller.triggerRebuild).toHaveBeenCalledWith([BAIDU_ID, QUARK_ID]);
  });

  it('一个连接都没勾选时：提示「尚未勾选」且不渲染「全部重建」按钮', async () => {
    controller.globalSearchConnectionIds.value = [];

    const wrapper = await mountGlobalSearch();

    expect(wrapper.find('[data-test="remote-index-empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="rebuild-remote-index-all"]').exists()).toBe(false);
  });

  it('取消勾选：当场落库为 false，且那一行的重建按钮立刻消失', async () => {
    const wrapper = await mountGlobalSearch();

    const baiduRow = wrapper.find(`[data-test="remote-index-item-${BAIDU_ID}"]`);
    expect(baiduRow.find('[data-test="rebuild-remote-index-item"]').exists()).toBe(true);

    const baiduCheckbox = wrapper.find(`[data-test="remote-index-checkbox-${BAIDU_ID}"]`);
    await baiduCheckbox.trigger('click');
    await flushPromises();

    // 数据层：勾选状态被写成 false（不是等下次重建时顺手清）。
    const store = useRemoteConnectionsStore();
    expect(store.getConnection(BAIDU_ID)?.includeInGlobalSearch).toBe(false);

    // 界面层：这一行当场不再提供「重建」入口。
    const baiduRowAfter = wrapper.find(`[data-test="remote-index-item-${BAIDU_ID}"]`);
    expect(baiduRowAfter.find('[data-test="rebuild-remote-index-item"]').exists()).toBe(false);
  });
});
