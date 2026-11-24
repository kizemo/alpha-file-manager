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

/**
 * 所有**重建**调用（`global_search_index_remote_connections`）传出去的 connectionIds。
 *
 * ⚠️ 批次 44 起 `markReady()` 还会额外问一次「云盘索引是什么状态」
 * （`global_search_remote_index_state`），那**不是**重建调用 ——
 * 所以这里按命令名过滤，不能直接数 `invokeMock` 的总调用数。
 */
function rebuildCalls(): string[][] {
  return invokeMock.mock.calls
    .filter(call => call[0] === 'global_search_index_remote_connections')
    .map(call => (call[1] as { connectionIds: string[] }).connectionIds);
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
    expect(rebuildCalls()).toHaveLength(0);
  });

  // 批次 44（O-1）：云盘索引加了 `connection_id` 字段 ⇒ 升级前建的老索引格式对不上，
  // 查询侧会直接忽略它 ⇒ 用户表现为「升级之后云盘搜不到东西」。
  // 所以进设置页时问一次后端「索引是什么状态」，`stale` 就补跑一次重建。
  //
  // ⚠️ 这条用例必须排在下面那条 `markReady` 用例**之前**：
  // 「索引状态只问一次」是**模块级**闸门，问过就不再问（升级后的一次性成本）。
  it('索引格式落后（stale）时，进设置页自动补跑一次重建', async () => {
    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);

    invokeMock.mockImplementation(async (command: string) => (
      command === 'global_search_remote_index_state' ? 'stale' : 3
    ));

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(1);
    expect(rebuildCalls()[0]).toEqual(['bd-1']);
  });

  // O-2：一次失败**最多**自动补跑一次。
  // 两个设置页的 `onMounted` 都会调 `markReady()`；没有闸门的话
  // 「失败 → 进 A 页补跑（也失败）→ 再进 B 页」会多补跑一次（多花一次网盘 API）。
  it('一次失败最多自动补跑一次：补跑仍失败时，再进设置页不再补跑', async () => {
    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);

    invokeMock.mockRejectedValue(new Error('quota exhausted'));

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.triggerRebuild(['bd-1']);
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(1);

    // 第一次进设置页 ⇒ 补跑一次（这次也失败）。
    controller.markReady();
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(2);

    // 第二次进设置页 ⇒ **不再**补跑（O-2 修掉的就是这一次多余的调用）。
    controller.markReady();
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(2);
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

    expect(rebuildCalls()).toHaveLength(1);
    expect(rebuildCalls()[0].slice().sort()).toEqual(['bd-1', 'quark-1']);
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

    expect(rebuildCalls()).toHaveLength(1);
    expect(rebuildCalls()[0]).toEqual(['bd-1']);
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

    expect(rebuildCalls()).toHaveLength(0);
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
    expect(rebuildCalls()).toHaveLength(1);

    // 第一次还没跑完就又改了一次。
    controller.triggerRebuild(['bd-1', 'quark-1']);
    await nextTick();

    expect(rebuildCalls()).toHaveLength(1);

    first.resolve(9);
    await flushPromises();
    await nextTick();
    await flushPromises();

    // 排队的那次必须在第一次**完成之后**才发起 —— 否则两个整体重建会互相踩。
    expect(rebuildCalls()).toHaveLength(2);
    expect(rebuildCalls()[1]).toEqual(['bd-1', 'quark-1']);
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

    expect(rebuildCalls()).toHaveLength(1);

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

    expect(rebuildCalls()[1]).toEqual(['bd-1', 'quark-1']);
  });
});

