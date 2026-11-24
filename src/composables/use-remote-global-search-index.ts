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
    return;
  }

  isRebuildingRemoteIndex.value = true;

  try {
    const indexedCount = await invoke<number>('global_search_index_remote_connections', {
      connectionIds: ids,
    });

    lastRemoteIndexedItemCount.value = indexedCount;

    toast({
      title: translate('globalSearch.remoteIndexDone'),
    });
  }
  catch (error) {
    console.error('Failed to rebuild remote global search index:', error);
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
  /** 正在重建云盘索引吗。 */
  isRebuilding: Ref<boolean>;
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
    pendingRemoteIndexIds.value = [...connectionIds];

    if (isRebuildingRemoteIndex.value) {
      return;
    }

    // 不 await：设置界面不能被索引重建卡住。
    void drainRemoteIndexRebuild(translate);
  }

  function markReady(): void {
    knownGlobalSearchIdsKey = globalSearchIdsKey(globalSearchConnectionIds.value);
    canTriggerRemoteIndex = true;
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

    // 不 await：保存界面不能被索引重建卡住。
    triggerRebuild(ids);
  });

  return {
    globalSearchConnectionIds,
    isRebuilding: isRebuildingRemoteIndex,
    lastIndexedItemCount: lastRemoteIndexedItemCount,
    triggerRebuild,
    markReady,
  };
}
