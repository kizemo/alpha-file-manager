// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
//
// FORK-MODIFICATION: 文件浏览器右键「图片编辑四动作」的执行链路单测 ——
// 断言动作 id 被正确翻译成后端 operation 并真的调了 `apply_image_edit`。

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
  invokeMock,
  toastSuccessMock,
  toastErrorMock,
  handleDirectoryContentsChangedMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
  handleDirectoryContentsChangedMock: vi.fn(),
}));

// vue-i18n 的 createI18n 必须保留：@/localization 在模块加载期就会调用它，
// 少了它会让图片编辑链路在 import 阶段就炸掉（invoke 根本不会被调用）。
vi.mock('vue-i18n', async (importOriginal) => ({
  ...(await importOriginal<typeof import('vue-i18n')>()),
  useI18n: () => ({ t: (key: string) => key }),
}));

// 图片编辑成功/失败文案走 @/localization 的 i18n.global.t，这里给出最小实现。
vi.mock('@/localization', () => ({
  i18n: { global: { t: (key: string) => key } },
}));

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
  convertFileSrc: (path: string) => path,
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

vi.mock('@/stores/storage/workspaces', () => ({
  useWorkspacesStore: () => ({
    handleDirectoryContentsChanged: handleDirectoryContentsChangedMock,
  }),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    error: toastErrorMock,
    warning: vi.fn(),
    success: toastSuccessMock,
    loading: vi.fn(() => 'progress-1'),
    dismiss: vi.fn(),
    custom: vi.fn(),
  },
  ToastStatic: class ToastStatic {},
}));

vi.mock('@/utils/open-native-properties', () => ({
  openNativeProperties: vi.fn(),
}));

vi.mock('@/utils/open-in-explorer', () => ({
  revealInExplorer: vi.fn(),
}));

vi.mock('@/utils/open-navigator-directory', () => ({
  openNavigatorRevealEntry: vi.fn(),
}));

vi.mock('@/utils/default-file-manager', () => ({
  isDefaultFileManagerSafe: vi.fn(),
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

function makeEntry(path: string, ext: string | null, isDir = false): DirEntry {
  return {
    name: path.split(/[\\/]/).pop() ?? path,
    ext,
    path,
    size: 2048,
    item_count: null,
    modified_time: 1700000000000,
    accessed_time: 1700000000000,
    created_time: 1700000000000,
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

const PHOTO_PATH = 'C:/Users/Duanyi/afm-acceptance/photo.jpg';

describe('文件浏览器的图片编辑动作', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockResolvedValue({
      path: PHOTO_PATH,
      width: 100,
      height: 80,
      bytes: 4096,
      format: 'jpg',
    });
  });

  it('四个动作分别调出四种 operation', async () => {
    const expectations = [
      ['rotate-image-left', 'rotate-left'],
      ['rotate-image-right', 'rotate-right'],
      ['flip-image-horizontal', 'flip-horizontal'],
      ['flip-image-vertical', 'flip-vertical'],
    ] as const;

    for (const [action, operation] of expectations) {
      invokeMock.mockClear();

      const actions = createActions([makeEntry(PHOTO_PATH, 'jpg')]);
      actions.onContextMenuAction(action);

      await vi.waitFor(() => {
        expect(invokeMock).toHaveBeenCalledTimes(1);
      });

      expect(invokeMock).toHaveBeenCalledWith('apply_image_edit', {
        path: PHOTO_PATH,
        operation,
        quality: 90,
      });
    }
  });

  it('成功后弹成功 toast 并让父目录重读', async () => {
    const actions = createActions([makeEntry(PHOTO_PATH, 'jpg')]);
    actions.onContextMenuAction('rotate-image-right');

    await vi.waitFor(() => {
      expect(toastSuccessMock).toHaveBeenCalledTimes(1);
    });

    expect(toastErrorMock).not.toHaveBeenCalled();
    expect(handleDirectoryContentsChangedMock).toHaveBeenCalledTimes(1);
  });

  it('目录条目不触发后端调用', async () => {
    const actions = createActions([makeEntry('C:/Users/Duanyi/afm-acceptance/folder', null, true)]);
    actions.onContextMenuAction('rotate-image-left');

    await new Promise(resolve => setTimeout(resolve, 0));

    expect(invokeMock).not.toHaveBeenCalled();
  });

  it('后端报错走错误 toast，且不向上冒泡', async () => {
    invokeMock.mockRejectedValue(new Error('boom'));

    const actions = createActions([makeEntry(PHOTO_PATH, 'jpg')]);
    actions.onContextMenuAction('rotate-image-left');

    await vi.waitFor(() => {
      expect(toastErrorMock).toHaveBeenCalledTimes(1);
    });

    expect(toastSuccessMock).not.toHaveBeenCalled();
  });
});
