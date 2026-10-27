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

// FORK-MODIFICATION: 目录 / 图片 / 视频 / 其它文件的先后顺序由这一个函数说了算。
// 网格视图委托它，列表视图也用它，于是两边的分组顺序永远一致。
export function getFileBrowserSectionEntryOrder(entries: readonly DirEntry[]): DirEntry[] {
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
