// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { FILE_EXTENSIONS } from '@/constants';
import type { DirEntry } from '@/types/dir-entry';
import type { GroupedEntries } from './types';

export function isFileBrowserImageEntry(entry: DirEntry): boolean {
  if (entry.is_dir) return false;
  const extension = entry.ext?.toLowerCase();
  return extension ? FILE_EXTENSIONS.IMAGE.includes(extension) : false;
}

export function isFileBrowserVideoEntry(entry: DirEntry): boolean {
  if (entry.is_dir) return false;
  const extension = entry.ext?.toLowerCase();
  return extension ? FILE_EXTENSIONS.VIDEO.includes(extension) : false;
}

export function groupFileBrowserEntries(entries: readonly DirEntry[]): GroupedEntries {
  const dirs: DirEntry[] = [];
  const images: DirEntry[] = [];
  const videos: DirEntry[] = [];
  const others: DirEntry[] = [];

  for (const entry of entries) {
    if (entry.is_dir) {
      dirs.push(entry);
    }
    else if (isFileBrowserImageEntry(entry)) {
      images.push(entry);
    }
    else if (isFileBrowserVideoEntry(entry)) {
      videos.push(entry);
    }
    else {
      others.push(entry);
    }
  }

  return {
    dirs,
    images,
    videos,
    others,
  };
}

// ---------------------------------------------------------------------------
// FORK-MODIFICATION（2026-11-08，用户需求 1）：「位置」页按「分区 / 网盘 / 远程」分组。
//
// 「位置」页（`sfm://locations`）里的条目都带 `location_group` 标记，
// 普通的「目录 / 图片 / 视频 / 其它」分组对它是**错的** ——
// 分区里的盘符与网盘条目都是目录，按旧口径会全落进「文件夹」一组。
//
// 判据是「条目有没有带标记」，不是「路径像不像」：普通目录里不可能出现这个标记。
// ---------------------------------------------------------------------------

export interface FileBrowserLocationGroups {
  drives: DirEntry[];
  netdisk: DirEntry[];
  remote: DirEntry[];
}

export function hasLocationGroupEntries(entries: readonly DirEntry[]): boolean {
  return entries.some(entry => entry.location_group !== undefined && entry.location_group !== null);
}

/**
 * 按「分区 / 网盘 / 远程连接」分组。
 *
 * 没带标记的条目一律归到 `drives` —— 它们只可能出现在「位置」页里，
 * 而「位置」页的默认分组就是分区组，不会凭空多出一个「未分组」区。
 */
export function groupFileBrowserLocationEntries(entries: readonly DirEntry[]): FileBrowserLocationGroups {
  const groups: FileBrowserLocationGroups = {
    drives: [],
    netdisk: [],
    remote: [],
  };

  for (const entry of entries) {
    if (entry.location_group === 'netdisk') {
      groups.netdisk.push(entry);
    }
    else if (entry.location_group === 'remote') {
      groups.remote.push(entry);
    }
    else {
      groups.drives.push(entry);
    }
  }

  return groups;
}

// FORK-MODIFICATION: 目录 / 图片 / 视频 / 其它文件的先后顺序由这一个函数说了算。
// 网格视图委托它，列表视图也用它，于是两边的分组顺序永远一致。
export function getFileBrowserSectionEntryOrder(entries: readonly DirEntry[]): DirEntry[] {
  // 「位置」页走自己的分组口径（分区 → 网盘 → 远程连接）。
  if (hasLocationGroupEntries(entries)) {
    const locationGroups = groupFileBrowserLocationEntries(entries);

    return [
      ...locationGroups.drives,
      ...locationGroups.netdisk,
      ...locationGroups.remote,
    ];
  }

  const groupedEntries = groupFileBrowserEntries(entries);
  return [
    ...groupedEntries.dirs,
    ...groupedEntries.images,
    ...groupedEntries.videos,
    ...groupedEntries.others,
  ];
}

export function getFileBrowserGridEntryOrder(entries: readonly DirEntry[]): DirEntry[] {
  return getFileBrowserSectionEntryOrder(entries);
}

export function getFileBrowserVisualEntryOrder(
  entries: readonly DirEntry[],
  layout: 'list' | 'grid' | 'tree' | undefined,
): DirEntry[] {
  // FORK-MODIFICATION: 列表视图也分区了。键盘导航（getNavigationEntries）与
  // Shift 范围选择（getOrderedEntries）都走这里，于是「视觉顺序 / 键盘顺序 /
  // 范围选择顺序」三者天然一致，不必各自维护一份。
  // tree 布局保持平铺 —— 那是层级视图，顺序另有含义。
  if (layout === 'grid' || layout === 'list') {
    return getFileBrowserSectionEntryOrder(entries);
  }

  return [...entries];
}
