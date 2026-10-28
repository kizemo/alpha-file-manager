// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';
import normalizePath from '@/utils/normalize-path';
import {
  groupFileBrowserEntries,
  isFileBrowserImageEntry,
  isFileBrowserVideoEntry,
} from '../file-browser-entry-groups';
import { resolveThumbnailMetrics } from './file-browser-thumbnail-sizes';
import type { ThumbnailSize } from '../types';

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
  /**
   * FORK-MODIFICATION（批次 C3）：这一行在**整个网格**里的序号（跨分区连续编号）。
   *
   * 以前是「分区内的行号」，两个分区都从 0 开始 —— 同一个 rowIndex 在两个分区里
   * 会撞车，所以行 key 必须再拼 sectionKey 才唯一。现在改成全局连续编号，
   * rowIndex 本身就是全局唯一的，行 key 直接用它（`grid-items:${rowIndex}`）。
   */
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

// FORK-MODIFICATION（批次 C3）：列表视图的分区标题行高度。
// 它必须与渲染侧（file-browser-list-view.vue 的 .file-browser-list-view__section）
// 同源：区隔行内联 height = 40px（上下各 2px padding + 36px 的段头 bar），
// 虚拟模型按同一个数累加 offset —— 两边对不上就会滚动错位。
export const LIST_SECTION_HEADER_HEIGHT = 40;

// FORK-MODIFICATION（批次 C3）：网格段头行高度（与渲染侧同源）。
export const GRID_SECTION_HEADER_HEIGHT = 42;

// FORK-MODIFICATION（批次 C3）：网格段头的 z-index 层级，按分区固定。
const GRID_SECTION_STICKY_INDEXES: Record<FileBrowserGridSectionKey, number> = {
  dirs: 10,
  images: 11,
  videos: 12,
  others: 13,
};

const ROW_KIND_LIST_ENTRY = 0;
const ROW_KIND_LIST_SECTION = 1;
const ROW_KIND_GRID_SECTION = 2;
const ROW_KIND_GRID_ITEMS = 3;

// FORK-MODIFICATION（批次 C3）：最小列宽随缩略图档位变化，这里必须由调用方传入，
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

// ---------------------------------------------------------------------------
// FORK-MODIFICATION（批次 C3）：行模型（紧凑元数据 + 按需物化）
// ---------------------------------------------------------------------------
//
// 改造前的契约：`rows` 是一个 computed，对**全部条目**物化行对象
// （每个条目一个对象 + 一条含完整路径的新字符串 key）。
// DOM 侧早就是虚拟的，但 JS 堆仍然是 O(N) —— 十万条目的目录就是十万个行对象
// 加十万条路径字符串。
//
// 现在的契约分成两层：
//   ① **元数据**（FileBrowserVirtualRowModel）：O(N) 但**紧凑数值型** ——
//      偏移 / 高度 / 类型 / 条目下标全部放在 TypedArray 里，外加一份条目引用数组。
//      这里**没有**任何行对象，也**没有**任何含路径的字符串。
//   ② **行对象**：只在「可视窗口」里按需物化（materializeVirtualRow(s)），
//      物化出来的对象用完即可回收。
//
// 行 key 也随之改成下标：列表行 `list:${entryIndex}`、网格行 `grid-items:${rowIndex}`，
// 区隔行 `list-section:${sectionKey}` / `grid-section:${sectionKey}`。
// 以前 `list:${entry.path}` / `grid-items:${key}:${rowIndex}:${paths.join('|')}`
// 每行都要新建一条含完整路径的字符串 —— 那是 O(N) × 路径长度 的常驻内存。

export interface FileBrowserVirtualRowSection {
  key: FileBrowserSectionKey;
  variant: FileBrowserSectionVariant;
  /** 该分区在「显示顺序」里的条目起始下标 */
  entryStart: number;
  /** 该分区的条目数 */
  entryCount: number;
  /** 段头行在行序列里的下标（-1 = 没有段头行） */
  headerRowIndex: number;
  /** 网格：该分区第一条 grid-items 行在行序列里的下标 */
  itemRowStart: number;
  /** 网格：该分区的 grid-items 行数 */
  itemRowCount: number;
  /** 网格：该分区第一条 grid-items 行在「网格行序」里的序号 */
  gridOrdinalStart: number;
  /** 网格段头的 z-index 层级 */
  stickyIndex: number;
}

