// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { RemoteConnection, RemoteProtocol } from '@/types/remote';
import { buildRemoteConnectionPath } from '@/utils/remote-paths';

/**
 * 「位置」页里的一条远程条目。
 *
 * `protocol` 一并留着：「位置」页要按「网盘 / 远程连接」分区（用户 2026-11-08 需求 1），
 * 归类口径与设置页、主页完全一致 —— `isBaiduPanProtocol`。
 */
export interface RemoteLocationRef {
  path: string;
  name: string;
  protocol: RemoteProtocol;
}

let remoteLocationRefs: RemoteLocationRef[] = [];

/**
 * 「位置」页的远程条目变了就通知订阅者。
 *
 * 用途（2026-11-08）：位置页在导航器里，它的内容是**读目录时顺手拼**的 ——
 * 连接列表改了之后，页面不会自己重读目录，远程条目要等下一次刷新才出现。
 * 订阅机制让「位置」页能在连接变化时立刻重画，不必用户手动切页。
 *
 * 刻意做成**订阅**而不是轮询：轮询要么靠定时器（常驻开销），
 * 要么靠 store 的响应式（又会把 store 拖进这个纯工具模块 —— 见下面的说明）。
 */
type RemoteLocationRefsListener = () => void;

const remoteLocationRefsListeners = new Set<RemoteLocationRefsListener>();

/**
 * 记录「位置」页要展示的远程条目（用户 2026-11-08 需求 3）。
 *
 * 为什么不让 `virtual-locations.ts` 直接读 store：那个模块被工具栏、地址栏和一堆纯函数单测
 * 引入，一旦它 import 了 store，就会把 `@tauri-apps/plugin-store` 与 `@/data/user-paths`
 * 一并拖进每个使用方的模块图。实测代价是拖坏了
 * `use-file-browser-drag-session.test.ts` —— 它的 `@tauri-apps/api/path` mock 只有
 * `resolveResource`，模块图变宽后 `audioDir` 缺失，一次挂掉 8 个用例。
 *
 * 所以这里只存数据、不依赖任何运行时，由 `remote-connections` store 在连接变化时写入。
 */
export function setRemoteLocationRefs(connectionList: RemoteConnection[]): void {
  const nextRefs = connectionList.flatMap((connection) => {
    const path = buildRemoteConnectionPath(connection.id);

    // 非法 id 会得到 null，直接跳过 —— 宁可少一条，也不要造一条点开就报错的条目。
    return path
      ? [{
          path,
          name: connection.name,
          protocol: connection.protocol,
        }]
      : [];
  });

  const previousKey = buildRefsFingerprint(remoteLocationRefs);
  const nextKey = buildRefsFingerprint(nextRefs);

  remoteLocationRefs = nextRefs;

  // 内容没变就不通知：store 的 watch 可能在别的字段变动时也调到这里，
  // 每次都通知会让「位置」页无意义地重读目录。
  if (previousKey === nextKey) {
    return;
  }

  // 复制一份再遍历：监听者可能在回调里退订，直接遍历 Set 会踩到「集合在迭代中被修改」。
  for (const listener of Array.from(remoteLocationRefsListeners)) {
    try {
      listener();
    }
    catch (error) {
      // 一个监听者抛错不能连累其它监听者，更不能让 store 的写入路径整个断掉。
      console.error('[remote-location-refs] listener failed:', error);
    }
  }
}

export function getRemoteLocationRefs(): RemoteLocationRef[] {
  return remoteLocationRefs;
}

/**
 * 订阅「位置」页远程条目的变化通知。
 *
 * 返回退订函数；调用方必须在组件卸载时退订，否则监听者会一直挂在模块级的 Set 上，
 * 组件被反复挂载就会累积成泄漏。
 */
export function subscribeRemoteLocationRefs(listener: RemoteLocationRefsListener): () => void {
  remoteLocationRefsListeners.add(listener);

  return () => {
    remoteLocationRefsListeners.delete(listener);
  };
}

/**
 * 条目集合的指纹：只取「页面上看得见」的三项。
 *
 * 刻意不取连接 id 之外的东西 —— 指纹要能反映「用户看到的列表变没变」，
 * 而不是「store 里的对象换没换」。连接被改名会让指纹变，连接顺序调整不会
 * （顺序由条目排序负责，不属于「内容变了」）。
 */
function buildRefsFingerprint(refs: RemoteLocationRef[]): string {
  return refs
    .map(ref => `${ref.path}\x1f${ref.name}\x1f${ref.protocol}`)
    .sort()
    .join('\x1e');
}
