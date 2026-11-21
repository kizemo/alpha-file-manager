// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-21（批次 32）：远程网盘「左键单击选中两个文件」的回归。
//
// 现象：云盘里同一个目录存在同名文件时，左键点其中一个，**两个都高亮**。
// 根因：远程条目的 `path` 由「父目录路径 + 条目名」拼成，同目录同名条目的 path
// 逐字相同；选中判定原本按 path 比较，于是「已选中」这一个判断同时命中两行。
//
// 修法：引入「条目唯一键」（`entry_id`），选中判定 / 移除 / 连选 / 键盘落点全部改用它。
//
// 这个文件用的是**真的** `useFileBrowserSelection`（mock 夹具与
// `use-file-browser-selection-focus.test.ts` 同源），不是自己实现的假身。

import { ref } from 'vue';
import { describe, expect, it, vi } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

vi.mock('vue-i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-i18n')>();

  return {
    ...actual,
    useI18n: () => ({ t: (key: string) => key }),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(),
}));

vi.mock('@/stores/storage/workspaces', () => ({
  useWorkspacesStore: () => ({
    openNewTabGroup: vi.fn(),
    handleDirectoryContentsChanged: vi.fn(),
    handlePathRenamed: vi.fn(),
    handlePathsDeleted: vi.fn(),
  }),
}));

