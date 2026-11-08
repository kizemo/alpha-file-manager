// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { RemoteConnection } from '@/types/remote';
import { buildRemoteConnectionPath } from '@/utils/remote-paths';

/** 「位置」页里的一条远程条目。这里只要路径与显示名，协议细节「位置」页用不上。 */
export interface RemoteLocationRef {
  path: string;
  name: string;
}

let remoteLocationRefs: RemoteLocationRef[] = [];

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
  remoteLocationRefs = connectionList.flatMap((connection) => {
    const path = buildRemoteConnectionPath(connection.id);

    // 非法 id 会得到 null，直接跳过 —— 宁可少一条，也不要造一条点开就报错的条目。
    return path
      ? [{
          path,
          name: connection.name,
        }]
      : [];
  });
}

export function getRemoteLocationRefs(): RemoteLocationRef[] {
  return remoteLocationRefs;
}
