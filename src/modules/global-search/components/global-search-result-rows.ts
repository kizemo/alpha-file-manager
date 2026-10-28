// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';

/** 搜索结果按「根目录」分组后的一个组。 */
export interface GlobalSearchResultGroup {
  driveRoot: string;
  entries: DirEntry[];
}

/** 键盘导航的方向。 */
export type GlobalSearchNavigateDirection = 'next' | 'previous' | 'first' | 'last';

/**
 * 把条目路径归到它所属的根目录。
 *
 * 盘符路径（`X:/...`）归到 `X:/`；其余按路径的第一段归组。
 * 口径与批次 C1 之前的 `getDriveRoot` 完全一致，避免出现「分组方式变了」的差异。
 */
export function getGlobalSearchDriveRoot(path: string): string {
  if (/^[a-zA-Z]:/.test(path)) {
    return path.substring(0, 2).toUpperCase() + '/';
  }

  const parts = path.split('/').filter(Boolean);

  if (parts.length > 0) {
    return '/' + parts[0];
  }

  return '/';
}

/**
 * 按根目录分组：组内保持原始顺序，组间按根目录字典序。
 */
export function groupGlobalSearchResults(entries: readonly DirEntry[]): GlobalSearchResultGroup[] {
  const groups = new Map<string, DirEntry[]>();

  for (const entry of entries) {
    const driveRoot = getGlobalSearchDriveRoot(entry.path);
    const existing = groups.get(driveRoot);

    if (existing) {
      existing.push(entry);
    }
    else {
      groups.set(driveRoot, [entry]);
    }
  }

  return Array.from(groups.entries())
    .sort(([rootA], [rootB]) => rootA.localeCompare(rootB))
    .map(([driveRoot, groupEntries]) => ({
      driveRoot,
      entries: groupEntries,
    }));
}

/**
 * 行的 key：只用下标。
 *
 * 拿路径当 key 会让每条结果都持有一份长字符串；同一组里的顺序本来就稳定，下标足够。
 */
export function getGlobalSearchResultRowKey(index: number): string {
  return String(index);
}

/** 普通单击：把选中替换成这一项。 */
export function applyGlobalSearchSingleSelection(entry: DirEntry): DirEntry[] {
  return [entry];
}

/** Ctrl+单击：在当前选中里切换这一项。 */
export function applyGlobalSearchToggleSelection(
  currentEntries: readonly DirEntry[],
  entry: DirEntry,
): DirEntry[] {
  const isSelected = currentEntries.some(item => item.path === entry.path);

  if (isSelected) {
    return currentEntries.filter(item => item.path !== entry.path);
  }

  return [...currentEntries, entry];
}

/** 找到条目在列表里的下标；找不到返回 -1。 */
export function findGlobalSearchResultIndex(entries: readonly DirEntry[], path: string): number {
  return entries.findIndex(entry => entry.path === path);
}

/**
 * Shift+单击 / Shift+方向键：把锚点到目标之间的条目整段选中。
 *
 * 锚点无效（没有锚点或锚点已被移出列表）时返回空数组 ——
 * 由调用方决定回退成「只选中目标」。
 */
export function resolveGlobalSearchRange(
  entries: readonly DirEntry[],
  anchorIndex: number,
  targetIndex: number,
): DirEntry[] {
  if (entries.length === 0 || anchorIndex < 0 || targetIndex < 0) {
    return [];
  }

  const start = Math.max(0, Math.min(anchorIndex, targetIndex));
  const end = Math.min(entries.length - 1, Math.max(anchorIndex, targetIndex));

  return entries.slice(start, end + 1);
}