vi.mock('@/stores/storage/user-stats', () => ({
  useUserStatsStore: () => ({
    isFavorite: vi.fn(() => false),
    removeFromFavorites: vi.fn(),
    handlePathRenamed: vi.fn(),
    handlePathsDeleted: vi.fn(),
  }),
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => ({
    handlePathRenamed: vi.fn(),
    handlePathsDeleted: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/clipboard', () => ({
  useClipboardStore: () => ({
    setClipboard: vi.fn(),
    readSystemClipboardFiles: vi.fn(),
    pasteSystemClipboardImage: vi.fn(),
    discardClipboard: vi.fn(),
    hasFileItems: false,
    clipboardItems: [],
    isCopyOperation: true,
  }),
}));

vi.mock('@/stores/runtime/dir-sizes', () => ({
  useDirSizesStore: () => ({
    invalidate: vi.fn(),
    refreshSizesAfterCopyMove: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/link-metadata', () => ({
  useLinkMetadataStore: () => ({
    invalidate: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/delete-jobs', () => ({
  useDeleteJobsStore: () => ({
    startJob: vi.fn(),
  }),
}));

vi.mock('@/stores/runtime/folder-tree', () => ({
  useFolderTreeStore: () => ({
    markTreeStale: vi.fn(),
    handlePathRenamed: vi.fn(),
  }),
  parentDirectoryPath: (path: string) => path,
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({
    currentPlatform: 'windows',
    isWindows: true,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: Object.assign(vi.fn(), {
    success: vi.fn(),
    error: vi.fn(),
    info: vi.fn(),
    warning: vi.fn(),
  }),
  ToastProgress: class {},
  ToastStatic: class {},
}));

vi.mock('@/composables/use-lan-share', () => ({
  useLanShare: () => ({ startShare: vi.fn() }),
}));

vi.mock('@/composables/use-copy-move-with-conflicts', () => ({
  useCopyMoveWithConflicts: () => ({
    performCopyMoveWithConflicts: vi.fn(),
    conflictDialogState: ref(null),
    handleConflictResolution: vi.fn(),
    handleConflictCancel: vi.fn(),
    topLevelNameConflictDialogState: ref(null),
    handleTopLevelNameConflictRename: vi.fn(),
    handleTopLevelNameConflictMerge: vi.fn(),
    handleTopLevelNameConflictCancel: vi.fn(),
  }),
}));

vi.mock('@/composables/use-permanent-delete-confirm', () => ({
  usePermanentDeleteConfirm: () => ({
    requestConfirm: vi.fn(),
  }),
}));

vi.mock('@/modules/remote/utils/remote-delete-confirm', () => ({
  useRemoteDeleteConfirm: () => ({
    requestConfirm: vi.fn(),
  }),
}));

vi.mock('@/modules/remote/utils/remote-delete', () => ({
  deleteRemoteEntries: vi.fn(),
}));

vi.mock('@/modules/remote/utils/remote-write', () => ({
  createRemoteDirectory: vi.fn(),
  hasRemoteTarget: vi.fn(() => false),
  renameRemoteEntry: vi.fn(),
}));

vi.mock('@/utils/disconnect-drive', () => ({
  disconnectDriveForEntry: vi.fn(),
}));

vi.mock('@/modules/home/composables/use-drives', () => ({
  refreshDrives: vi.fn(),
}));

vi.mock('@/utils/document-selection', () => ({
  clearDocumentTextSelection: vi.fn(),
}));

import { useFileBrowserSelection } from '../use-file-browser-selection';

const DUPLICATE_PATH = 'sfm://remote/conn-a/photos/IMG_0001.jpg';

function createEntry(overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    name: 'IMG_0001.jpg',
    ext: 'jpg',
    path: DUPLICATE_PATH,
    size: 1024,
    item_count: null,
    modified_time: 0,
    accessed_time: 0,
    created_time: 0,
    mime: 'image/jpeg',
    is_file: true,
    is_dir: false,
    is_symlink: false,
    is_hidden: false,
    ...overrides,
  };
}

function createHarness(entries: DirEntry[]) {
  const entriesRef = ref<DirEntry[]>(entries);
  const currentPathRef = ref('sfm://remote/conn-a/photos');
  const onSelect = vi.fn();

  const selection = useFileBrowserSelection(
    entriesRef,
    currentPathRef,
    onSelect,
    vi.fn(),
    vi.fn(),
    vi.fn(),
    vi.fn(),
    () => 'list',
  );

  return {
    selection,
    onSelect,
    entriesRef,
  };
}

function createMouseEvent(type: 'mousedown' | 'mouseup', ctrlKey = false): MouseEvent {
  return new MouseEvent(type, {
    button: 0,
    bubbles: true,
    ctrlKey,
  });
}

// 同目录的两个同名条目：path 逐字相同，只有 entry_id 不同。
function createDuplicatePair() {
  return {
    first: createEntry({ entry_id: 'aliyun:file-a' }),
    second: createEntry({ entry_id: 'aliyun:file-b' }),
  };
}

describe('远程网盘同名条目的选中判定', () => {
  it('左键单击一个同名条目时，不会把另一个同名条目也算成已选中', () => {
    const { first, second } = createDuplicatePair();
    const { selection } = createHarness([first, second]);

    selection.handleEntryMouseDown(second, createMouseEvent('mousedown'));
    selection.handleEntryMouseUp(second, createMouseEvent('mouseup'));

    expect(selection.selectedEntries.value).toEqual([second]);
    expect(selection.isEntrySelected(second)).toBe(true);
    expect(selection.isEntrySelected(first)).toBe(false);
  });

  it('选中一个同名条目后，第二个同名条目仍可被单独选中', () => {
    const { first, second } = createDuplicatePair();
    const { selection } = createHarness([first, second]);

    selection.replaceSelection(first);
    expect(selection.isEntrySelected(first)).toBe(true);
    expect(selection.isEntrySelected(second)).toBe(false);

    selection.handleEntryMouseDown(second, createMouseEvent('mousedown'));
    selection.handleEntryMouseUp(second, createMouseEvent('mouseup'));

    expect(selection.selectedEntries.value).toEqual([second]);
    expect(selection.isEntrySelected(first)).toBe(false);
  });

  it('Ctrl+单击取消选中一个同名条目时，不会把另一个一起取消', () => {
    const { first, second } = createDuplicatePair();
    const { selection } = createHarness([first, second]);

    selection.replaceSelection(first);

    // Ctrl+单击第二个同名条目 → 追加进选区（走真实的加选路径，不直接调内部函数）
    selection.handleEntryMouseDown(second, createMouseEvent('mousedown', true));
    selection.handleEntryMouseUp(second, createMouseEvent('mouseup', true));

    expect(selection.selectedEntries.value).toHaveLength(2);

    // 再 Ctrl+单击一次 → 只取消这一个，另一个仍留在选区里
    selection.handleEntryMouseDown(second, createMouseEvent('mousedown', true));
    selection.handleEntryMouseUp(second, createMouseEvent('mouseup', true));

    expect(selection.selectedEntries.value).toEqual([first]);
    expect(selection.isEntrySelected(first)).toBe(true);
    expect(selection.isEntrySelected(second)).toBe(false);
  });

  it('按唯一键能选中指定的那一个，而不是第一个同名条目', () => {
    const { first, second } = createDuplicatePair();
    const { selection } = createHarness([first, second]);

    expect(selection.selectEntryByKey('id:aliyun:file-b')).toBe(true);

    expect(selection.selectedEntries.value).toEqual([second]);
    expect(selection.isEntrySelected(first)).toBe(false);
  });

  it('连选区间时，同名条目按唯一键定位起点与终点', () => {
    const { first, second } = createDuplicatePair();
    const other = createEntry({
      name: 'IMG_0002.jpg',
      path: 'sfm://remote/conn-a/photos/IMG_0002.jpg',
      entry_id: 'aliyun:file-c',
    });
    const { selection } = createHarness([first, second, other]);

    selection.selectRange(second, other);

    const selectedIds = selection.selectedEntries.value.map(
      entry => entry.entry_id,
    );

    expect(selectedIds).toContain('aliyun:file-b');
    expect(selectedIds).toContain('aliyun:file-c');
    expect(selectedIds).not.toContain('aliyun:file-a');
  });
});
