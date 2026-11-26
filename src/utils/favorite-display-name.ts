// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 37（2026-11-22）—— 收藏条目的**显示名**（别名）。
//
// 别名只影响收藏界面上的显示，**绝不**改原始文件名 / 目录名，也不改 path。
// 仪表盘收藏卡与侧边栏「快速访问」两处都必须显示同一个名字，所以这段判断
// 抽成一个纯函数：两处各自写一遍，早晚会长歪（一边忘了 trim、一边忘了空串回退）。
//
// 回退链条保持原样：别名 → getPathDisplayName(path) → path 本身。

import { getPathDisplayName } from '@/utils/normalize-path';

/**
 * 算出收藏条目在界面上显示的名字。
 *
 * @param path 条目的真实路径（打开收藏用的就是它）
 * @param alias 用户自定义的显示名；空串 / null / undefined 表示没有别名
 * @param translate i18n 翻译函数，交给 getPathDisplayName 用
 */
export function resolveFavoriteDisplayName(
  path: string,
  alias: string | null | undefined,
  translate?: (key: string) => string,
): string {
  const trimmedAlias = alias?.trim() ?? '';

  if (trimmedAlias) {
    return trimmedAlias;
  }

  if (!path) {
    return '';
  }

  return getPathDisplayName(path, translate) || path;
}
