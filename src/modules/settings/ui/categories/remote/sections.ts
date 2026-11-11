// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root.

// 远程分类内部的「网盘 / 远程连接」两分区判定（用户 2026-11-08）。
//
// **单拎成一个模块，是因为有两个消费方**：`connections-section.vue` 用它决定
// 列表与表单落在哪个标签，`index.vue` 用它决定「信任的主机」要不要挂载。
// 两份口径各写一遍的话，以后把 `isNetdiskProtocol` 扩到别的网盘协议时，
// 只改一处就会出现「连接在网盘标签下，信任的主机却在远程连接标签下」这种错位。
//
// 判定口径固定在 `isNetdiskProtocol`（认 `baidu_pan` 与 `quark_pan`）。
// 以后接入阿里云盘之类的网盘，把判定扩进去，上下游分区自动跟上。

import { isNetdiskProtocol } from '@/types/remote';
import type { RemoteConnection, RemoteProtocol } from '@/types/remote';

export type RemoteSection = 'netdisk' | 'remote';

export function sectionOfProtocol(protocol: RemoteProtocol): RemoteSection {
  return isNetdiskProtocol(protocol) ? 'netdisk' : 'remote';
}

export function sectionOfConnection(connection: RemoteConnection): RemoteSection {
  return sectionOfProtocol(connection.protocol);
}
