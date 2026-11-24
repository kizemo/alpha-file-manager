// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * 「加入全局检索」的云盘索引：**串行**重建 + 按网盘类型分组。
 *
 * 为什么要抽成共享 composable（批次 43）：
 *
 * - 后端的 `global_search_index_remote_connections` 是**整体重建**语义
 *   （按传进来的连接集合重建，漏掉一条就等于把那条的条目从索引里抹掉）。
 *   两个重建并发跑会互相踩 —— 后一个把前一个刚建好的索引删掉。
 * - 「设置 → 远程」页与「设置 → 搜索」页都会因勾选变化触发重建。
 *   两边各写一份 ⇒ 两把锁 ⇒ 两页同时开着就可能并发。
 *
 * ⇒ **锁与 drain 队列放在模块级**（两页共用同一份），页面只拿控制器。
 * 页面级的 `watch` 只是「勾选集合变了没有」的探测器，真正排队的那把锁是共用的。
 */

import { computed, ref, watch } from 'vue';
import type { ComputedRef, Ref } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { useI18n } from 'vue-i18n';
import { toast } from '@/components/ui/toaster';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { NETDISK_PROTOCOLS, NETDISK_PROVIDER_NAME_KEYS } from '@/types/remote';
import type { NetdiskProtocol, RemoteConnection } from '@/types/remote';
import { ONEDRIVE_ENABLED } from '@/constants/feature-flags';

/** 一次重建要传的连接 id 集合。 */
type ConnectionIdList = string[];

/** i18n 取值函数。模块级的 drain 要靠它出错误提示。 */
type TranslateFn = (key: string) => string;

/**
 * 正在排队的重建：**最新**的那份连接集合。
 *
 * 重建期间又有勾选变化时记在这里，跑完用最新集合补跑一次 ——
 * 补跑用的是最新集合，不是当时那个旧的。
 */
const pendingRemoteIndexIds = ref<ConnectionIdList | null>(null);

/**
 * 是否已有重建在跑。
 *
 * **这既是那把锁，也是按钮上的转圈状态** —— 两页共用同一份，
 * 所以「设置 → 远程」页正在重建时，「设置 → 搜索」页的那颗按钮也是禁用态。
 */
const isRebuildingRemoteIndex = ref(false);

/** 上一次重建写进去的条目数。`null` = 本次会话还没重建过。 */
const lastRemoteIndexedItemCount = ref<number | null>(null);

/**
 * 正在被索引的连接 ID 集合。
 *
 * **在 `triggerRebuild()` 提交意图的那一刻同步写入**，不等 drain 真正开跑 ——
 * 否则取消勾选后仍要等上一轮索引跑完才回落，用户看到的正是「状态没变化」。
 *
 * 为什么不直接拿 `isRebuildingRemoteIndex` 渲染每一行：那是全局互斥锁，
 * 后端「按传入集合整体重建」的语义下任何一条连接在索引都会让它为真，
 * 用它当每行状态会让没参与的网盘一起显示「正在索引云盘」。
 */
const remoteIndexTargetIds = ref<ConnectionIdList>([]);

/**
 * 上一次重建**失败**了 ⇒ 索引内容可能与当前勾选不一致。
 *
 * 批次 44 起（O-1）查询侧已经按**当前**勾选集合过滤，所以「取消勾选了还能搜到」
 * 这个问题已经不存在了；这里要补的只是**新勾选的连接条目还没进索引**这一类缺口。
 *
 * 这个标记让失败可以自愈：下一次进设置页（`markReady()`）自动补跑一次重建。
 */
let remoteIndexResyncNeeded = false;

/**
 * 本轮失败**已经自动补跑过一次**了。
 *
 * 这是 O-2 的修复：两个设置页的 `onMounted` 都会调 `markReady()`，
 * 而 `pendingRemoteIndexIds` 的合并只覆盖「正在跑」的期间 ——
 * 「已 drain 完 + 失败」之后每进一次页就会再触发一次重建（多花一次网盘 API 调用）。
 *
 * 有了这道闸门：**一次失败最多自动补跑一次**；补跑仍失败就等用户手动重建
 * （设置页上有重建按钮）或下一次勾选变化。
 */