export interface FileBrowserVirtualRowModel {
  layout: 'list' | 'grid' | 'tree' | undefined;
  /** 行总数（区隔行也算一行） */
  rowCount: number;
  /** 每行的起始偏移（虚拟内容坐标） */
  starts: Float64Array;
  /** 每行的高度 */
  sizes: Float64Array;
  /** 每行的类型（见 ROW_KIND_*） */
  kinds: Uint8Array;
  /** 每行所属分区下标（-1 = 无） */
  sectionIndexes: Int32Array;
  /** 每行覆盖的条目在显示顺序里的起始下标 */
  entryStarts: Int32Array;
  /** 每行覆盖的条目数 */
  entryCounts: Int32Array;
  /** 显示顺序下的条目（目录 → 图片 → 视频 → 其它；tree 布局保持原顺序） */
  displayEntries: DirEntry[];
  /** 分区定义（按显示顺序） */
  sections: FileBrowserVirtualRowSection[];
  /** 网格列数 */
  columnCount: number;
  /** 虚拟内容总高（最后一行 start + size） */
  totalSize: number;
  /** FORK-MODIFICATION（批次 C3）：已物化的行对象数量（诊断 / 测试用） */
  materializedRowCount: number;
  /** 路径 → 显示顺序下标（惰性构建，只在真的按路径定位时才建） */
  pathIndex: Map<string, number> | null;
  /** 规范化路径 → 显示顺序下标（惰性构建，只在原始路径没命中时才建） */
  normalizedPathIndex: Map<string, number> | null;
}

/**
 * 列表行高：有缩略图（图片 / 视频）才跟着档位走。
 *
 * FORK-MODIFICATION: 反馈 4（2026-10-15）。判据必须与渲染侧一致：
 * file-browser-list-entry-thumbnail.vue 里 `isMedia` 就是
 * `isFileBrowserImageEntry || isFileBrowserVideoEntry`，非媒体时那个组件渲染空，
 * 行里只剩一个 19px 的文件图标。拿别的谓词（比如「不是目录」）当判据，
 * 会把非媒体文件也当成有缩略图而行高照样长 —— 那正好是用户报的现象。
 *
 * 与 CSS 的 `--navigator-list-view-entry-height` 也必须同源：
 * 那一行是 `min-height`，若它还随档位变，虚拟行给 64 而 min-height 给 96，
 * 元素会撑到 96 并压到下一个虚拟行槽位上，滚动就会错位。
 */
function getEntryDescriptionHeight(
  entry: DirEntry,
  entryDescription?: (entry: DirEntry) => string | undefined,
  thumbnailSize: ThumbnailSize = 'medium',
): number {
  const metrics = resolveThumbnailMetrics(thumbnailSize);
  const hasThumbnail = isFileBrowserImageEntry(entry) || isFileBrowserVideoEntry(entry);

  const rowHeight = hasThumbnail ? metrics.listRowHeight : metrics.listFileEntryHeight;
  const rowHeightWithDescription = hasThumbnail
    ? metrics.listRowHeightWithDescription
    : metrics.listFileEntryHeightWithDescription;

  return entryDescription?.(entry) ? rowHeightWithDescription : rowHeight;
}

export interface FileBrowserVirtualRowModelOptions {
  entries: readonly DirEntry[];
  layout: 'list' | 'grid' | 'tree' | undefined;
  /** 网格列数（其它布局忽略） */
  columnCount: number;
  /** 网格卡片之间的间距（其它布局忽略） */
  gridGap: number;
  thumbnailSize: ThumbnailSize;
  entryDescription?: (entry: DirEntry) => string | undefined;
}

interface MutableSectionBuilder {
  key: FileBrowserSectionKey;
  variant: FileBrowserSectionVariant;
  entries: DirEntry[];
  entryHeight: number;
}

/**
 * 分区定义。
 *
 * 网格布局下四个分区的行高**不一样**：目录卡与非媒体文件卡固定高
 * （反馈 2 / 反馈 4：内容尺寸不变，盒子不该跟着档位长高），
 * 只有媒体卡跟缩略图档位走。所以高度必须按分区给，不能一个数套四个分区。
 * 列表布局用不到这个高度（列表行高由 getEntryDescriptionHeight 决定）。
 */
