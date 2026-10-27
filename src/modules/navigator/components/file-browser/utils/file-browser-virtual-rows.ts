// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';

export const GRID_VIEW_PADDING_X = 0;

// FORK-MODIFICATION: 列表视图按类型分区后，分区键 / 分区变体与网格视图共用同一套，
// 免得两处各写一份字面量而漂移。旧的网格别名保留，网格侧代码一行都不用改。
export type FileBrowserSectionKey = 'dirs' | 'images' | 'videos' | 'others';
export type FileBrowserSectionVariant = 'dir' | 'image' | 'video' | 'other';

export type FileBrowserGridSectionKey = FileBrowserSectionKey;
export type FileBrowserGridEntryVariant = FileBrowserSectionVariant;

interface FileBrowserVirtualRowBase {
  key: string;
  start: number;
  size: number;
}

export interface FileBrowserListVirtualRow extends FileBrowserVirtualRowBase {
  type: 'list-entry';
  entry: DirEntry;
  entryIndex: number;
}

// FORK-MODIFICATION: 列表视图的分区标题行。它不是条目：不可点选、不进选中集合、
// 不响应条目级事件，只占一段固定高度，把不同类型的条目隔开。
export interface FileBrowserListSectionVirtualRow extends FileBrowserVirtualRowBase {
  type: 'list-section';
  sectionKey: FileBrowserSectionKey;
  variant: FileBrowserSectionVariant;
  count: number;
}

export interface FileBrowserGridSectionVirtualRow extends FileBrowserVirtualRowBase {
  type: 'grid-section';
  sectionKey: FileBrowserGridSectionKey;
  variant: FileBrowserGridEntryVariant;
  count: number;
  stickyIndex: number;
}

export interface FileBrowserGridItemsVirtualRow extends FileBrowserVirtualRowBase {
  type: 'grid-items';
  sectionKey: FileBrowserGridSectionKey;
  variant: FileBrowserGridEntryVariant;
  entries: DirEntry[];
  rowIndex: number;
}

// FORK-MODIFICATION: 列表视图的分区标题行也是虚拟行的一员，必须进联合类型，
// 否则 TS 会把 row.type === 'list-section' 判成「永不成立」而直接报错。
export type FileBrowserVirtualRow
  = FileBrowserListVirtualRow
    | FileBrowserListSectionVirtualRow
    | FileBrowserGridSectionVirtualRow
    | FileBrowserGridItemsVirtualRow;

export interface FileBrowserGridLayoutMetrics {
  gridGap: number;
  columnCount: number;
  columnWidth: number;
  availableWidth: number;
}

// FORK-MODIFICATION: 最小列宽随缩略图档位变化，这里必须由调用方传入，
// 不能给默认值 —— 默认值会让漏传的调用方静默按 medium 列数布局。
export function getGridColumnCount(
  viewportWidth: number,
  gridGap: number,
  minColumnWidth: number,
): number {
  const availableWidth = Math.max(0, viewportWidth - GRID_VIEW_PADDING_X);
  return Math.max(1, Math.floor((availableWidth + gridGap) / (minColumnWidth + gridGap)));
}

export function getVirtualRowEnd(row: FileBrowserVirtualRow): number {
  return row.start + row.size;
}

export function findVirtualRowRangeOverlappingContentSpan(
  rows: readonly FileBrowserVirtualRow[],
  contentTop: number,
  contentBottom: number,
): {
  startIndex: number;
  endIndex: number;
} {
  let startIndex = 0;
  let endIndex = rows.length - 1;
  let firstOverlappingIndex = rows.length;

  while (startIndex <= endIndex) {
    const middleIndex = Math.floor((startIndex + endIndex) / 2);

    if (getVirtualRowEnd(rows[middleIndex]) >= contentTop) {
      firstOverlappingIndex = middleIndex;
      endIndex = middleIndex - 1;
    }
    else {
      startIndex = middleIndex + 1;
    }
  }

  startIndex = firstOverlappingIndex;
  endIndex = rows.length;

  while (startIndex < endIndex) {
    const middleIndex = Math.floor((startIndex + endIndex) / 2);

    if (rows[middleIndex].start <= contentBottom) {
      startIndex = middleIndex + 1;
    }
    else {
      endIndex = middleIndex;
    }
  }

  return {
    startIndex: firstOverlappingIndex,
    endIndex: startIndex,
  };
}

export function getFileBrowserGridLayoutMetrics(
  contentWidth: number,
  gridGap: number,
  minColumnWidth: number,
): FileBrowserGridLayoutMetrics {
  const availableWidth = Math.max(0, contentWidth - GRID_VIEW_PADDING_X);
  const columnCount = getGridColumnCount(contentWidth, gridGap, minColumnWidth);
  const columnWidth = columnCount > 0
    ? (availableWidth - ((columnCount - 1) * gridGap)) / columnCount
    : availableWidth;

  return {
    gridGap,
    columnCount,
    columnWidth,
    availableWidth,
  };
}