let remoteIndexResyncAttempted = false;

/**
 * 云盘索引的**格式**在本进程里检查过了没有。
 *
 * 批次 44 给云盘索引加了 `connection_id` 字段 ⇒ 升级前建的老索引格式对不上，
 * 查询侧会直接忽略它 ⇒ 用户表现为「升级之后云盘搜不到东西」。
 * 所以进设置页时问一次后端「索引是什么状态」，`stale` 就补跑一次重建。
 *
 * **每个进程只问一次**：这是升级后的一次性成本，不该每进一次设置页就查一遍。
 */
let remoteIndexFormatChecked = false;

/** 上一次已知的勾选集合（排序后拼成的串）。用来判「集合变了没有」。 */
let knownGlobalSearchIdsKey = '';

/**
 * 初始读盘**不算**「集合变了」。
 *
 * 刚打开设置页就去重建云盘索引是白耗网盘 API 配额 ——
 * 只有用户真的改了勾选状态才需要重建。
 */
let canTriggerRemoteIndex = false;

function globalSearchIdsKey(ids: ConnectionIdList): string {
  return [...ids].sort().join('|');
}

/**
 * 把排队中的重建跑干净（一次真正的重建 + 必要时用最新集合补跑）。
 *
 * 用递归而不是 `while` + `await`：项目 ESLint 开着严格规则，循环里 await
 * 容易踩 `no-await-in-loop`。递归每次只走一层，深度最多 2
 * （一次重建 + 一次收尾），不会无限递归。
 */
async function drainRemoteIndexRebuild(translate: TranslateFn): Promise<void> {
  const ids = pendingRemoteIndexIds.value;
  pendingRemoteIndexIds.value = null;

  if (ids === null) {
    isRebuildingRemoteIndex.value = false;
    // 本轮结束（含被取消勾选后收尾的那轮）⇒ 按连接的状态一并清空，
    // 不能让「正在索引」挂在已经不该索引的连接上。
    remoteIndexTargetIds.value = [];
    return;
  }

  isRebuildingRemoteIndex.value = true;

  try {
    const indexedCount = await invoke<number>('global_search_index_remote_connections', {
      connectionIds: ids,
    });

    lastRemoteIndexedItemCount.value = indexedCount;
    // 索引已经和传进去的勾选集合对齐了，自愈标记可以清掉。
    remoteIndexResyncNeeded = false;
    // 这一轮没失败 ⇒ 「本轮是否已自动补跑过」也回到初始态。
    remoteIndexResyncAttempted = false;

    toast({
      title: translate('globalSearch.remoteIndexDone'),
    });
  }
  catch (error) {
    console.error('Failed to rebuild remote global search index:', error);
    remoteIndexResyncNeeded = true;
    // ⚠️ **不动 `remoteIndexResyncAttempted`**：
    // 这次失败如果本身就是「自动补跑」那一次，再清一次闸门就等于每次进页都补跑一次
    // （O-2 要消掉的正是这个）。闸门只在「勾选集合变了」时重开。
    toast({
      title: translate('globalSearch.remoteIndexFailed'),
    });
  }

  // 重建期间又有勾选变化 ⇒ 用最新集合再跑一次。
  await drainRemoteIndexRebuild(translate);
}

/** 一个网盘类型分组。 */
export interface RemoteNetdiskGroup {
  /** 网盘协议（`baidu_pan` / `quark_pan` / `aliyun_pan` / …）。 */
  protocol: NetdiskProtocol;
  /** 组标题的 i18n 键（走 `NETDISK_PROVIDER_NAME_KEYS`）。 */
  nameKey: string;
  /** 该类型下的连接。**没有连接时是空数组** —— 分组照样显示。 */
  connections: RemoteConnection[];
}