function createSectionBuilders(
  entries: readonly DirEntry[],
  metrics: ReturnType<typeof resolveThumbnailMetrics>,
): MutableSectionBuilder[] {
  const groupedEntries = groupFileBrowserEntries(entries);

  const builders: MutableSectionBuilder[] = [
    {
      key: 'dirs',
      variant: 'dir',
      entries: groupedEntries.dirs,
      entryHeight: metrics.gridDirEntryHeight,
    },
    {
      key: 'images',
      variant: 'image',
      entries: groupedEntries.images,
      entryHeight: metrics.gridEntryHeight,
    },
    {
      key: 'videos',
      variant: 'video',
      entries: groupedEntries.videos,
      entryHeight: metrics.gridEntryHeight,
    },
    {
      key: 'others',
      variant: 'other',
      entries: groupedEntries.others,
      entryHeight: metrics.gridFileEntryHeight,
    },
  ];

  return builders.filter(section => section.entries.length > 0);
}

/**
 * 建立行模型。
 *
 * 只分配紧凑的数值元数据（TypedArray）+ 一份条目引用数组，
 * **不为任何条目分配行对象**。
 */
export function createFileBrowserVirtualRowModel(
  options: FileBrowserVirtualRowModelOptions,
): FileBrowserVirtualRowModel {
  const { layout } = options;
  const metrics = resolveThumbnailMetrics(options.thumbnailSize);

  if (layout === 'grid') {
    return createGridRowModel(options, metrics);
  }

  if (layout === 'list') {
    return createGroupedListRowModel(options, metrics);
  }

  return createFlatListRowModel(options);
}

// tree 布局：平铺，没有分区，也没有段头行。
function createFlatListRowModel(
  options: FileBrowserVirtualRowModelOptions,
): FileBrowserVirtualRowModel {
  const { entries } = options;
  const rowCount = entries.length;
  const starts = new Float64Array(rowCount);
  const sizes = new Float64Array(rowCount);
  const kinds = new Uint8Array(rowCount);
  const sectionIndexes = new Int32Array(rowCount);
  const entryStarts = new Int32Array(rowCount);
  const entryCounts = new Int32Array(rowCount);
  let offset = 0;

  for (let index = 0; index < rowCount; index += 1) {
    const size = getEntryDescriptionHeight(
      entries[index],
      options.entryDescription,
      options.thumbnailSize,
    );

    kinds[index] = ROW_KIND_LIST_ENTRY;
    sectionIndexes[index] = -1;
    entryStarts[index] = index;
    entryCounts[index] = 1;
    starts[index] = offset;
    sizes[index] = size;
    offset += size;
  }

  return {
    layout: options.layout,
    rowCount,
    starts,
    sizes,
    kinds,
    sectionIndexes,
    entryStarts,
    entryCounts,
    displayEntries: entries as DirEntry[],
    sections: [],
    columnCount: options.columnCount,
    totalSize: offset,
    materializedRowCount: 0,
    pathIndex: null,
    normalizedPathIndex: null,
  };
}

