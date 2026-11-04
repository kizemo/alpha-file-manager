// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';
import type { ContextMenuAction, ContextMenuItemConfig, EntryType, SelectionType } from '@/modules/navigator/components/file-browser/types';
import type { ContextMenuActionInput } from '@/core/actions/context-menu-adapter';
import { isActionBlockedByEntryPolicy } from '@/utils/entry-action-policy';
import { canDisconnectDriveEntry } from '@/utils/drive-disconnect-policy';
// FORK-MODIFICATION: 远程四动作的可见性判定（不进上面那张表）。
import {
  isRemoteDeleteActionVisible,
  isRemoteTransferActionVisible,
  type RemoteContextMenuAction,
  type RemoteTransferVisibilityOptions,
} from '@/modules/remote/utils/remote-context-menu-visibility';

const CONTEXT_MENU_ITEMS: ContextMenuItemConfig[] = [
  {
    action: 'rename',
    selectionTypes: ['single'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'copy',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'cut',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'link',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'paste',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'delete',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'open-with',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'properties',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'quick-view',
    selectionTypes: ['single'],
    entryTypes: ['file'],
  },
  {
    action: 'print',
    selectionTypes: ['single'],
    entryTypes: ['file'],
  },
  {
    action: 'share',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'open-in-new-tab',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['directory'],
  },
  {
    action: 'toggle-favorite',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'edit-tags',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  {
    action: 'copy-path',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file', 'directory'],
  },
  // 批量改名只在多选时出现：单文件改名走上面的 `rename`，那是既有路径。
  // 目录混在多选里也能改名 —— 批量改名的执行器对目录与文件一视同仁。
  {
    action: 'batch-rename',
    selectionTypes: ['multiple'],
    entryTypes: ['file', 'directory'],
  },
  // FORK-MODIFICATION: 「打开文件所在目录」仅 file 类型可见（对目录无意义——目录即所在目录本身）
  {
    action: 'open-containing-directory',
    selectionTypes: ['single', 'multiple'],
    entryTypes: ['file'],
  },
  // FORK-MODIFICATION: 应用内「打开所属目录」——跳到父目录并选中该条目。
  // 仅在搜索结果视图里出现：那里条目的父目录不在当前视图内，这个动作才有意义；
  // 普通目录视图里父目录就是当前目录，跳过去等于什么都不做。
  // 文件和目录都允许：目录的父目录同样需要「跳过去并选中它」。
  {
    action: 'open-containing-directory-in-app',
    selectionTypes: ['single'],
    entryTypes: ['file', 'directory'],
  },
];

const WINDOWS_ONLY_ACTIONS = new Set<ContextMenuAction>(['properties', 'open-containing-directory']);

// FORK-MODIFICATION 2026-10-24：「解除文件锁定」与「彻底删除」。
//
// 这两个动作背后是 Windows 特有的能力（Restart Manager 查占用者、
// delete-pending 标记删除、重启后删除），在别的平台上**没有对应实现**，
// 所以只在 Windows 上出现 —— 与 `properties` 的处理方式一致。
const FILE_LOCK_ACTIONS = new Set<ContextMenuAction>(['unlock', 'force-delete']);

// FORK-MODIFICATION: 只在搜索结果视图里出现的动作。见上方 `open-containing-directory-in-app` 的说明。
const SEARCH_RESULTS_ONLY_ACTIONS = new Set<ContextMenuAction>(['open-containing-directory-in-app']);

function getSelectionStats(entries: DirEntry[]) {
  const selectionType: SelectionType = entries.length === 1 ? 'single' : 'multiple';
  const entryTypes = new Set<EntryType>();

  for (const entry of entries) {
    entryTypes.add(entry.is_dir ? 'directory' : 'file');
  }

  return {
    selectionType,
    entryTypes: Array.from(entryTypes),
    hasDirectories: entryTypes.has('directory'),
    hasFiles: entryTypes.has('file'),
    isMixed: entryTypes.size > 1,
  };
}

export function isContextMenuActionVisible(
  action: ContextMenuAction,
  entries: DirEntry[],
  options?: {
    platform?: string | null;
    disableDestructiveActions?: boolean;
    /** FORK-MODIFICATION: 当前 FileBrowser 是否在渲染搜索结果（全局搜索或目录内过滤）。 */
    isSearchResultsView?: boolean;
  },
): boolean {
  if (entries.length === 0) {
    return false;
  }

  if (action === 'delete-permanently') {
    return isContextMenuActionVisible('delete', entries, options);
  }

  // FORK-MODIFICATION 2026-10-24：解除锁定 / 彻底删除。
  //
  // ① **只对 Windows 开放**（底层是 Windows 特有的 API）；
  // ② **只对单个条目开放** —— 这两个动作都作用在「一个路径」上，
  //    后端的 `unlock_file` / `force_delete_path` 签名就是单路径。
  //
  // ⚠️ 这里**必须自己跑一遍策略门禁**（`isActionBlockedByEntryPolicy`），
  // 因为下面那个通用门禁在本分支之前就 return 了 ——
  // 忘了这一句的后果是「远程条目 / 虚拟位置 / 盘根」上也会冒出这两个入口。
  if (FILE_LOCK_ACTIONS.has(action)) {
    if (options?.platform !== 'windows') {
      return false;
    }

    if (entries.length !== 1) {
      return false;
    }

    if (isActionBlockedByEntryPolicy(action, entries, options?.platform ?? null, options)) {
      return false;
    }

    return true;
  }

  if (action === 'disconnect') {
    if (entries.length !== 1 || !entries[0].is_dir) {
      return false;
    }

    return canDisconnectDriveEntry(entries[0], options?.platform ?? null);
  }

  const config = CONTEXT_MENU_ITEMS.find(item => item.action === action);

  if (!config) {
    return false;
  }

  const stats = getSelectionStats(entries);
  const platform = options?.platform ?? null;

  const matchesSelectionType = config.selectionTypes.includes(stats.selectionType);

  if (!matchesSelectionType) {
    return false;
  }

  const allEntriesMatchAllowedTypes = stats.entryTypes.every(
    entryType => config.entryTypes.includes(entryType),
  );

  if (!allEntriesMatchAllowedTypes) {
    return false;
  }

  if (WINDOWS_ONLY_ACTIONS.has(action) && platform !== 'windows') {
    return false;
  }

  // FORK-MODIFICATION: 搜索结果专属动作在普通目录视图里必须隐藏
  if (SEARCH_RESULTS_ONLY_ACTIONS.has(action) && !options?.isSearchResultsView) {
    return false;
  }

  if (isActionBlockedByEntryPolicy(action, entries, platform, options)) {
    return false;
  }

  return true;
}

export function getContextMenuSelectionStats(entries: DirEntry[]) {
  return getSelectionStats(entries);
}

// ---------------------------------------------------------------------------
// 批次 3：右键菜单接动作注册表
//
// 下面这一段把**既有**的可见性判定包成动作注册表的 `when` 谓词。
// 判定逻辑本身一个字没改 —— 只是从「界面层逐个问 isActionVisible」改成
// 「注册表算一遍，界面层读投影」。
// ---------------------------------------------------------------------------

/**
 * 右键菜单（含顶部 quick-actions 图标条）会渲染的动作。
 *
 * 这是「菜单里有哪些动作」的**唯一事实源**：注册表的声明与界面渲染都对着它，
 * 判据 A（菜单投影与注册表一致）就是拿它当基准。
 *
 * ⚠️ 远程四动作（`upload-to-remote` / `download-to-local` /
 * `remote-permissions` / `remote-delete`）**故意不在**上面的
 * `CONTEXT_MENU_ITEMS` 表里：它们的前提是「对面还有一格」或
 * 「选中的全是远程条目」，与条目自身类别无关。
 */
export const CONTEXT_MENU_ACTION_IDS: readonly ContextMenuAction[] = [
  // 顶部 quick-actions 图标条
  'rename',
  'batch-rename',
  'copy',
  'cut',
  'link',
  'paste',
  'delete',
  'remote-delete',
  // 菜单项
  'open-with',
  'quick-view',
  'print',
  'open-in-new-tab',
  'open-containing-directory',
  'open-containing-directory-in-app',
  'disconnect',
  'copy-path',
  'unlock',
  'force-delete',
  'upload-to-remote',
  'download-to-local',
  'remote-permissions',
  'share',
  'properties',
  'toggle-favorite',
  'edit-tags',
  // 「删除」下拉里的第二项
  'delete-permanently',
  // 「新建」子菜单里的两项
  'create-file',
  'create-directory',
];

/**
 * 右键菜单可见性判定需要的全部上下文。
 *
 * 既有判定分散在好几个地方（条目策略表、Windows 专属集合、搜索结果专属集合、
 * 远程传输上下文、剪贴板状态、平台能力），这里把它们收成一份快照，
 * 让注册表的谓词只读这一份快照 —— 一次渲染里所有动作看到的是同一份事实。
 */
export type ContextMenuActionVisibilityContext = {
  /** 当前选中的条目。 */
  entries: DirEntry[];
  /** 当前平台（`windows` / `linux` / `macos`）。 */
  platform: string | null;
  /** 是否禁用破坏性动作（盘根等场景）。 */
  disableDestructiveActions?: boolean;
  /** 当前是否在渲染搜索结果视图。 */
  isSearchResultsView?: boolean;
  /** 远程传输动作需要的上下文（对面那一格 + 选中条目的协议）。 */
  remoteTransfer?: RemoteTransferVisibilityOptions;
  /** 剪贴板能不能粘到选中的目录（既有 `canPasteToSelectedDirectory`）。 */
  canPaste?: boolean;
  /** 当前平台能不能建链接（既有 `linkCreationOptions.length > 0`）。 */
  canCreateLink?: boolean;
  /** 目标目录能不能「新建空文件」（既有 `canCreateFile`）。 */
  canCreateFile?: boolean;
  /** 有没有可用的「新建」目标（选中了目录，且不是虚拟位置）。 */
  hasCreateTarget?: boolean;
};

/** 远程专属的四个动作。 */
export function isRemoteContextMenuAction(
  action: ContextMenuAction,
): action is RemoteContextMenuAction {
  return action === 'upload-to-remote'
    || action === 'download-to-local'
    || action === 'remote-permissions'
    || action === 'remote-delete';
}

/**
 * 这个动作在当前上下文里能不能用。
 *
 * ⚠️ **判定语义与批次 3 之前逐字等价**：
 *
 * - `paste` 的入口条件一直是「剪贴板能粘到选中的目录」，**不看**上面那张表；
 * - `link` 一直是「表里允许」**且**「当前平台有可用的链接方式」；
 * - 远程四动作走 `remote-context-menu-visibility.ts`；
 * - 其余动作一律走 `isContextMenuActionVisible`（表 + Windows 专属 +
 *   搜索结果专属 + 条目策略门禁）。
 */
export function isContextMenuActionAvailable(
  action: ContextMenuAction,
  context: ContextMenuActionVisibilityContext,
): boolean {
  if (action === 'paste') {
    return context.canPaste === true;
  }

  if (action === 'link') {
    return context.canCreateLink !== false
      && isContextMenuActionVisible(action, context.entries, {
        platform: context.platform,
        disableDestructiveActions: context.disableDestructiveActions,
        isSearchResultsView: context.isSearchResultsView,
      });
  }

  if (action === 'create-file') {
    return context.hasCreateTarget === true && context.canCreateFile !== false;
  }

  if (action === 'create-directory') {
    return context.hasCreateTarget === true;
  }

  if (isRemoteContextMenuAction(action)) {
    // FORK-MODIFICATION：远程删除的判定只看「选中的全是远程条目」，
    // 不需要「对面那一格」的上下文，所以单独走一支。
    if (action === 'remote-delete') {
      return isRemoteDeleteActionVisible(context.entries);
    }

    return isRemoteTransferActionVisible(action, context.entries, context.remoteTransfer ?? {
      hasRemoteCounterpart: false,
      hasLocalCounterpart: false,
    });
  }

  return isContextMenuActionVisible(action, context.entries, {
    platform: context.platform,
    disableDestructiveActions: context.disableDestructiveActions,
    isSearchResultsView: context.isSearchResultsView,
  });
}

/**
 * 把既有可见性判定包成动作注册表的声明。
 *
 * 每条声明的 `when` 就是「这个动作现在能不能用」，
 * `hideWhenUnavailable` 保持右键菜单的既有语义：**不适用就不显示**。
 *
 * 「显示但置灰」的能力在 `core/actions/context-menu-adapter.ts` 里，
 * 需要置灰的动作把 `hideWhenUnavailable` 传 false 即可。
 */
export function buildContextMenuActionDeclarations(
  context: ContextMenuActionVisibilityContext,
): ContextMenuActionInput[] {
  return CONTEXT_MENU_ACTION_IDS.map(action => ({
    id: action,
    title: action,
    when: () => isContextMenuActionAvailable(action, context),
    hideWhenUnavailable: true,
  }));
}
