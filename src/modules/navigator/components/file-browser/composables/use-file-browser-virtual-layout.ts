// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  computed,
  nextTick,
  onBeforeUnmount,
  ref,
  watch,
  type ComponentPublicInstance,
  type ComputedRef,
} from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import normalizePath from '@/utils/normalize-path';
import { getFileBrowserGridGap } from '../utils/file-browser-layout-gaps';
import { getElementContentBoxWidth } from '../utils/file-browser-content-box';
import {
  computeVirtualRowRange,
  createFileBrowserVirtualRowModel,
  createLazyVirtualRowArray,
  findVirtualRowDisplayIndex,
  getFileBrowserGridNavigationEntry,
  getGridColumnCount,
  GRID_SECTION_HEADER_HEIGHT,
  materializeVirtualRow,
  resolveVirtualRowIndexByDisplayIndex,
  type FileBrowserGridSectionVirtualRow,
  type FileBrowserVirtualRow,
  type FileBrowserVirtualRowModel,
} from '../utils/file-browser-virtual-rows';
import { resolveThumbnailMetrics } from '../utils/file-browser-thumbnail-sizes';
import type { ThumbnailSize } from '../types';

export type {
  FileBrowserGridEntryVariant,
  FileBrowserGridItemsVirtualRow,
  FileBrowserGridSectionKey,
  FileBrowserGridSectionVirtualRow,
  FileBrowserListSectionVirtualRow,
  FileBrowserListVirtualRow,
  FileBrowserSectionKey,
  FileBrowserSectionVariant,
  FileBrowserVirtualRow,
} from '../utils/file-browser-virtual-rows';
export { getGridColumnCount } from '../utils/file-browser-virtual-rows';
// FORK-MODIFICATION（批次 C3）：行模型、区隔行高度、网格导航都搬到
// ../utils/file-browser-virtual-rows.ts（与行模型同源，避免两处各写一份数字）。
// 这里保留 re-export —— 既有调用方（框选命中测试等）是从这个路径 import 的。
export { LIST_SECTION_HEADER_HEIGHT } from '../utils/file-browser-virtual-rows';
export { getFileBrowserGridNavigationEntry } from '../utils/file-browser-virtual-rows';

const VIRTUAL_OVERSCAN_PX = 420;
const SCROLL_TO_PATH_DOM_SYNC_ATTEMPTS = 12;
const STATUS_BAR_BORDER_CLEARANCE = 2;
const SCROLL_VIEWPORT_SELECTOR = '.file-browser__scroll-area-viewport';
const ENTRIES_CONTAINER_SELECTOR = '.file-browser__entries-container';
const CONTENT_INNER_SELECTOR = '.file-browser__content-inner';
const FILE_BROWSER_SELECTOR = '.file-browser';
const STATUS_BAR_SELECTOR = '.file-browser-status-bar';
const VIRTUAL_SPACER_SELECTOR = '.file-browser-list-view__list, .file-browser-grid-view__spacer';

function resolveScrollViewportElement(element: HTMLElement): HTMLElement {
  if (element.matches(SCROLL_VIEWPORT_SELECTOR)) {
    return element;
  }

  const closestViewport = element.closest<HTMLElement>(SCROLL_VIEWPORT_SELECTOR);

  if (closestViewport) {
    return closestViewport;
  }

  return element.querySelector<HTMLElement>(SCROLL_VIEWPORT_SELECTOR) ?? element;
}

function resolveElement(element: Element | ComponentPublicInstance | null): HTMLElement | null {
  const rawElement = element instanceof HTMLElement
    ? element
    : element && '$el' in element && element.$el instanceof HTMLElement
      ? element.$el
      : null;

  return rawElement ? resolveScrollViewportElement(rawElement) : null;
}

function getRowEnd(row: FileBrowserVirtualRow): number {
  return row.start + row.size;
}

function getRowScrollMarginTop(
  row: FileBrowserVirtualRow,
  virtualContentOffset: number,
  gridGap: number,
): number {
  // FORK-MODIFICATION: 列表分区标题行与条目行同属列表流，滚动留白规则一致。
  if (row.type === 'list-entry' || row.type === 'list-section') {
    return virtualContentOffset;
  }

  return row.type === 'grid-items' ? GRID_SECTION_HEADER_HEIGHT + gridGap : 0;
}