// 列表布局：按类型分区，每个非空分组前面插一行区隔标题行。
// 条目行的 entryIndex 用「显示顺序里的序号」，与未分区时的语义保持一致
// （它表示这个条目排在第几个，而不是它在原始数组里的下标）。
function createGroupedListRowModel(
  options: FileBrowserVirtualRowModelOptions,
  metrics: ReturnType<typeof resolveThumbnailMetrics>,
): FileBrowserVirtualRowModel {
  const sectionBuilders = createSectionBuilders(options.entries, metrics);
  const displayEntries: DirEntry[] = [];
  const sections: FileBrowserVirtualRowSection[] = [];

  for (const builder of sectionBuilders) {
    sections.push({
      key: builder.key,
      variant: builder.variant,
      entryStart: displayEntries.length,
      entryCount: builder.entries.length,
      headerRowIndex: -1,
      itemRowStart: -1,
      itemRowCount: 0,
      gridOrdinalStart: 0,
      stickyIndex: GRID_SECTION_STICKY_INDEXES[builder.key],
    });

    // 不用 push(...entries)：十万条目的目录会撞上「展开调用参数过多」的栈上限。
    for (const entry of builder.entries) {
      displayEntries.push(entry);
    }
  }

  const rowCount = displayEntries.length + sections.length;
  const starts = new Float64Array(rowCount);
  const sizes = new Float64Array(rowCount);
  const kinds = new Uint8Array(rowCount);
  const sectionIndexes = new Int32Array(rowCount);
  const entryStarts = new Int32Array(rowCount);
  const entryCounts = new Int32Array(rowCount);
  let offset = 0;
  let rowIndex = 0;

  for (let sectionIndex = 0; sectionIndex < sections.length; sectionIndex += 1) {
    const section = sections[sectionIndex];
    const builder = sectionBuilders[sectionIndex];

    section.headerRowIndex = rowIndex;
    kinds[rowIndex] = ROW_KIND_LIST_SECTION;
    sectionIndexes[rowIndex] = sectionIndex;
    entryStarts[rowIndex] = -1;
    entryCounts[rowIndex] = 0;
    starts[rowIndex] = offset;
    sizes[rowIndex] = LIST_SECTION_HEADER_HEIGHT;
    offset += LIST_SECTION_HEADER_HEIGHT;
    rowIndex += 1;

    for (let index = 0; index < builder.entries.length; index += 1) {
      const size = getEntryDescriptionHeight(
        builder.entries[index],
        options.entryDescription,
        options.thumbnailSize,
      );

      kinds[rowIndex] = ROW_KIND_LIST_ENTRY;
      sectionIndexes[rowIndex] = sectionIndex;
      entryStarts[rowIndex] = section.entryStart + index;
      entryCounts[rowIndex] = 1;
      starts[rowIndex] = offset;
      sizes[rowIndex] = size;
      offset += size;
      rowIndex += 1;
    }
  }

  return {
    layout: options.layout,
    rowCount,
    starts,
    sizes,
    kinds,
    sectionIndexes,
    entryStarts,
    entryCounts,
    displayEntries,
    sections,
    columnCount: options.columnCount,
    totalSize: offset,
    materializedRowCount: 0,
    pathIndex: null,
    normalizedPathIndex: null,
  };
}

// 网格布局：每个分区一个段头行 + 若干 grid-items 行。
function createGridRowModel(
  options: FileBrowserVirtualRowModelOptions,
  metrics: ReturnType<typeof resolveThumbnailMetrics>,
): FileBrowserVirtualRowModel {
  const columnCount = Math.max(1, options.columnCount);
  const sectionBuilders = createSectionBuilders(options.entries, metrics);
  const displayEntries: DirEntry[] = [];
  const sections: FileBrowserVirtualRowSection[] = [];

  for (const builder of sectionBuilders) {
    sections.push({
      key: builder.key,
      variant: builder.variant,
      entryStart: displayEntries.length,
      entryCount: builder.entries.length,
      headerRowIndex: -1,
      itemRowStart: -1,
      itemRowCount: 0,
      gridOrdinalStart: 0,
      stickyIndex: GRID_SECTION_STICKY_INDEXES[builder.key],
    });

    // 不用 push(...entries)：十万条目的目录会撞上「展开调用参数过多」的栈上限。
    for (const entry of builder.entries) {
      displayEntries.push(entry);
    }
  }

  let rowCount = 0;
  let gridOrdinal = 0;

  for (let sectionIndex = 0; sectionIndex < sections.length; sectionIndex += 1) {
    const section = sections[sectionIndex];
    const builder = sectionBuilders[sectionIndex];

    section.gridOrdinalStart = gridOrdinal;
    section.itemRowCount = Math.ceil(builder.entries.length / columnCount);
    gridOrdinal += section.itemRowCount;
    rowCount += 1 + section.itemRowCount;
  }

  const starts = new Float64Array(rowCount);
  const sizes = new Float64Array(rowCount);
  const kinds = new Uint8Array(rowCount);
  const sectionIndexes = new Int32Array(rowCount);
  const entryStarts = new Int32Array(rowCount);
  const entryCounts = new Int32Array(rowCount);
  let offset = 0;
  let rowIndex = 0;

  for (let sectionIndex = 0; sectionIndex < sections.length; sectionIndex += 1) {
    const section = sections[sectionIndex];
    const builder = sectionBuilders[sectionIndex];

    section.headerRowIndex = rowIndex;
    kinds[rowIndex] = ROW_KIND_GRID_SECTION;
    sectionIndexes[rowIndex] = sectionIndex;
    entryStarts[rowIndex] = -1;
    entryCounts[rowIndex] = 0;
    starts[rowIndex] = offset;
    sizes[rowIndex] = GRID_SECTION_HEADER_HEIGHT + options.gridGap;
    offset += GRID_SECTION_HEADER_HEIGHT + options.gridGap;
    rowIndex += 1;

    section.itemRowStart = rowIndex;

    for (let index = 0; index < builder.entries.length; index += columnCount) {
      const count = Math.min(columnCount, builder.entries.length - index);

      kinds[rowIndex] = ROW_KIND_GRID_ITEMS;
      sectionIndexes[rowIndex] = sectionIndex;
      entryStarts[rowIndex] = section.entryStart + index;
      entryCounts[rowIndex] = count;
      starts[rowIndex] = offset;
      sizes[rowIndex] = builder.entryHeight + options.gridGap;
      offset += builder.entryHeight + options.gridGap;
      rowIndex += 1;
    }
  }

  return {
    layout: options.layout,
    rowCount,
    starts,
    sizes,
    kinds,
    sectionIndexes,
    entryStarts,
    entryCounts,
    displayEntries,
    sections,
    columnCount,
    totalSize: offset,
    materializedRowCount: 0,
    pathIndex: null,
    normalizedPathIndex: null,
  };
}

