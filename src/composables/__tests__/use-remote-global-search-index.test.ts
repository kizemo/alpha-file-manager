// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「加入全局检索」的云盘索引 composable —— 批次 43 需求 2 抽出来的共享实现。
//
// 用户要的是「设置 → 搜索 → 云盘检索」能按网盘类型列出百度/阿里/夸克
// 以及**未来会添加的云盘**。实现上有两条容易踩坏的骨架，这批用例就是钉住它们：
//
// 1. **类型清单必须是动态派生的**（来自 `NETDISK_PROTOCOLS`），不是写死三个名字。
//    写死 ⇒ 接了第四家还得再来改一次界面，而用户明确要求「及未来会添加的云盘」。
// 2. **重建必须全局串行**。后端 `global_search_index_remote_connections` 是
//    整体重建语义，两个重建并发跑会互相把对方刚建好的索引删掉。
//    「设置 → 远程」页和「设置 → 搜索」页都挂了这个 composable ⇒ 锁必须在模块级共用。

import {
  afterEach, beforeEach, describe, expect, it, vi,
} from 'vitest';
import { nextTick } from 'vue';
import { flushPromises } from '@vue/test-utils';
import { NETDISK_PROTOCOLS, NETDISK_PROVIDER_NAME_KEYS } from '@/types/remote';
import type { RemoteConnection } from '@/types/remote';
import { ONEDRIVE_ENABLED } from '@/constants/feature-flags';

const { invokeMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastMock: vi.fn(),
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: (...args: unknown[]) => toastMock(...args),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// 只替掉 store 的**读取口**：composable 需要的是一个能响应勾选变化的
// `globalSearchConnections`。用真实 pinia store 会让这批用例去碰磁盘与网盘会话，
// 而这里要验的是「勾选集合变化 → 排队重建」这条纯前端链路。
//
// `setFakeConnections` 是本模块自己加的测试出口（真实 store 没有），
// 所以下面用 `await import()` + 断言取值，不做静态 import（会过不了 vue-tsc）。
vi.mock('@/stores/storage/remote-connections', async () => {
  const { ref } = await vi.importActual<{ ref: <T>(value: T) => { value: T } }>('vue');
  const connections = ref<RemoteConnection[]>([]);

  return {
    useRemoteConnectionsStore: () => ({
      get globalSearchConnections(): RemoteConnection[] {
        return connections.value.filter(connection => connection.includeInGlobalSearch === true);
      },
    }),
    setFakeConnections: (next: RemoteConnection[]) => {
      connections.value = next;
    },
  };
});

import * as remoteGlobalSearchIndex from '@/composables/use-remote-global-search-index';

type SetFakeConnections = (next: RemoteConnection[]) => void;

function createConnection(
  id: string,
  protocol: RemoteConnection['protocol'],
  includeInGlobalSearch: boolean,
): RemoteConnection {
  return {
    id,
    name: id,
    protocol,
    host: 'example.test',
    port: 443,
    username: 'demo',
    initialPath: '/',
    proxy: { kind: 'direct' },
    includeInGlobalSearch,
  };
}

function createDeferred<T>() {
  let resolvePromise!: (value: T) => void;
  let rejectPromise!: (reason?: unknown) => void;

  const promise = new Promise<T>((resolve, reject) => {
    resolvePromise = resolve;
    rejectPromise = reject;
  });

  return {
    promise,
    resolve: resolvePromise,
    reject: rejectPromise,
  };
}

/** 某次 `invoke` 调用实际传出去的 connectionIds（按调用顺序取）。 */
function invokedIds(callIndex: number): string[] {
  const call = invokeMock.mock.calls[callIndex];

  return (call?.[1] as { connectionIds: string[] }).connectionIds;
}

let setFakeConnections: SetFakeConnections;

beforeEach(async () => {
  invokeMock.mockReset();
  invokeMock.mockResolvedValue(3);
  toastMock.mockReset();

  const storeModule = await import('@/stores/storage/remote-connections');

  setFakeConnections = (storeModule as unknown as { setFakeConnections: SetFakeConnections })
    .setFakeConnections;
});

// ⚠️ 本文件**故意不用** `vi.resetModules()`：锁与 drain 队列是模块级的，
// 重置模块会连 vue 的响应式实例一起换掉，反而失真。取而代之的做法是
// 让用例顺序与模块状态兼容（见每个用例开头的注释）。
afterEach(async () => {
  // 把上一轮排队的重建跑干净，避免残留的 in-flight 影响下一个用例。
  await flushPromises();
  await nextTick();
});

describe('resolveVisibleNetdiskProtocols：网盘类型清单是派生的', () => {
  it('包含百度 / 阿里 / 夸克三家', () => {
    const visible = remoteGlobalSearchIndex.resolveVisibleNetdiskProtocols();

    expect(visible).toContain('baidu_pan');
    expect(visible).toContain('aliyun_pan');
    expect(visible).toContain('quark_pan');
  });

  it('OneDrive 未上线时不出现在清单里', () => {
    const visible = remoteGlobalSearchIndex.resolveVisibleNetdiskProtocols();

    // 不加 if 守卫（`vitest/no-conditional-expect`），并且把断言写成
    // **双向**：清单里有 onedrive_pan 当且仅当开关是开的。
    // 开关以后翻成 true，这条用例仍然成立，同时仍然能抓住
    // 「开关关着却把 OneDrive 渲染出来」这个真实缺陷。
    expect(visible.includes('onedrive_pan')).toBe(ONEDRIVE_ENABLED);
  });

  it('清单恰好等于 NETDISK_PROTOCOLS 减去被开关关掉的那些（新增网盘会自动出现）', () => {
    expect(remoteGlobalSearchIndex.resolveVisibleNetdiskProtocols()).toEqual(
      NETDISK_PROTOCOLS.filter(protocol => protocol !== 'onedrive_pan' || ONEDRIVE_ENABLED),
    );
  });
});

describe('buildRemoteNetdiskGroups：按网盘类型分组', () => {
  it('一条连接都没有时，每个可见类型仍然各有一个分组', () => {
    const groups = remoteGlobalSearchIndex.buildRemoteNetdiskGroups([]);
    const visible = remoteGlobalSearchIndex.resolveVisibleNetdiskProtocols();

    // 这一条就是「未来会添加的云盘」的实现口径：没有连接也要列出来，
    // 而不是等有连接了才凭空多出一行。
    expect(groups.map(group => group.protocol)).toEqual(visible);
    expect(groups.every(group => group.connections.length === 0)).toBe(true);
  });

  it('连接按协议落到各自的组里，不会串组', () => {
    const groups = remoteGlobalSearchIndex.buildRemoteNetdiskGroups([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('quark-1', 'quark_pan', false),
      createConnection('bd-2', 'baidu_pan', false),
      createConnection('sftp-1', 'sftp', true),
    ]);

    const baidu = groups.find(group => group.protocol === 'baidu_pan');
    const quark = groups.find(group => group.protocol === 'quark_pan');

    expect(baidu?.connections.map(connection => connection.id)).toEqual(['bd-1', 'bd-2']);
    expect(quark?.connections.map(connection => connection.id)).toEqual(['quark-1']);
  });

  it('非网盘连接（sftp）不占任何组', () => {
    const groups = remoteGlobalSearchIndex.buildRemoteNetdiskGroups([
      createConnection('sftp-1', 'sftp', true),
    ]);

    expect(groups.every(group => group.connections.length === 0)).toBe(true);
  });

  it('每组的标题键取自 NETDISK_PROVIDER_NAME_KEYS', () => {
    const groups = remoteGlobalSearchIndex.buildRemoteNetdiskGroups([]);

    for (const group of groups) {
      expect(group.nameKey).toBe(NETDISK_PROVIDER_NAME_KEYS[group.protocol]);
    }
  });
});

describe('勾选变化 → 云盘索引重建', () => {
  it('初始读盘不算「集合变了」，不触发重建', () => {
    // 模块级的 canTriggerRemoteIndex 初始为 false —— 这条用例必须是本 describe
    // 里第一个碰 store 的，否则闸门已被前面的用例打开。
    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    expect(controller.globalSearchConnectionIds.value).toEqual(['bd-1']);
    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('markReady 之后改勾选，按新的已勾选集合重建一次', async () => {
    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('quark-1', 'quark_pan', true),
    ]);
    await nextTick();
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokedIds(0).slice().sort()).toEqual(['bd-1', 'quark-1']);
    expect(toastMock).toHaveBeenCalledWith({ title: 'globalSearch.remoteIndexDone' });
  });

  it('勾掉的连接会从重建集合里去掉（整体重建不能漏条目）', async () => {
    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('quark-1', 'quark_pan', true),
    ]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);
    await nextTick();
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokedIds(0)).toEqual(['bd-1']);
  });

  it('改的是没勾检索的连接 ⇒ 集合没变，不重建', async () => {
    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('quark-1', 'quark_pan', false),
    ]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    // 同一批勾选集合，只是未勾选的那条换了名字/协议顺序。
    setFakeConnections([
      createConnection('quark-1', 'quark_pan', false),
      createConnection('bd-1', 'baidu_pan', true),
    ]);
    await nextTick();
    await flushPromises();

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('重建进行中再改勾选 ⇒ 串行补跑，且补跑用最新集合', async () => {
    const first = createDeferred<number>();

    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    invokeMock.mockImplementationOnce(() => first.promise);

    controller.triggerRebuild(['bd-1']);
    await nextTick();

    expect(controller.isRebuilding.value).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(1);

    // 第一次还没跑完就又改了一次。
    controller.triggerRebuild(['bd-1', 'quark-1']);
    await nextTick();

    expect(invokeMock).toHaveBeenCalledTimes(1);

    first.resolve(9);
    await flushPromises();
    await nextTick();
    await flushPromises();

    // 排队的那次必须在第一次**完成之后**才发起 —— 否则两个整体重建会互相踩。
    expect(invokeMock).toHaveBeenCalledTimes(2);
    expect(invokedIds(1)).toEqual(['bd-1', 'quark-1']);
    // 补跑那一次走的是 beforeEach 里的默认实现（`mockResolvedValue(3)`）——
    // `mockImplementationOnce` 只覆盖第 1 次调用。所以最终计数是 3 而不是 9：
    // 这恰好证明**第二次调用真的发出并写回了自己的结果**，而不是沿用第一次。
    expect(controller.lastIndexedItemCount.value).toBe(3);
    expect(controller.isRebuilding.value).toBe(false);
  });

  it('两个页面各自拿一个控制器，仍然共用同一把锁', async () => {
    const pending = createDeferred<number>();

    setFakeConnections([]);

    const remotePage = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();
    const searchPage = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    invokeMock.mockImplementationOnce(() => pending.promise);

    remotePage.triggerRebuild(['bd-1']);
    await nextTick();

    // 远程页正在重建 ⇒ 搜索页的按钮也必须是禁用态（两页同时开着才会撞）。
    expect(remotePage.isRebuilding.value).toBe(true);
    expect(searchPage.isRebuilding.value).toBe(true);

    // 搜索页此时手动再点一次，不能并发起第二个重建。
    searchPage.triggerRebuild(['bd-1', 'quark-1']);
    await nextTick();

    expect(invokeMock).toHaveBeenCalledTimes(1);

    pending.resolve(1);
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(searchPage.isRebuilding.value).toBe(false);
    expect(remotePage.isRebuilding.value).toBe(false);
  });

  it('重建失败只提示一次，并把锁释放掉（不卡死后续重建）', async () => {
    invokeMock.mockRejectedValueOnce(new Error('quota exhausted'));

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.triggerRebuild(['bd-1']);
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(toastMock).toHaveBeenCalledWith({ title: 'globalSearch.remoteIndexFailed' });
    expect(controller.isRebuilding.value).toBe(false);

    // 锁没卡死：还能再重建。
    controller.triggerRebuild(['bd-1', 'quark-1']);
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(invokedIds(1)).toEqual(['bd-1', 'quark-1']);
  });
});
