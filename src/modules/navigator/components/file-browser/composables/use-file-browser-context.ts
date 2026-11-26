// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  provide, inject, type InjectionKey, type Ref, type ComputedRef, type ComponentPublicInstance,
} from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import type { ContextMenuAction, ContextMenuState } from '@/modules/navigator/components/file-browser/types';
import type { LinkCreationKind } from '@/utils/link-operations';
import type {
  FileBrowserGridSectionVirtualRow,
  FileBrowserVirtualRow,
} from './use-file-browser-virtual-layout';

/**
 * FORK-MODIFICATION 2026-10-27：就地改名的上下文。
 *
 * 视图层（列表视图 / 缩略图卡片）只认这个接口，不直接碰 selection ——
 * 这样「编辑态是哪一个条目」这件事只有一处说了算。
 */
export interface FileBrowserInlineRenameContext {
  /** 当前是否有条目处于就地编辑态。 */
  isActive: ComputedRef<boolean>;
  /** 处于就地编辑态的条目路径；没有则 `null`。 */
  editingEntryPath: ComputedRef<string | null>;
  /** 连续改名模式是否开着（提交过一次之后 ↑/↓ 的落点会自动进入编辑态）。 */
  isChainActive: ComputedRef<boolean>;
  /** 提交当前编辑。 */
  submit: (name: string, trigger: 'enter' | 'blur') => Promise<boolean>;
  /** 放弃当前编辑（Esc）。 */
  cancel: () => void;
  /** 编辑态内按 ↑/↓：先提交当前改名，提交成功再移动到相邻条目。 */
  move: (direction: 'up' | 'down', currentName: string) => Promise<void>;
}

export interface FileBrowserContext {
  /** FORK-MODIFICATION 2026-10-27：就地重命名。 */
  inlineRename: FileBrowserInlineRenameContext;
  entries: ComputedRef<DirEntry[]>;
  directoryEntryCount: ComputedRef<number>;
  currentPath: ComputedRef<string>;
  isLoading: Ref<boolean>;
  isLinkMetadataLoading: ComputedRef<boolean>;
  isDirectoryEmpty: ComputedRef<boolean>;
  error: Ref<string | null>;

  selectedEntries: Ref<DirEntry[]>;
  isEntrySelected: (entry: DirEntry) => boolean;
  contextMenu: Ref<ContextMenuState>;
  /** FORK-MODIFICATION: 当前是否在渲染搜索结果（全局搜索 externalEntries 或目录内过滤）。 */
  isSearchResultsView: ComputedRef<boolean>;

  getImageThumbnail: (entry: DirEntry, maxDimension?: number) => string | undefined;
  getImageThumbnailPlaceholder: (entry: DirEntry, maxDimension?: number) => string | undefined;
  shouldShowImageThumbnailFallback: (entry: DirEntry, maxDimension?: number) => boolean;
  cancelImageThumbnail: (entry: DirEntry, maxDimension?: number) => void;
  getVideoThumbnail: (entry: DirEntry) => string | undefined;
  cancelVideoThumbnail: (entry: DirEntry) => void;
  setEntriesContainerRef: (element: Element | ComponentPublicInstance | null) => void;
  setScrollViewportRef: (element: Element | ComponentPublicInstance | null) => void;
  handleVirtualScroll: (event: Event) => void;
  virtualRows: ComputedRef<FileBrowserVirtualRow[]>;
  visibleVirtualRows: ComputedRef<FileBrowserVirtualRow[]>;
  activeGridSectionRow: ComputedRef<FileBrowserGridSectionVirtualRow | null>;
  virtualTotalSize: ComputedRef<number>;
  virtualOffsetY: ComputedRef<number>;
  virtualSpacerStyle: ComputedRef<Record<string, string>>;
  virtualWindowStyle: ComputedRef<Record<string, string>>;
  virtualGridColumnCount: ComputedRef<number>;

  onEntryMouseDown: (entry: DirEntry, event: MouseEvent) => void;
  onEntryMouseUp: (entry: DirEntry, event: MouseEvent) => void;
  handleEntriesContainerPointerDown: (event: PointerEvent) => void;
  increaseFileViewGaps: ComputedRef<boolean>;
  handleEntryFocus: (entry: DirEntry, event: FocusEvent) => void;
  handleEntryContextMenu: (entry: DirEntry) => void;
  handleBackgroundContextMenu: () => void;
  closeContextMenu: () => void;
  onContextMenuAction: (action: ContextMenuAction) => void;
  openProperties: (entries: DirEntry[]) => Promise<void>;
  createLinksForEntries: (entries: DirEntry[], linkKind: LinkCreationKind, destinationPath?: string) => Promise<boolean>;
  openOpenWithDialog: (entries: DirEntry[]) => void;
  openNewItemDialog: (type: 'file' | 'directory', targetPaths?: string[]) => void;
  navigateToHome: () => void | Promise<void>;
  navigateToPath: (path: string) => Promise<void>;

  refresh: () => void;

  requestFocusEntryAfterRefresh: (parentDirectoryPath: string, entryPath: string) => void;

  entryDescription?: (entry: DirEntry) => string | undefined;
  requestArchiveOptions: (
    needsPassword?: boolean,
    needsEncoding?: boolean,
    detectedEncoding?: string,
  ) => Promise<{
    password: string;
    encoding: string | undefined;
  } | null>;
}

export const FILE_BROWSER_CONTEXT_KEY: InjectionKey<FileBrowserContext> = Symbol('FileBrowserContext');

export function provideFileBrowserContext(context: FileBrowserContext): void {
  provide(FILE_BROWSER_CONTEXT_KEY, context);
}

export function useFileBrowserContext(): FileBrowserContext {
  const context = inject(FILE_BROWSER_CONTEXT_KEY);

  if (!context) {
    throw new Error('useFileBrowserContext must be used within a FileBrowser component that provides the context');
  }

  return context;
}