/**
 * 物化一行。
 *
 * 只有可视窗口（以及少量按需定位）会走到这里，所以行对象的数量与
 * 「可视行数」同阶，而不是与条目数同阶。
 */
export function materializeVirtualRow(
  model: FileBrowserVirtualRowModel,
  index: number,
): FileBrowserVirtualRow | undefined {
  if (!Number.isInteger(index) || index < 0 || index >= model.rowCount) {
    return undefined;
  }

  model.materializedRowCount += 1;

  const kind = model.kinds[index];
  const start = model.starts[index];
  const size = model.sizes[index];
  const sectionIndex = model.sectionIndexes[index];
  const section = sectionIndex >= 0 ? model.sections[sectionIndex] : undefined;

  if (kind === ROW_KIND_LIST_ENTRY) {
    const entryIndex = model.entryStarts[index];

    return {
      type: 'list-entry',
      key: `list:${entryIndex}`,
      entry: model.displayEntries[entryIndex],
      entryIndex,
      start,
      size,
    };
  }

  if (kind === ROW_KIND_LIST_SECTION && section) {
    return {
      type: 'list-section',
      key: `list-section:${section.key}`,
      sectionKey: section.key,
      variant: section.variant,
      count: section.entryCount,
      start,
      size,
    };
  }

  if (kind === ROW_KIND_GRID_SECTION && section) {
    return {
      type: 'grid-section',
      key: `grid-section:${section.key}`,
      sectionKey: section.key,
      variant: section.variant,
      count: section.entryCount,
      stickyIndex: section.stickyIndex,
      start,
      size,
    };
  }

  if (kind === ROW_KIND_GRID_ITEMS && section) {
    const entryStart = model.entryStarts[index];
    const entryCount = model.entryCounts[index];
    const rowIndex = section.gridOrdinalStart
      + Math.floor((entryStart - section.entryStart) / model.columnCount);

    return {
      type: 'grid-items',
      key: `grid-items:${rowIndex}`,
      sectionKey: section.key,
      variant: section.variant,
      entries: model.displayEntries.slice(entryStart, entryStart + entryCount),
      rowIndex,
      start,
      size,
    };
  }

  return undefined;
}

/** 物化一段连续的行（可视窗口）。 */
export function materializeVirtualRows(
  model: FileBrowserVirtualRowModel,
  startIndex: number,
  endIndex: number,
): FileBrowserVirtualRow[] {
  const rows: FileBrowserVirtualRow[] = [];
  const from = Math.max(0, Math.trunc(startIndex));
  const to = Math.min(model.rowCount, Math.trunc(endIndex));

  for (let index = from; index < to; index += 1) {
    const row = materializeVirtualRow(model, index);

    if (row) {
      rows.push(row);
    }
  }

  return rows;
}

/**
 * 可视窗口计算。
 *
 * 与 use-vertical-virtual-list 的 computeVerticalVirtualRange **逐字等价**，
 * 区别只在于它直接在模型的 TypedArray 上二分，不再为每行分配 {start, size} 对象。
 */
