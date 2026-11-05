// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { Router } from 'vue-router';
import type { DirEntry } from '@/types/dir-entry';
import { revealInExplorer } from '@/utils/open-in-explorer';
import { openNavigatorRevealEntry } from '@/utils/open-navigator-directory';
// FORK-MODIFICATION (2026-11-03)：L2 —— 默认资源管理器时改走应用内定位。
import { isDefaultFileManagerSafe } from '@/utils/default-file-manager';

/**
 * FORK-MODIFICATION (2026-11-05)：「打开文件所在目录」的共享执行逻辑。
 *
 * 为什么抽出来：导航器右键菜单与全局搜索结果面板的右键菜单都要这个动作，
 * 但两边的执行上下文完全不同 —— 导航器那边是 `useFileBrowserActions`
 * （拿得到 `contextMenu` 状态与过滤词），搜索面板那边是独立组件。
 * 抽成共享 util 之后，两边走的是**同一条**定位路径，不会再出现
 * 「导航器里能定位、搜索结果里定位不了」的分裂。
 */
export interface RevealContainingDirectoryOptions {
  /** 路由实例：应用内定位要靠它切到导航器那一格。 */
  router: Router;
  /** 当前过滤词（交接给拆栏后的右格）。 */
  filterQuery?: string;
  /** 定位失败时的回调 —— 由调用方决定怎么提示用户（界面层才有 i18n 与 toast）。 */
  onError?: (error: unknown) => void;
}

/**
 * 在原生 Windows Explorer 里定位选中文件（`explorer /select`）。
 *
 * - AFM 自己就是默认资源管理器时，**不**把用户踢到系统 Explorer ——
 *   改为在 AFM 内部跳到父目录并选中该条目（2026-11-03 默认资源管理器改造 L2）。
 * - 目录条目直接跳过：目录的父目录就是所在目录本身，这个动作对它没有意义
 *   （可见性规则本来就只对 file 开放，这里再兜一层底）。
 */
export async function revealEntriesContainingDirectory(
  entries: DirEntry[],
  options: RevealContainingDirectoryOptions,
): Promise<void> {
  const useInAppReveal = await isDefaultFileManagerSafe();
  const filterQuery = options.filterQuery ?? '';

  for (const entry of entries) {
    if (entry.is_dir) {
      continue;
    }

    if (useInAppReveal) {
      openNavigatorRevealEntry(options.router, entry.path, {
        filterQuery,
        reuseCurrentPaneOnly: true,
      });

      continue;
    }

    try {
      await revealInExplorer(entry.path);
    }
    catch (error: unknown) {
      options.onError?.(error);
    }
  }
}