/**
 * 当前应当出现在界面上的网盘类型清单。
 *
 * **动态派生**：清单来自 `NETDISK_PROTOCOLS`，不是写死三个名字 ——
 * 以后接了第四家网盘，这一页自动多出一组，不用再改界面。
 */
export function resolveVisibleNetdiskProtocols(): NetdiskProtocol[] {
  const protocols: NetdiskProtocol[] = [];

  for (const protocol of NETDISK_PROTOCOLS) {
    // ⚠️ **顺序不能反**：先比较协议，再取反开关。
    // 写成 `ONEDRIVE_ENABLED && protocol === 'onedrive_pan'` 会让 TS 把
    // `protocol` 收窄成 `never`（`ONEDRIVE_ENABLED` 是字面量 `false`），触发 TS2345。
    if (protocol === 'onedrive_pan' && !ONEDRIVE_ENABLED) {
      continue;
    }

    protocols.push(protocol);
  }

  return protocols;
}

/**
 * 按网盘类型分组。
 *
 * **没有连接的网盘类型也会出现在结果里**（`connections` 是空数组）——
 * 这样「未来会添加的云盘」自然出现在列表里，用户也看得懂这一家存在。
 */
export function buildRemoteNetdiskGroups(
  connections: readonly RemoteConnection[],
): RemoteNetdiskGroup[] {
  return resolveVisibleNetdiskProtocols().map(protocol => ({
    protocol,
    nameKey: NETDISK_PROVIDER_NAME_KEYS[protocol],
    connections: connections.filter(connection => connection.protocol === protocol),
  }));
}

/** 勾选变化 → 索引重建 这条链路的控制器。 */
export interface RemoteGlobalSearchIndexController {
  /** 勾选了「加入全局检索」的连接 id —— 建索引时传的就是这个集合。 */
  globalSearchConnectionIds: ComputedRef<ConnectionIdList>;
  /** 正在重建云盘索引吗（全局互斥锁：任何一条在跑都为真）。 */
  isRebuilding: Ref<boolean>;
  /**
   * 本轮被索引的连接 id。**渲染单行状态用这个**，别用 `isRebuilding` ——
   * 后者是全局锁，会让没参与的网盘一起显示「正在索引云盘」。
   */
  indexTargetConnectionIds: Ref<ConnectionIdList>;
  /** 上一次重建写进去的条目数。`null` = 本次会话还没重建过。 */
  lastIndexedItemCount: Ref<number | null>;
  /** 手动触发一次重建（传当前全部已勾选的连接）。 */
  triggerRebuild: (connectionIds: ConnectionIdList) => void;
  /** 读盘完成后开闸：初始的那批连接不算「集合变了」。 */
  markReady: () => void;
}

/**
 * 拿到「加入全局检索」的云盘索引控制器。
 *
 * 「设置 → 远程」页与「设置 → 搜索」页**共用这一份实现**：
 * 同一把锁、同一个 drain 队列、同一条「勾选变化 → 重建」的触发链。
 */