export function computeVirtualRowRange(
  model: FileBrowserVirtualRowModel,
  options: {
    overscanPx: number;
    scrollTop: number;
    viewportHeight: number;
  },
): {
  start: number;
  end: number;
} {
  const { starts, sizes, rowCount } = model;

  if (rowCount === 0) {
    return {
      start: 0,
      end: 0,
    };
  }

  const viewportHeight = Math.max(0, options.viewportHeight);
  const maxScrollTop = Math.max(0, model.totalSize - viewportHeight);
  const effectiveScrollTop = Math.min(Math.max(0, options.scrollTop), maxScrollTop);
  const viewportStart = Math.max(0, effectiveScrollTop - options.overscanPx);
  const viewportEnd = effectiveScrollTop + viewportHeight + options.overscanPx;
  let startSearchIndex = 0;
  let startSearchEnd = rowCount;

  while (startSearchIndex < startSearchEnd) {
    const middleIndex = Math.floor((startSearchIndex + startSearchEnd) / 2);

    if (starts[middleIndex] + sizes[middleIndex] <= viewportStart) {
      startSearchIndex = middleIndex + 1;
    }
    else {
      startSearchEnd = middleIndex;
    }
  }

  const rangeStart = startSearchIndex;
  let endSearchIndex = rangeStart;
  let endSearchEnd = rowCount;

  while (endSearchIndex < endSearchEnd) {
    const middleIndex = Math.floor((endSearchIndex + endSearchEnd) / 2);

    if (starts[middleIndex] < viewportEnd) {
      endSearchIndex = middleIndex + 1;
    }
    else {
      endSearchEnd = middleIndex;
    }
  }

  return {
    start: rangeStart,
    end: Math.max(rangeStart, endSearchIndex),
  };
}

/**
 * 找出覆盖给定内容区间的行区间。
 *
 * 两条路径：
 *   ① 行模型（批次 C3 之后）：直接在 TypedArray 上二分，一个行对象都不物化；
 *   ② 普通数组：保持旧实现，供不经过模型的调用方使用。
 */
