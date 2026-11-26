// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 2026-10-31 —— 收藏夹排序的纯函数。
//
// 为什么单独抽出来：排序的**语义**（拖到目标前面还是后面、能不能跨分类）
// 一旦散在组件里，就会出现「拖拽写一份数据、上下按钮写另一份数据」的
// 经典分裂。抽成纯函数后，组件只负责把「指针落在哪里」翻译成
// before / after，两条交互路径最终调用的是同一份语义。

import type { FavoriteCategory, FavoriteItem } from '@/types/user-stats';

/**
 * 拖放落点相对目标项的位置。
 *
 * `before` = 插到目标项前面，`after` = 插到目标项后面。
 */
export type DropPlacement = 'before' | 'after';

/**
 * 把 `draggedIndex` 处的元素挪到 `targetIndex` 处那个元素的前面 / 后面。
 *
 * 用的是「摘出来再插进去」而不是「整体换位」：整体换位只能把元素挪一格，
 * 而拖拽要能一次跨过好几项。
 */
export function reorderByDrop<T>(
  items: T[],
  draggedIndex: number,
  targetIndex: number,
  placement: DropPlacement,
): T[] {
  if (draggedIndex < 0 || targetIndex < 0 || draggedIndex >= items.length || targetIndex >= items.length) {
    return items;
  }

  if (draggedIndex === targetIndex) {
    return items;
  }

  const next = [...items];
  const [moved] = next.splice(draggedIndex, 1);

  // 被摘走的元素原本排在目标前面时，目标的下标会跟着前移一位。
  const adjustedTargetIndex = draggedIndex < targetIndex ? targetIndex - 1 : targetIndex;
  const insertIndex = placement === 'before' ? adjustedTargetIndex : adjustedTargetIndex + 1;

  next.splice(insertIndex, 0, moved);

  return next;
}

/**
 * 拖拽重排**某个分类内**的收藏条目。
 *
 * 两条硬约束：
 *   1. 只在同一个 `categoryId` 内换位。跨分类移动已经有「移到分类」下拉，
 *      拖拽若也跨分类，两条路径的语义会打架（一个改归属、一个只改顺序）。
 *   2. 别的分类的条目**相对顺序**保持不变 —— `favorites` 是一张扁平表，
 *      界面按 `categoryId` 过滤后展示，所以只有同类目内部的相对顺序
 *      才是用户看得见的顺序。
 */
export function reorderFavoritesByDrop(
  favorites: FavoriteItem[],
  draggedPath: string,
  targetPath: string,
  placement: DropPlacement = 'before',
): FavoriteItem[] {
  const draggedIndex = favorites.findIndex(item => item.path === draggedPath);
  const targetIndex = favorites.findIndex(item => item.path === targetPath);

  if (draggedIndex === -1 || targetIndex === -1) {
    return favorites;
  }

  if (favorites[draggedIndex].categoryId !== favorites[targetIndex].categoryId) {
    return favorites;
  }

  return reorderByDrop(favorites, draggedIndex, targetIndex, placement);
}

/**
 * 拖拽重排收藏分类。分类数组的顺序就是界面上的分区顺序。
 */
export function reorderFavoriteCategoriesByDrop(
  categories: FavoriteCategory[],
  draggedId: string,
  targetId: string,
  placement: DropPlacement = 'before',
): FavoriteCategory[] {
  const draggedIndex = categories.findIndex(category => category.id === draggedId);
  const targetIndex = categories.findIndex(category => category.id === targetId);

  if (draggedIndex === -1 || targetIndex === -1) {
    return categories;
  }

  return reorderByDrop(categories, draggedIndex, targetIndex, placement);
}