// 批次 43 真机反馈缺陷：「设置 → 搜索」里勾一家网盘，三家同时显示「正在索引云盘」；
// 取消勾选之后状态又不回落。根因是把**全局互斥锁**当成了单行状态来渲染 ——
// 后端是「按传入集合整体重建」，任何一条在索引都会让全局锁为真。
//
// 修复口径是两条判据分开：`isRebuilding` 只管按钮能不能点（重建必须串行），
// `indexTargetConnectionIds` 才管某一行的文案。下面这批用例钉住的就是这个分工。
//
// ⚠️ 本 describe 仍然受文件级约定约束（不用 `vi.resetModules()`）：
// 前面的用例已经把 `canTriggerRemoteIndex` 闸门打开，所以这里每个用例都显式
// `markReady()`，并且**每条都以成功收尾**，不留失败自愈标记污染下一条。
describe('按连接的索引状态：全局锁管禁用、按连接集合管文案', () => {
  it('triggerRebuild 提交的那一刻就记下索引目标，跑完一并清空', async () => {
    setFakeConnections([]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();
    const pending = createDeferred<number>();

    invokeMock.mockImplementationOnce(() => pending.promise);

    controller.triggerRebuild(['bd-1']);

    // ⚠️ 这里**故意不 await**：这条断言要的就是「同步写入」。
    // 等后端返回才更新的话，用户点下去的那一下界面仍然毫无反应。
    expect(controller.indexTargetConnectionIds.value).toEqual(['bd-1']);
    expect(controller.isRebuilding.value).toBe(true);

    pending.resolve(5);
    await flushPromises();
    await nextTick();
    await flushPromises();

    // 一轮跑完 ⇒ 按连接的状态必须清空，不能挂在已经不该索引的行上。
    expect(controller.indexTargetConnectionIds.value).toEqual([]);
    expect(controller.isRebuilding.value).toBe(false);
  });

  it('索引进行中的那一轮，没参与网盘的 id 不在集合里', async () => {
    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('ali-1', 'aliyun_pan', true),
      createConnection('quark-1', 'quark_pan', true),
    ]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    const pending = createDeferred<number>();

    invokeMock.mockImplementationOnce(() => pending.promise);

    // 只索引百度那一条 ⇒ 阿里/夸克不能跟着显示「正在索引云盘」。
    controller.triggerRebuild(['bd-1']);

    expect(controller.indexTargetConnectionIds.value).toEqual(['bd-1']);
    expect(controller.indexTargetConnectionIds.value.includes('ali-1')).toBe(false);
    expect(controller.indexTargetConnectionIds.value.includes('quark-1')).toBe(false);
    // 全局锁仍然为真 —— 它只负责「同一时刻只能有一个重建」，按钮禁用靠它。
    expect(controller.isRebuilding.value).toBe(true);

    pending.resolve(1);
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(controller.indexTargetConnectionIds.value).toEqual([]);
  });

  it('取消勾选的瞬间该连接就摘出集合，不等在跑的那轮结束', async () => {
    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('quark-1', 'quark_pan', true),
    ]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    const pending = createDeferred<number>();

    invokeMock.mockImplementationOnce(() => pending.promise);

    controller.triggerRebuild(['bd-1', 'quark-1']);

    expect(controller.indexTargetConnectionIds.value).toEqual(['bd-1', 'quark-1']);

    // 用户在这一轮还没跑完的时候就取消了勾选。
    // 修复前：全局锁还是 true ⇒ 那一行继续显示「正在索引云盘」，状态不回落。
    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);
    await nextTick();

    expect(controller.indexTargetConnectionIds.value).toEqual(['bd-1']);

    pending.resolve(5);
    await flushPromises();
    await nextTick();
    await flushPromises();

    // 补跑用的是**最新**集合（只剩 bd-1），跑完才清空。
    expect(rebuildCalls()[rebuildCalls().length - 1]).toEqual(['bd-1']);
    expect(controller.indexTargetConnectionIds.value).toEqual([]);
    expect(controller.isRebuilding.value).toBe(false);
  });

  it('新勾一条时传出去的是全部已勾选连接，不只是刚勾的那条', async () => {
    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('ali-1', 'aliyun_pan', true),
      createConnection('quark-1', 'quark_pan', false),
    ]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    controller.markReady();

    // 把夸克勾上。后端是整体重建 ⇒ 必须把百度/阿里一起带上，
    // 只传「刚勾的那条」会把它们的条目从索引里抹掉。
    // 这条同时是「勾选是否真正控制检索」的前端口径：传出去的集合就是被检索的集合。
    setFakeConnections([
      createConnection('bd-1', 'baidu_pan', true),
      createConnection('ali-1', 'aliyun_pan', true),
      createConnection('quark-1', 'quark_pan', true),
    ]);
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(1);
    expect(rebuildCalls()[0].slice().sort()).toEqual(['ali-1', 'bd-1', 'quark-1']);
    expect(controller.indexTargetConnectionIds.value).toEqual([]);
  });

  it('重建失败后下一次进设置页自动补跑，成功补跑后不再重试', async () => {
    setFakeConnections([createConnection('bd-1', 'baidu_pan', true)]);

    const controller = remoteGlobalSearchIndex.useRemoteGlobalSearchIndex();

    // 进页面开闸。此时没有失败标记 ⇒ 不该发任何重建（初始读盘不算集合变了）。
    controller.markReady();

    expect(rebuildCalls()).toHaveLength(0);

    invokeMock.mockRejectedValueOnce(new Error('quota exhausted'));
    controller.triggerRebuild(['bd-1']);
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(toastMock).toHaveBeenCalledWith({ title: 'globalSearch.remoteIndexFailed' });

    // 失败 ⇒ 索引内容与勾选可能已经不一致（查询侧不做读时过滤），
    // 下一次进设置页必须补跑，而不是把「取消勾选了还搜得到」永久留在索引里。
    const base = rebuildCalls().length;

    controller.markReady();
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(base + 1);
    expect(rebuildCalls()[base]).toEqual(['bd-1']);

    // 补跑成功 ⇒ 自愈标记清掉，不会每次进页面都白烧一次网盘 API 配额。
    controller.markReady();
    await flushPromises();
    await nextTick();
    await flushPromises();

    expect(rebuildCalls()).toHaveLength(base + 1);
  });
});