function getStatusBarOverlap(viewport: HTMLElement, statusBar: HTMLElement | null | undefined): number {
  if (!statusBar) {
    return 0;
  }

  const viewportRect = viewport.getBoundingClientRect();
  const statusBarRect = statusBar.getBoundingClientRect();
  const overlap = Math.max(0, viewportRect.bottom - statusBarRect.top);

  return overlap > 0 ? overlap + STATUS_BAR_BORDER_CLEARANCE : 0;
}

function getVirtualContentOffset(viewport: HTMLElement): number {
  const virtualSpacer = viewport.querySelector<HTMLElement>(VIRTUAL_SPACER_SELECTOR);

  if (!virtualSpacer) {
    return 0;
  }

  return Math.max(0, virtualSpacer.getBoundingClientRect().top - viewport.getBoundingClientRect().top + viewport.scrollTop);
}

function escapeCssAttribute(value: string): string {
  return typeof CSS !== 'undefined' && CSS.escape ? CSS.escape(value) : value.replace(/"/g, '\\"');
}

export function resolveViewportContentWidth(viewport: HTMLElement): number {
  const entriesContainer = viewport.querySelector<HTMLElement>(ENTRIES_CONTAINER_SELECTOR);

  if (entriesContainer) {
    return getElementContentBoxWidth(entriesContainer);
  }

  const contentInner = viewport.querySelector<HTMLElement>(CONTENT_INNER_SELECTOR);

  if (contentInner) {
    return getElementContentBoxWidth(contentInner);
  }

  return getElementContentBoxWidth(viewport);
}

/**
 * FORK-MODIFICATION（批次 C3）：行数组入口。
 *
 * 返回的仍然是一个 `FileBrowserVirtualRow[]`，但它是**惰性**的：
 * 内部只持有紧凑的数值元数据（偏移 / 高度 / 类型 / 条目下标），
 * 行对象等到真的被读到时才物化。
 *
 * 既有调用方（框选命中测试、列表 / 网格视图）一行都不用改 ——
 * `length`、下标、`slice()` 都照旧可用。
 */
export function createFileBrowserVirtualRows(options: {
  entries: readonly DirEntry[];
  layout: 'list' | 'grid' | 'tree' | undefined;
  viewportWidth: number;
  entryDescription?: (entry: DirEntry) => string | undefined;
  increaseFileViewGaps?: boolean;
  // FORK-MODIFICATION: 缩略图大小档位（同时影响 list 行高与 grid 卡高/列宽）。默认 medium。
  thumbnailSize?: ThumbnailSize;
}): FileBrowserVirtualRow[] {
  const thumbnailSize = options.thumbnailSize ?? 'medium';
  const gridGap = getFileBrowserGridGap(!!options.increaseFileViewGaps);
  const model = createFileBrowserVirtualRowModel({
    entries: options.entries,
    layout: options.layout,
    columnCount: getGridColumnCount(
      options.viewportWidth,
      gridGap,
      resolveThumbnailMetrics(thumbnailSize).gridMinColumnWidth,
    ),
    gridGap,
    thumbnailSize,
    entryDescription: options.entryDescription,
  });

  return createLazyVirtualRowArray(model, index => materializeVirtualRow(model, index));
}

export function useFileBrowserVirtualLayout(options: {
  entries: ComputedRef<DirEntry[]>;
  layout: () => 'list' | 'grid' | 'tree' | undefined;
  entryDescription?: (entry: DirEntry) => string | undefined;
  increaseFileViewGaps?: () => boolean;
  // FORK-MODIFICATION: 缩略图大小档位（影响 list 行高与 grid 卡高/列宽）
  thumbnailSize?: () => ThumbnailSize;
}) {
  const scrollViewportRef = ref<HTMLElement | null>(null);
  const viewportHeight = ref(0);
  const viewportWidth = ref(0);
  const bottomScrollMargin = ref(0);
  const scrollTop = ref(0);
  const virtualContentOffset = ref(0);
  let viewportResizeObserver: ResizeObserver | null = null;

  const gridGap = computed(() => getFileBrowserGridGap(!!options.increaseFileViewGaps?.()));
  const gridColumnCount = computed(() => getGridColumnCount(
    viewportWidth.value,
    gridGap.value,
    resolveThumbnailMetrics(options.thumbnailSize?.() ?? 'medium').gridMinColumnWidth,
  ));

  // FORK-MODIFICATION（批次 C3）：**行模型**。
  //
  // 改造前这里直接算出「全部条目 × 全部行」的行对象数组：十万条目的目录
  // 就是十万个行对象，外加十万条含完整路径的 key 字符串。
  // 现在只分配紧凑的数值元数据（TypedArray）+ 一份条目引用数组，
  // **一个行对象都不建**；行对象留给可视窗口按需物化。
  const rowModel = computed<FileBrowserVirtualRowModel>(() => createFileBrowserVirtualRowModel({
    entries: options.entries.value,
    layout: options.layout(),
    columnCount: gridColumnCount.value,
    gridGap: gridGap.value,
    thumbnailSize: options.thumbnailSize?.() ?? 'medium',
    entryDescription: options.entryDescription,
  }));

  // 行数组：类型仍是 FileBrowserVirtualRow[]（既有调用方零改动），
  // 但行对象只在被读到时才物化。
  const rows = computed<FileBrowserVirtualRow[]>(() => {
    const model = rowModel.value;

    return createLazyVirtualRowArray(model, index => materializeVirtualRow(model, index));
  });

  const totalSize = computed(() => rowModel.value.totalSize + bottomScrollMargin.value);

  // 可视窗口：在模型的 TypedArray 上二分，与改造前
  // computeVerticalVirtualRange 的语义逐字等价（同一个 overscan / 夹取规则）。
  const visibleRange = computed(() => {
    const virtualScrollTop = Math.max(0, scrollTop.value - virtualContentOffset.value);

    return computeVirtualRowRange(rowModel.value, {
      overscanPx: VIRTUAL_OVERSCAN_PX,
      scrollTop: virtualScrollTop,
      viewportHeight: viewportHeight.value,
    });
  });

  // 只物化可视窗口这一段行对象。
  const visibleRows = computed<FileBrowserVirtualRow[]>(() => {
    const range = visibleRange.value;

    return rows.value.slice(range.start, range.end);
  });

  const activeGridSectionRow = computed<FileBrowserGridSectionVirtualRow | null>(() => {
    if (options.layout() !== 'grid') {
      return null;
    }

    const model = rowModel.value;
    const virtualScrollTop = Math.max(0, scrollTop.value - virtualContentOffset.value);
    let activeRow: FileBrowserGridSectionVirtualRow | null = null;

    // 改造前这里是把整个 rows 数组扫一遍；现在只扫分区（最多 4 个）。
    for (const section of model.sections) {
      if (section.headerRowIndex < 0) {
        continue;
      }

      const headerStart = model.starts[section.headerRowIndex];

      if (headerStart > virtualScrollTop) {
        break;
      }

      const row = materializeVirtualRow(model, section.headerRowIndex);

      if (row && row.type === 'grid-section') {
        activeRow = row;
      }
    }

    return activeRow;
  });

  const offsetY = computed(() => visibleRows.value[0]?.start ?? 0);

  const spacerStyle = computed(() => ({
    height: `${totalSize.value}px`,
  }));

  const windowStyle = computed(() => ({
    transform: `translate3d(0, ${offsetY.value}px, 0)`,
  }));

  function updateViewportSize() {
    const viewport = scrollViewportRef.value;

    if (!viewport) {
      viewportHeight.value = 0;
      viewportWidth.value = 0;
      virtualContentOffset.value = 0;
      scrollTop.value = 0;
      return;
    }

    const fileBrowser = viewport.closest<HTMLElement>(FILE_BROWSER_SELECTOR);
    const statusBar = fileBrowser?.querySelector<HTMLElement>(STATUS_BAR_SELECTOR);

    viewportHeight.value = viewport.clientHeight;
    viewportWidth.value = resolveViewportContentWidth(viewport);
    virtualContentOffset.value = getVirtualContentOffset(viewport);
    bottomScrollMargin.value = getStatusBarOverlap(viewport, statusBar);
    scrollTop.value = Math.max(0, viewport.scrollTop);
  }

  function disconnectViewportResizeObserver() {
    viewportResizeObserver?.disconnect();
    viewportResizeObserver = null;
  }

  function observeViewportSize() {
    disconnectViewportResizeObserver();

    const viewport = scrollViewportRef.value;

    if (!viewport || typeof ResizeObserver === 'undefined') {
      updateViewportSize();
      return;
    }

    viewportResizeObserver = new ResizeObserver(updateViewportSize);
    viewportResizeObserver.observe(viewport);

    const contentInner = viewport.querySelector<HTMLElement>(CONTENT_INNER_SELECTOR);

    if (contentInner) {
      viewportResizeObserver.observe(contentInner);
    }

    updateViewportSize();
  }

  function scheduleViewportSizeUpdate() {
    nextTick(() => {
      updateViewportSize();
      requestAnimationFrame(updateViewportSize);
    });
  }

  function setScrollViewportRef(element: Element | ComponentPublicInstance | null) {
    const viewport = resolveElement(element);

    if (scrollViewportRef.value === viewport) {
      return;
    }

    scrollViewportRef.value = viewport;
    observeViewportSize();
  }

  function handleScroll(event: Event) {
    const target = event.currentTarget;

    if (target instanceof HTMLElement) {
      const viewport = resolveScrollViewportElement(target);

      if (scrollViewportRef.value !== viewport) {
        scrollViewportRef.value = viewport;
        observeViewportSize();
      }

      scrollTop.value = Math.max(0, viewport.scrollTop);
    }
  }

  function getEntryElement(path: string): HTMLElement | null {
    const viewport = scrollViewportRef.value;

    if (!viewport) {
      return null;
    }

    const normalizedPath = normalizePath(path);
    const candidates = path === normalizedPath ? [path] : [normalizedPath, path];

    for (const candidate of [...new Set(candidates)]) {
      const match = viewport.querySelector<HTMLElement>(
        `[data-entry-path="${escapeCssAttribute(candidate)}"]`,
      );

      if (match) {
        return match;
      }
    }

    return null;
  }

  async function ensureEntryElementInView(
    path: string,
    row: FileBrowserVirtualRow,
    align: ScrollLogicalPosition,
  ) {
    await nextTick();

    const viewport = scrollViewportRef.value;
    const entryElement = getEntryElement(path);

    if (!viewport || !entryElement) {
      return;
    }

    const viewportRect = viewport.getBoundingClientRect();
    const entryRect = entryElement.getBoundingClientRect();
    const minVisibleTop = viewportRect.top + getRowScrollMarginTop(row, virtualContentOffset.value, gridGap.value);
    const maxVisibleBottom = viewportRect.bottom - bottomScrollMargin.value;
    let scrollAdjustment = 0;

    if (align === 'center') {
      const viewportCenter = minVisibleTop + (maxVisibleBottom - minVisibleTop) / 2;
      const entryCenter = entryRect.top + entryRect.height / 2;
      scrollAdjustment = entryCenter - viewportCenter;
    }
    else if (align === 'start') {
      scrollAdjustment = entryRect.top - minVisibleTop;
    }
    else if (align === 'end') {
      scrollAdjustment = entryRect.bottom - maxVisibleBottom;
    }
    else if (entryRect.top < minVisibleTop) {
      scrollAdjustment = entryRect.top - minVisibleTop;
    }
    else if (entryRect.bottom > maxVisibleBottom) {
      scrollAdjustment = entryRect.bottom - maxVisibleBottom;
    }

    if (scrollAdjustment !== 0) {
      setScrollTop(scrollTop.value + scrollAdjustment);
      await nextTick();
    }
  }

  /**
   * 按路径找行。
   *
   * 改造前是把整个 rows 数组扫一遍（O(N)，而且 rows 本身已经是 O(N) 个行对象）。
   * 现在走「路径 → 显示顺序下标 → 行序列下标 → 物化这一行」，
   * 路径索引在模型上惰性构建一次（键直接复用条目上已有的 path 字符串）。
   */
  function findRowByPath(path: string): FileBrowserVirtualRow | null {
    const model = rowModel.value;
    const displayIndex = findVirtualRowDisplayIndex(model, path);

    if (displayIndex === undefined) {
      return null;
    }

    const rowIndex = resolveVirtualRowIndexByDisplayIndex(model, displayIndex);

    if (rowIndex === undefined) {
      return null;
    }

    return materializeVirtualRow(model, rowIndex) ?? null;
  }

  function getMaxScrollTop() {
    const viewport = scrollViewportRef.value;
    const virtualMaxScrollTop = Math.max(0, virtualContentOffset.value + totalSize.value - viewportHeight.value);
    const browserMaxScrollTop = viewport ? Math.max(0, viewport.scrollHeight - viewport.clientHeight) : 0;

    return Math.max(virtualMaxScrollTop, browserMaxScrollTop);
  }

  function setScrollTop(nextScrollTop: number) {
    const viewport = scrollViewportRef.value;

    if (!viewport) {
      return;
    }

    const maxScrollTop = getMaxScrollTop();
    const normalizedScrollTop = Math.min(Math.max(0, nextScrollTop), maxScrollTop);
    viewport.scrollTop = normalizedScrollTop;
    scrollTop.value = normalizedScrollTop;
  }

  async function scrollToRow(row: FileBrowserVirtualRow, align: ScrollLogicalPosition = 'nearest') {
    const currentViewportHeight = viewportHeight.value;
    const currentScrollTop = scrollTop.value;
    const rowStart = virtualContentOffset.value + row.start;
    const rowEnd = virtualContentOffset.value + getRowEnd(row);
    const scrollMarginTop = getRowScrollMarginTop(row, virtualContentOffset.value, gridGap.value);
    const scrollMarginBottom = bottomScrollMargin.value;

    if (align === 'center') {
      setScrollTop(rowStart - (currentViewportHeight - row.size) / 2);
    }
    else if (align === 'end') {
      setScrollTop(rowEnd - currentViewportHeight + scrollMarginBottom);
    }
    else if (align === 'start') {
      setScrollTop(rowStart - scrollMarginTop);
    }
    else if (rowStart < currentScrollTop + scrollMarginTop) {
      setScrollTop(rowStart - scrollMarginTop);
    }
    else if (rowEnd > currentScrollTop + currentViewportHeight - scrollMarginBottom) {
      setScrollTop(rowEnd - currentViewportHeight + scrollMarginBottom);
    }

    await nextTick();
  }

  async function scrollToPath(path: string, align: ScrollLogicalPosition = 'nearest'): Promise<boolean> {
    const row = findRowByPath(path);

    if (!row) {
      return false;
    }

    for (let attemptIndex = 0; attemptIndex < SCROLL_TO_PATH_DOM_SYNC_ATTEMPTS; attemptIndex += 1) {
      await scrollToRow(row, align);
      await nextTick();
      await new Promise<void>((resolveFrame) => {
        requestAnimationFrame(() => resolveFrame());
      });

      const entryElement = getEntryElement(path);

      if (entryElement) {
        await ensureEntryElementInView(path, row, align);
        return true;
      }
    }

    return false;
  }

  async function scrollToIndex(index: number, align: ScrollLogicalPosition = 'nearest'): Promise<boolean> {
    const entry = options.entries.value[index];

    if (!entry) {
      return false;
    }

    return scrollToPath(entry.path, align);
  }

  function getGridNavigationEntry(path: string, direction: 'up' | 'down'): DirEntry | null {
    return getFileBrowserGridNavigationEntry(rowModel.value, path, direction);
  }

  watch(rowModel, () => {
    const viewport = scrollViewportRef.value;

    if (!viewport) {
      return;
    }

    if (scrollTop.value > getMaxScrollTop()) {
      setScrollTop(getMaxScrollTop());
    }
  });

  watch(options.entries, () => {
    scheduleViewportSizeUpdate();
  });

  watch(() => options.layout(), () => {
    scheduleViewportSizeUpdate();
  });

  watch(() => options.increaseFileViewGaps?.(), () => {
    scheduleViewportSizeUpdate();
  });

  onBeforeUnmount(() => {
    disconnectViewportResizeObserver();
  });

  return {
    scrollViewportRef,
    viewportHeight,
    viewportWidth,
    scrollTop,
    virtualContentOffset,
    gridColumnCount,
    gridGap,
    rowModel,
    rows,
    visibleRows,
    activeGridSectionRow,
    totalSize,
    offsetY,
    spacerStyle,
    windowStyle,
    setScrollViewportRef,
    setScrollTop,
    scheduleViewportSizeUpdate,
    handleScroll,
    getEntryElement,
    scrollToPath,
    scrollToIndex,
    getGridNavigationEntry,
  };
}