export function useRemoteGlobalSearchIndex(): RemoteGlobalSearchIndexController {
  const store = useRemoteConnectionsStore();
  const { t } = useI18n();

  function translate(key: string): string {
    return t(key);
  }

  const globalSearchConnectionIds = computed<ConnectionIdList>(() => (
    store.globalSearchConnections.map(connection => connection.id)
  ));

  /**
   * 重建云盘索引。
   *
   * **串行**：同一时刻只跑一个重建（理由见上面的 `isRebuildingRemoteIndex`）。
   *
   * **失败绝不影响保存**：连接已经落盘了，索引重建失败只是「这次没抓到」，
   * 用户还能手动重来。所以这里只弹一条提示，不往上抛。
   */
  function triggerRebuild(connectionIds: ConnectionIdList): void {
    // ⚠️ **必须在这里同步写，不能等 drain 开跑才写**：
    // 取消勾选的那一瞬间，界面就该认定这一行不再被索引。
    remoteIndexTargetIds.value = [...connectionIds];
    pendingRemoteIndexIds.value = [...connectionIds];

    if (isRebuildingRemoteIndex.value) {
      return;
    }

    // 不 await：设置界面不能被索引重建卡住。
    void drainRemoteIndexRebuild(translate);
  }

  /**
   * 云盘索引的**格式**是不是落后于当前版本了？落后就补跑一次重建。
   *
   * 批次 44 给云盘索引加了 `connection_id` 字段（O-1）⇒ 升级前建的老索引
   * 与新 schema 对不上，查询侧会直接忽略它 ⇒ 用户表现为「升级之后云盘搜不到东西」。
   *
   * **只在进设置页时问一次**，不在每次搜索时问：
   * 重建要花网盘 API 配额，不能因为「格式不对」就在用户搜索时偷偷烧。
   *
   * `missing`（压根没建过索引）**不自动重建** —— 那通常是用户还没勾选网盘，
   * 不该替他花配额（这一档留给手动重建按钮）。
   */
  async function ensureRemoteIndexFormatCurrent(ids: ConnectionIdList): Promise<void> {
    if (remoteIndexFormatChecked) {
      return;
    }

    remoteIndexFormatChecked = true;

    // 已经有重建在跑/排队 ⇒ 那一次重建本身就是用新格式写的，格式问题顺带解决。
    if (isRebuildingRemoteIndex.value || pendingRemoteIndexIds.value !== null) {
      return;
    }

    try {
      const state = await invoke<string>('global_search_remote_index_state');

      if (state !== 'stale') {
        return;
      }

      // 问的这几行里可能刚好起了一轮重建 ⇒ 那轮就是新格式，不必再来一次。
      if (isRebuildingRemoteIndex.value || pendingRemoteIndexIds.value !== null) {
        return;
      }

      triggerRebuild(ids);
    }
    catch (error) {
      console.error('Failed to read the remote global search index state:', error);
    }
  }

  /**
   * 读盘完成后开闸。
   *
   * 顺带做两件**自愈**：
   *
   * 1. 上一轮重建失败 ⇒ 索引里可能缺新勾选连接的条目 ⇒ 自动补跑一次重建；
   * 2. 索引格式落后于当前版本（升级换过 schema）⇒ 同样补跑一次重建。
   *
   * ⚠️ 第 1 条**一次失败最多补跑一次**（O-2）：两个设置页都会调 `markReady()`，
   * 没有闸门的话「失败 → 进 A 页补跑 → 进 B 页再补跑」会白花两次网盘 API。
   */
  function markReady(): void {
    knownGlobalSearchIdsKey = globalSearchIdsKey(globalSearchConnectionIds.value);
    canTriggerRemoteIndex = true;

    if (remoteIndexResyncNeeded && !remoteIndexResyncAttempted) {
      remoteIndexResyncAttempted = true;
      triggerRebuild(globalSearchConnectionIds.value);
    }

    // 升级换了索引格式 ⇒ 老索引用不了，补跑一次重建（一次性成本）。
    // 不 await：设置界面不能被索引重建卡住。
    void ensureRemoteIndexFormatCurrent(globalSearchConnectionIds.value);
  }

  watch(globalSearchConnectionIds, (ids) => {
    const key = globalSearchIdsKey(ids);

    if (!canTriggerRemoteIndex) {
      knownGlobalSearchIdsKey = key;
      return;
    }

    if (key === knownGlobalSearchIdsKey) {
      return;
    }

    knownGlobalSearchIdsKey = key;

    // 勾选集合变了 = 用户有了新意图 ⇒ 重开「自动补跑」的闸门，
    // 这样「改完勾选又失败」之后仍允许自动补跑一次（O-2）。
    remoteIndexResyncAttempted = false;

    // 不 await：保存界面不能被索引重建卡住。
    triggerRebuild(ids);
  });

  return {
    globalSearchConnectionIds,
    isRebuilding: isRebuildingRemoteIndex,
    indexTargetConnectionIds: remoteIndexTargetIds,
    lastIndexedItemCount: lastRemoteIndexedItemCount,
    triggerRebuild,
    markReady,
  };
}
