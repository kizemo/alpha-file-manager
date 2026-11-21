// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';

/**
 * 条目的**唯一键**。选中判定、缩略图缓存都用它，而不是 `path`。
 *
 * 为什么不能用 `path`：
 *
 * 远程网盘条目的路径由「父目录路径 + 条目名」拼成，而**同一个目录里允许存在同名条目**。
 * 阿里云盘真机实测（2026-11-21，`.superpowers/sdd/b32/b32-probe-walk.txt`）：
 * 「手机备份/照片备份」下有约 90 组同名图片，「手机备份/视频备份」下有 8 组同名视频。
 * 两个不同条目的 `path` 因此**逐字相同** —— 按 `path` 比较时，
 * 点其中一个条目会把另一个一起算成「已选中」，于是同时高亮两个（用户 2026-11-21 报的 bug）。
 *
 * 远程条目带 `entry_id`（服务端给的稳定 ID，带 provider 前缀）⇒ 用它当键；
 * 本地条目的路径天然唯一 ⇒ 仍用 `path`。
 */
export function getFileBrowserEntryKey(entry: DirEntry): string {
  const entryId = entry.entry_id;

  if (entryId) {
    return `id:${entryId}`;
  }

  return `path:${entry.path}`;
}
