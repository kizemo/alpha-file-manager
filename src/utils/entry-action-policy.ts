// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';
import type { ContextMenuAction } from '@/modules/navigator/components/file-browser/types';
import { isProtectedSystemPath } from '@/utils/is-protected-system-path';
import { isVirtualDirectoryPath } from '@/utils/virtual-path-constants';
import { isRemotePath } from '@/utils/remote-paths';

export type EntryActionPolicy = 'virtual-location' | 'remote' | 'system-mount-root' | 'normal';

const VIRTUAL_LOCATION_BLOCKED_ACTIONS = new Set<ContextMenuAction>([
  'rename',
  'batch-rename',
  'copy',
  'cut',
  'link',
  'paste',
  'delete',
  'delete-permanently',
  // FORK-MODIFICATION 2026-10-24：解除锁定 / 彻底删除对虚拟位置也是非目标
  // —— 虚拟位置不是真实路径，本地那套 Win32 手段对它没有意义。
  'unlock',
  'force-delete',
  'open-with',
  'properties',
  'quick-view',
  'print',
  'share',
  // FORK-MODIFICATION: 图片编辑四动作对虚拟位置也是非目标 ——
  // 虚拟位置不是真实文件路径，后端那套「原地写回」对它没有意义。
  'rotate-image-left',
  'rotate-image-right',
  'flip-image-horizontal',
  'flip-image-vertical',
]);

const SYSTEM_MOUNT_ROOT_BLOCKED_ACTIONS = new Set<ContextMenuAction>([
  'rename',
  'batch-rename',
  'cut',
  'link',
  'delete',
  'delete-permanently',
  // FORK-MODIFICATION 2026-10-24：盘根本身不能「解锁」也不能「彻底删除」——
  // 那等于「把这个盘删了」。
  'unlock',
  'force-delete',
]);

/**
 * 远程空间在阶段 0 的门禁：**只放行导航类动作**。
 *
 * 阶段 0 还没有传输与远程写操作，放行它们会让用户点到没接线的按钮。
 * 阶段 3（新建目录 / 重命名 / 删除）与阶段 7（跨边界拖拽 / 复制粘贴）
 * 会**逐条从这里移出去** —— 每移一条，同步更新
 * `entry-action-policy.test.ts` 里那条「阶段 0 只放行导航」的断言。
 *
 * FORK-MODIFICATION 2026-10-20：远程**删除**走的是另一个动作
 * `remote-delete`，它**不在**这个集合里，所以远程条目可以有删除入口。
 * 本地的 `delete` / `delete-permanently` 继续对远程屏蔽 ——
 * 远程没有回收箱，「移至垃圾箱」在远程是假选项。
 *
 * FORK-MODIFICATION 2026-10-20（阶段 3 收尾）：两条改动 ——
 *
 * - **`rename` 移出**：远程重命名已实现（规格 §5「重命名 ✅」）。
 *   `batch-rename` **留在**集合里：批量改名是另一件事，阶段 6/7 再议。
 * - **`create-file` 移入**：规格 §1 把「在远程目录里新建空文件」列为
 *   **永久非目标** —— SFTP 没有 `CREATE`，FTP 的 `STOR` 需要先有本地文件。
 *   所以这一条**不是「暂时没做」，是「永远不做」**。
 *   `create-directory` **不在**这个集合里：阶段 3 已实现远程新建目录。
 * FORK-MODIFICATION 2026-11-11（T9 真机复测）：**`copy` 移出**这个集合。
 *
 * 用户反馈「重新扫码登录夸克后，网盘条目右键没有复制项、Ctrl+C 无效」。
 * 门禁把 `copy` 当阶段 0 未实现而屏蔽 ⇒ 「网盘里复制 → 本地窗格粘贴（= 下载到本地）」
 * 这条最自然的跨面板路径整个不存在。复制的落地链路是真实存在的
 * （`copyItems` → 应用内剪贴板 → `handleRemotePaste` → `downloadItemsFromRemote`），
 * 所以这里放行。
 *
 * 边界：只放 `'copy'`。`'cut'` 的语义是**远程移动**（后端未接线）、
 * 远程目录内的 `'paste'` 会往网盘写文件（同样未接线）⇒ 两条继续屏蔽，
 * 放出去就是用户点得到却没反应的假按钮。
 */
