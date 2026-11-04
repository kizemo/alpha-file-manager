// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L2 ——
// 右键「打开文件所在目录」在「AFM 就是默认资源管理器」时改走应用内定位。
//
// 判别点：
//   - 是默认资源管理器 → 调 `openNavigatorRevealEntry`，**不**碰系统 Explorer。
//   - 不是默认资源管理器 → 仍然调 `revealInExplorer`（原行为不能被改坏）。
//

import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import { computed, ref } from 'vue';
import type { DirEntry } from '@/types/dir-entry';

const {
  isDefaultFileManagerSafeMock,
  openNavigatorRevealEntryMock,
  revealInExplorerMock,
} = vi.hoisted(() => ({
  isDefaultFileManagerSafeMock: vi.fn<() => Promise<boolean>>(),
  openNavigatorRevealEntryMock: vi.fn(),
  revealInExplorerMock: vi.fn<() => Promise<void>>(),
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({ t: (key: string) => key }),
}));

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({
    currentPlatform: 'windows',
    isWindows: true,
  }),
}));

vi.mock('@/stores/runtime/quick-view', () => ({
  quickViewSupportedPathsFromVisibleEntries: () => [],
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: vi.fn(),
    warning: vi.fn(),
    success: vi.fn(),
    custom: vi.fn(),
  },
  ToastStatic: class ToastStatic {},
}));

vi.mock('@/utils/open-native-properties', () => ({
  openNativeProperties: vi.fn(),
}));

vi.mock('@/utils/open-in-explorer', () => ({
  revealInExplorer: revealInExplorerMock,
}));

vi.mock('@/utils/open-navigator-directory', () => ({
  openNavigatorRevealEntry: openNavigatorRevealEntryMock,
}));

vi.mock('@/utils/default-file-manager', () => ({
  isDefaultFileManagerSafe: isDefaultFileManagerSafeMock,
}));

vi.mock('@/modules/remote/utils/remote-permissions-dialog-state', () => ({
  openRemotePermissionsDialog: vi.fn(),
}));

vi.mock('@/modules/remote/utils/remote-transfer-service', () => ({
  uploadItemsToRemote: vi.fn(),
  downloadItemsFromRemote: vi.fn(),
}));

vi.mock('@/modules/remote/utils/remote-transfer-targets', () => ({
  useRemoteTransferTargets: () => ({
    remotePanePath: { value: null },
    localPanePath: { value: null },
  }),
}));

const { useFileBrowserActions } = await import('../use-file-browser-actions');

const LOCAL_DIR = 'C:/Users/Duanyi/afm-acceptance';

function makeEntry(path: string, name: string, isDir: boolean): DirEntry {
  return {
    name,
    ext: null,
    path,
    size: 12,
    item_count: null,
    modified_time: 0,
    accessed_time: 0,
    created_time: 0,
    mime: null,
    is_file: !isDir,
    is_dir: isDir,
    is_symlink: false,
    is_hidden: false,
  };
}

function createActions(selectedEntries: DirEntry[]) {
  const contextMenu = ref({
    targetEntry: selectedEntries[0] ?? null,
    selectedEntries,
  });
  const selected = ref<DirEntry[]>(selectedEntries);

  return useFileBrowserActions({
    contextMenu,
    selectedEntries: selected,
    visibleEntries: computed(() => selectedEntries),
    quickViewStore: {
      toggleQuickView: vi.fn(),
      openPrintViewFromMainWindow: vi.fn(),
    },
    handleContextMenuAction: vi.fn(),
    openOpenWithDialog: vi.fn(),
    handleEntryMouseDown: vi.fn(),
    handleEntryMouseUp: vi.fn(),
  });
}

describe('右键「打开文件所在目录」的改道（L2）', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    isDefaultFileManagerSafeMock.mockResolvedValue(true);
    revealInExplorerMock.mockResolvedValue(undefined);
  });

  it('AFM 是默认资源管理器 → 走应用内定位，不碰系统 Explorer', async () => {
    const actions = createActions([
      makeEntry(`${LOCAL_DIR}/b.txt`, 'b.txt', false),
    ]);

    actions.onContextMenuAction('open-containing-directory');

    await vi.waitFor(() => {
      expect(openNavigatorRevealEntryMock).toHaveBeenCalledTimes(1);
    });

    const [, entryPath, options] = openNavigatorRevealEntryMock.mock.calls[0];

    expect(entryPath).toBe(`${LOCAL_DIR}/b.txt`);
    // 应用内定位必须走「当前这一格」，不能拆栏、不能切 pane。
    expect(options).toMatchObject({ reuseCurrentPaneOnly: true });
    expect(revealInExplorerMock).not.toHaveBeenCalled();
  });

  it('不是默认资源管理器 → 仍然走系统 Explorer（原行为不能被改坏）', async () => {
    isDefaultFileManagerSafeMock.mockResolvedValue(false);

    const actions = createActions([
      makeEntry(`${LOCAL_DIR}/b.txt`, 'b.txt', false),
    ]);

    actions.onContextMenuAction('open-containing-directory');

    await vi.waitFor(() => {
      expect(revealInExplorerMock).toHaveBeenCalledWith(`${LOCAL_DIR}/b.txt`);
    });

    expect(openNavigatorRevealEntryMock).not.toHaveBeenCalled();
  });

  it('目录条目不触发定位动作（visibility 规则之外的兜底）', async () => {
    const actions = createActions([
      makeEntry(`${LOCAL_DIR}/dir`, 'dir', true),
    ]);

    actions.onContextMenuAction('open-containing-directory');

    await new Promise(resolve => setTimeout(resolve, 0));

    expect(revealInExplorerMock).not.toHaveBeenCalled();
    expect(openNavigatorRevealEntryMock).not.toHaveBeenCalled();
  });
});