export function findVirtualRowRangeOverlappingContentSpan(
  rows: readonly FileBrowserVirtualRow[],
  contentTop: number,
  contentBottom: number,
): {
  startIndex: number;
  endIndex: number;
} {
  const model = getFileBrowserVirtualRowModel(rows);

  if (model) {
    return findVirtualRowRangeOverlappingContentSpanInModel(model, contentTop, contentBottom);
  }

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

function findVirtualRowRangeOverlappingContentSpanInModel(
  model: FileBrowserVirtualRowModel,
  contentTop: number,
  contentBottom: number,
): {
  startIndex: number;
  endIndex: number;
} {
  const { starts, sizes, rowCount } = model;

  if (rowCount === 0) {
    return {
      startIndex: 0,
      endIndex: 0,
    };
  }

  let startIndex = 0;
  let endIndex = rowCount - 1;
  let firstOverlappingIndex = rowCount;

  while (startIndex <= endIndex) {
    const middleIndex = Math.floor((startIndex + endIndex) / 2);

    if (starts[middleIndex] + sizes[middleIndex] >= contentTop) {
      firstOverlappingIndex = middleIndex;
      endIndex = middleIndex - 1;
    }
    else {
      startIndex = middleIndex + 1;
    }
  }

  startIndex = firstOverlappingIndex;
  endIndex = rowCount;

  while (startIndex < endIndex) {
    const middleIndex = Math.floor((startIndex + endIndex) / 2);

    if (starts[middleIndex] <= contentBottom) {
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

/** 找出显示顺序里落在给定区间内的分区。 */
function findSectionByDisplayIndex(
  model: FileBrowserVirtualRowModel,
  displayIndex: number,
): FileBrowserVirtualRowSection | undefined {
  for (const section of model.sections) {
    if (displayIndex >= section.entryStart && displayIndex < section.entryStart + section.entryCount) {
      return section;
    }
  }

  return undefined;
}

/**
 * 路径 → 显示顺序下标。
 *
 * 原始路径索引惰性构建：只有真的需要「按路径定位」时才建那张表，
 * 而且键直接复用条目上已有的 path 字符串（不新建字符串）。
 * 原始路径没命中时（比如分隔符写法不同）才补建一张规范化路径索引。
 */
export function findVirtualRowDisplayIndex(
  model: FileBrowserVirtualRowModel,
  path: string,
): number | undefined {
  if (!model.pathIndex) {
    const index = new Map<string, number>();

    for (let displayIndex = 0; displayIndex < model.displayEntries.length; displayIndex += 1) {
      const entryPath = model.displayEntries[displayIndex].path;

      if (!index.has(entryPath)) {
        index.set(entryPath, displayIndex);
      }
    }

    model.pathIndex = index;
  }

  const direct = model.pathIndex.get(path);

  if (direct !== undefined) {
    return direct;
  }

  if (!model.normalizedPathIndex) {
    const index = new Map<string, number>();

    for (let displayIndex = 0; displayIndex < model.displayEntries.length; displayIndex += 1) {
      const normalized = normalizePath(model.displayEntries[displayIndex].path);

      if (!index.has(normalized)) {
        index.set(normalized, displayIndex);
      }
    }

    model.normalizedPathIndex = index;
  }

  return model.normalizedPathIndex.get(normalizePath(path));
}

/** 显示顺序下标 → 行序列下标。 */
export function resolveVirtualRowIndexByDisplayIndex(
  model: FileBrowserVirtualRowModel,
  displayIndex: number,
): number | undefined {
  const section = findSectionByDisplayIndex(model, displayIndex);

  if (!section) {
    // tree 布局：平铺，条目行下标就是显示顺序下标。
    return model.layout === 'grid' ? undefined : displayIndex;
  }

  if (model.layout === 'grid') {
    return section.itemRowStart
      + Math.floor((displayIndex - section.entryStart) / model.columnCount);
  }

  if (section.headerRowIndex >= 0) {
    return section.headerRowIndex + 1 + (displayIndex - section.entryStart);
  }

  return displayIndex;
}

/** 网格行序 → 行序列下标。 */
function resolveGridRowIndexByOrdinal(
  model: FileBrowserVirtualRowModel,
  ordinal: number,
): number | undefined {
  for (const section of model.sections) {
    if (section.itemRowCount <= 0) {
      continue;
    }

    if (ordinal >= section.gridOrdinalStart
      && ordinal < section.gridOrdinalStart + section.itemRowCount) {
      return section.itemRowStart + (ordinal - section.gridOrdinalStart);
    }
  }

  return undefined;
}

/**
 * 网格里的上下导航：按列走到相邻的 grid-items 行。
 *
 * 语义与改造前完全一致（跨分区连续编号，按列号对齐，末行不足时取最后一列）。
 */
export function getFileBrowserGridNavigationEntry(
  model: FileBrowserVirtualRowModel,
  path: string,
  direction: 'up' | 'down',
): DirEntry | null {
  if (model.layout !== 'grid') {
    return null;
  }

  const displayIndex = findVirtualRowDisplayIndex(model, path);

  if (displayIndex === undefined) {
    return null;
  }

  const section = findSectionByDisplayIndex(model, displayIndex);

  if (!section) {
    return null;
  }

  const indexInSection = displayIndex - section.entryStart;
  const columnIndex = indexInSection % model.columnCount;
  const currentOrdinal = section.gridOrdinalStart
    + Math.floor(indexInSection / model.columnCount);
  const targetOrdinal = direction === 'down' ? currentOrdinal + 1 : currentOrdinal - 1;
  const targetRowIndex = resolveGridRowIndexByOrdinal(model, targetOrdinal);

  if (targetRowIndex === undefined) {
    return null;
  }

  const targetRow = materializeVirtualRow(model, targetRowIndex);

  if (!targetRow || targetRow.type !== 'grid-items') {
    return null;
  }

  return targetRow.entries[Math.min(columnIndex, targetRow.entries.length - 1)] ?? null;
}

// ---------------------------------------------------------------------------
// FORK-MODIFICATION（批次 C3）：惰性行数组
// ---------------------------------------------------------------------------

const FILE_BROWSER_VIRTUAL_ROW_MODEL = Symbol('fileBrowserVirtualRowModel');

/**
 * 取出行数组背后的模型（普通数组返回 null）。
 *
 * 有了它，框选命中测试这类只需要「行下标 → 偏移/高度」的调用方
 * 就能走数值路径，一个行对象都不用物化。
 */
export function getFileBrowserVirtualRowModel(
  rows: readonly FileBrowserVirtualRow[] | null | undefined,
): FileBrowserVirtualRowModel | null {
  if (!rows || typeof rows !== 'object') {
    return null;
  }

  const model = (rows as unknown as Record<symbol, unknown>)[FILE_BROWSER_VIRTUAL_ROW_MODEL];

  return (model as FileBrowserVirtualRowModel | undefined) ?? null;
}

/**
 * 造一个「惰性行数组」。
 *
 * 它在类型上仍然是 FileBrowserVirtualRow[]，所以框选、命中测试这些
 * 既有调用方一行都不用改；但行对象只在**真的被读到**时才物化：
 *   · length      → 行总数（不物化）
 *   · rows[i]     → 只物化第 i 行
 *   · slice(a, b) → 只物化 [a, b)
 *   · 其余数组方法 → 兜底整体物化后调用原生实现（语义正确，但会物化全部）
 */
export function createLazyVirtualRowArray(
  model: FileBrowserVirtualRowModel,
  materialize: (index: number) => FileBrowserVirtualRow | undefined,
): FileBrowserVirtualRow[] {
  const target: FileBrowserVirtualRow[] = [];
  let allRows: FileBrowserVirtualRow[] | null = null;

  // FORK-MODIFICATION（批次 C3）：这里**故意不做缓存**。
  // 缓存会把「已经滚过去的行」一直留在堆里 —— 用户从头滚到尾，
  // 缓存就长成 O(N) 个行对象，正好把这次改造省下来的东西又还回去。
  // 行对象的身份稳定性由 Vue 的 computed 保证（visibleRows 在依赖不变时
  // 返回同一个数组、同样的行对象），不需要额外缓存。
  function getRow(index: number): FileBrowserVirtualRow | undefined {
    if (!Number.isInteger(index) || index < 0 || index >= model.rowCount) {
      return undefined;
    }

    return materialize(index);
  }

  function getAllRows(): FileBrowserVirtualRow[] {
    if (allRows) {
      return allRows;
    }

    const rows: FileBrowserVirtualRow[] = [];

    for (let index = 0; index < model.rowCount; index += 1) {
      const row = getRow(index);

      if (row) {
        rows.push(row);
      }
    }

    allRows = rows;
    return rows;
  }

  function sliceRows(start?: number, end?: number): FileBrowserVirtualRow[] {
    const length = model.rowCount;
    let from = start === undefined ? 0 : Math.trunc(start);

    if (from < 0) {
      from = Math.max(0, length + from);
    }

    let to = end === undefined ? length : Math.trunc(end);

    if (to < 0) {
      to = Math.max(0, length + to);
    }

    to = Math.min(to, length);

    const rows: FileBrowserVirtualRow[] = [];

    for (let index = from; index < to; index += 1) {
      const row = getRow(index);

      if (row) {
        rows.push(row);
      }
    }

    return rows;
  }

  return new Proxy(target, {
    get(proxyTarget, property, receiver) {
      if (property === 'length') {
        return model.rowCount;
      }

      if (property === FILE_BROWSER_VIRTUAL_ROW_MODEL) {
        return model;
      }

      if (property === 'slice') {
        return sliceRows;
      }

      if (property === Symbol.iterator) {
        return function* iterateLazyRows(): Generator<FileBrowserVirtualRow> {
          for (let index = 0; index < model.rowCount; index += 1) {
            const row = getRow(index);

            if (row) {
              yield row;
            }
          }
        };
      }

      if (typeof property === 'string') {
        const index = Number(property);

        if (Number.isInteger(index) && index >= 0) {
          return getRow(index);
        }
      }

      const value = Reflect.get(proxyTarget, property, receiver);

      if (typeof value === 'function') {
        return (...args: unknown[]) => value.apply(getAllRows(), args);
      }

      return value;
    },
    has(proxyTarget, property) {
      if (property === FILE_BROWSER_VIRTUAL_ROW_MODEL) {
        return true;
      }

      if (typeof property === 'string') {
        const index = Number(property);

        if (Number.isInteger(index) && index >= 0) {
          return index < model.rowCount;
        }
      }

      return Reflect.has(proxyTarget, property);
    },
  }) as FileBrowserVirtualRow[];
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
