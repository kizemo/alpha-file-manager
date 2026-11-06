// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { computed, type Ref } from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import type { ContextMenuAction } from '@/modules/navigator/components/file-browser/types';
import type { ActionAvailability } from '@/core/actions/types';
import { usePlatformStore } from '@/stores/runtime/platform';
import { useClipboardStore } from '@/stores/runtime/clipboard';
import {
  getContextMenuSelectionStats,
  buildContextMenuActionDeclarations,
} from '@/modules/navigator/components/file-browser/utils/context-menu-action-visibility';
// FORK-MODIFICATION: 传输类动作（上传 / 下载 / 权限设置）的上下文。
import { resolveSelectionProtocol } from '@/modules/remote/utils/remote-context-menu-visibility';
import { useRemoteTransferTargets } from '@/modules/remote/utils/remote-transfer-targets';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import { isSelectionVirtualLocation } from '@/utils/entry-action-policy';
// FORK-MODIFICATION 2026-10-20: 远程「新建文件」是永久非目标(规格 §1)。
import { hasRemoteTarget } from '@/modules/remote/utils/remote-write';
import { getAvailableLinkCreationOptions } from '@/utils/link-operations';
import {
  buildContextMenuActionRegistry,
  getContextMenuActionEntries,
  type ContextMenuActionEntry,
} from '@/core/actions/context-menu-adapter';
import { getActionContext } from '@/core/actions/predicates';

/**
 * 右键菜单里一个动作的投影状态。
 *
 * - `enabled` —— 显示，可点击；
 * - `disabled` —— **显示但置灰**；
 * - `hidden` —— 不显示。
 */
export type ContextMenuActionState = ActionAvailability;

export function useContextMenuItems(
  selectedEntries: Ref<DirEntry[]>,
  options?: {
    disableDestructiveActions?: Ref<boolean>;
    /** FORK-MODIFICATION: 供 `SEARCH_RESULTS_ONLY_ACTIONS` 判定当前是否在搜索结果视图。 */
    isSearchResultsView?: Ref<boolean>;
  },
) {
  const platformStore = usePlatformStore();
  const remoteConnectionsStore = useRemoteConnectionsStore();
  const clipboardStore = useClipboardStore();
  const transferTargets = useRemoteTransferTargets();

  const selectionStats = computed(() => getContextMenuSelectionStats(selectedEntries.value));

  /**
   * 传输类动作需要的上下文。
   *
   * 「对面那一格在哪儿」来自模块级登记表（由导航器写入），
   * 「选中条目的连接支不支持改权限」来自连接表。
   */
  const remoteTransferContext = computed(() => {
    const entries = selectedEntries.value;

    return {
      hasRemoteCounterpart: Boolean(transferTargets.remotePanePath.value),
      hasLocalCounterpart: Boolean(transferTargets.localPanePath.value),
      selectionProtocol: resolveSelectionProtocol(
        entries,
        connectionId => remoteConnectionsStore.getConnection(connectionId)?.protocol,
      ),
    };
  });

  const hasVirtualLocationSelection = computed(() => {
    return isSelectionVirtualLocation(selectedEntries.value);
  });

  const selectedDirectory = computed(() => {
    return selectedEntries.value.find(entry => entry.is_dir);
  });

  /**
   * 剪贴板能不能粘到选中的目录。
   *
   * 与批次 3 之前的 `canPasteToSelectedDirectory` 逐字等价 ——
   * 「粘贴」入口一直只看剪贴板状态，不看 `CONTEXT_MENU_ITEMS` 那张表。
   */
  const canPaste = computed(() => {
    if (hasVirtualLocationSelection.value || !clipboardStore.hasItems || !selectedDirectory.value) {
      return false;
    }

    return clipboardStore.canPasteTo(selectedDirectory.value.path);
  });

  /**
   * FORK-MODIFICATION 2026-11-06：能不能把剪贴板内容以**快捷方式**粘到选中的目录。
   *
   * 与 `canPaste` 只差一处：剪贴板里必须是**文件项** —— 图片剪贴板做不出 `.lnk`。
   */
  const canPasteShortcut = computed(() => {
    if (hasVirtualLocationSelection.value || !clipboardStore.hasFileItems || !selectedDirectory.value) {
      return false;
    }

    return clipboardStore.canPasteTo(selectedDirectory.value.path);
  });

  /** 当前平台可用的链接创建方式（快捷方式 / 符号链接 …）。 */
  const linkCreationOptions = computed(() =>
    getAvailableLinkCreationOptions(selectedEntries.value, platformStore.currentPlatform),
  );

  /** 能不能建链接（既有 `linkCreationOptions.length > 0`）。 */
  const canCreateLink = computed(() => linkCreationOptions.value.length > 0);

  /**
   * FORK-MODIFICATION 2026-10-20：目标目录能不能「新建空文件」。
   *
   * 目标就是**选中的那些目录**（右键菜单的「新建」建在选中目录里），
   * 没有选中目录时菜单本身不出现。
   *
   * **远程一律不行** —— 规格 §1 的永久非目标。
   */
  const canCreateFile = computed(() => {
    const targets = selectedEntries.value
      .filter(entry => entry.is_dir)
      .map(entry => entry.path);

    return !hasRemoteTarget(targets);
  });

  /** 有没有「新建」的落点（选中了目录，且不是虚拟位置）。 */
  const hasCreateTarget = computed(() => {
    return selectionStats.value.hasDirectories && !hasVirtualLocationSelection.value;
  });

  /**
   * 动作注册表 → 右键菜单条目投影。
   *
   * 注册表每次都按当前快照重建（与批次 1 的「不是全局单例」口径一致），
   * 界面层只读投影结果，不再逐个动作问「你现在能不能用」。
   */
  const menuActions = computed<ContextMenuActionEntry[]>(() => {
    const registry = buildContextMenuActionRegistry({
      actions: buildContextMenuActionDeclarations({
        entries: selectedEntries.value,
        platform: platformStore.currentPlatform,
        disableDestructiveActions: options?.disableDestructiveActions?.value,
        isSearchResultsView: options?.isSearchResultsView?.value,
        remoteTransfer: remoteTransferContext.value,
        canPaste: canPaste.value,
        canPasteShortcut: canPasteShortcut.value,
        canCreateLink: canCreateLink.value,
        canCreateFile: canCreateFile.value,
        hasCreateTarget: hasCreateTarget.value,
      }),
    });

    // 菜单层不依赖路由：动作谓词只看上面那份快照。
    return getContextMenuActionEntries(registry, getActionContext(null));
  });

  const menuActionStates = computed(() => {
    const states = new Map<string, ContextMenuActionState>();

    for (const entry of menuActions.value) {
      states.set(entry.actionId, entry.availability);
    }

    return states;
  });

  function getMenuActionState(action: ContextMenuAction): ContextMenuActionState {
    return menuActionStates.value.get(action) ?? 'hidden';
  }

  /** 动作该不该显示（`disabled` 也要显示，只是置灰）。 */
  function isMenuActionVisible(action: ContextMenuAction): boolean {
    return getMenuActionState(action) !== 'hidden';
  }

  /** 动作是不是「显示但置灰」。 */
  function isMenuActionDisabled(action: ContextMenuAction): boolean {
    return getMenuActionState(action) === 'disabled';
  }

  return {
    selectionStats,
    canPaste,
    canPasteShortcut,
    canCreateFile,
    canCreateLink,
    linkCreationOptions,
    hasVirtualLocationSelection,
    menuActions,
    getMenuActionState,
    isMenuActionVisible,
    isMenuActionDisabled,
  };
}