const REMOTE_PHASE0_BLOCKED_ACTIONS = new Set<ContextMenuAction>([
  'batch-rename',
  'cut',
  'link',
  'paste',
  'delete',
  'delete-permanently',
  // FORK-MODIFICATION 2026-10-24：远程服务器上的文件锁**不在本机**，
  // 本地的 Restart Manager / delete-pending 对它完全没有意义 ⇒ 一律屏蔽。
  'unlock',
  'force-delete',
  'create-file',
  'open-with',
  'properties',
  'quick-view',
  'print',
  'share',
  // FORK-MODIFICATION: 远程服务器上的图片不能原地旋转 / 翻转 ——
  // 那是本地文件写操作，远程写操作走的是另一套通道（本轮未实现）。
  'rotate-image-left',
  'rotate-image-right',
  'flip-image-horizontal',
  'flip-image-vertical',
]);

export function getEntryActionPolicy(path: string, platform: string | null): EntryActionPolicy {
  // `remote` 必须排在最前。当前 `sfm://remote/...` 既不匹配 `isVirtualDirectoryPath`
  // 也不匹配 `isProtectedSystemPath`，会一路落到 `normal` —— 那个策略对
  // delete / rename / copy **全部放行**，正是最不能落到的地方。
  // 后续阶段 1 把 `sfm://remote` 接进 `isVirtualDirectoryPath` 之后，
  // 这个顺序也不会打架。
  if (isRemotePath(path)) {
    return 'remote';
  }

  if (isVirtualDirectoryPath(path)) {
    return 'virtual-location';
  }

  if (isProtectedSystemPath(path, platform)) {
    return 'system-mount-root';
  }

  return 'normal';
}

export function isSelectionVirtualLocation(entries: DirEntry[]): boolean {
  return entries.some(entry => isVirtualDirectoryPath(entry.path));
}

export function isActionBlockedByEntryPolicy(
  action: ContextMenuAction,
  entries: DirEntry[],
  platform: string | null,
  options?: {
    disableDestructiveActions?: boolean;
  },
): boolean {
  if (options?.disableDestructiveActions && SYSTEM_MOUNT_ROOT_BLOCKED_ACTIONS.has(action)) {
    return true;
  }

  // FORK-MODIFICATION 2026-11-11（T9）：远程 `copy` 的放行**仅限整批都是远程**。
  //
  // 本轮把 `'copy'` 从 `REMOTE_PHASE0_BLOCKED_ACTIONS` 移出后，本地 + 远程混选
  // 会顺手得到「复制」—— 而混选的落地链路说不清（本地走 CF_HDROP、
  // 远程走 `downloadItemsFromRemote`），且 `syncToSystemClipboard` 只认真实路径。
  // 与该文件既有的「混选取最严格策略」（规格 §5）保守语义保持一致 ⇒ 混选继续屏蔽。
  if (action === 'copy') {
    const policies = entries.map(entry => getEntryActionPolicy(entry.path, platform));

    if (policies.includes('remote') && !policies.every(policy => policy === 'remote')) {
      return true;
    }
  }

  for (const entry of entries) {
    const policy = getEntryActionPolicy(entry.path, platform);

    if (policy === 'virtual-location' && VIRTUAL_LOCATION_BLOCKED_ACTIONS.has(action)) {
      return true;
    }

    if (policy === 'remote' && REMOTE_PHASE0_BLOCKED_ACTIONS.has(action)) {
      return true;
    }

    if (policy === 'system-mount-root' && SYSTEM_MOUNT_ROOT_BLOCKED_ACTIONS.has(action)) {
      return true;
    }
  }

  return false;
}
