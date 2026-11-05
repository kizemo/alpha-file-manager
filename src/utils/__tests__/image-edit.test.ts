// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
//
// FORK-MODIFICATION: 图片编辑封装（`src/utils/image-edit.ts`）单测 ——
// 断言 invoke 的命令名与参数形状、失败走 toast 且不冒泡。

import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

const {
  invokeMock,
  toastSuccessMock,
  toastErrorMock,
  toastLoadingMock,
  toastDismissMock,
  handleDirectoryContentsChangedMock,
  cancelImageThumbnailMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  toastSuccessMock: vi.fn(),
  toastErrorMock: vi.fn(),
  toastLoadingMock: vi.fn(() => 'progress-1'),
  toastDismissMock: vi.fn(),
  handleDirectoryContentsChangedMock: vi.fn(),
  cancelImageThumbnailMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: {
    success: toastSuccessMock,
    error: toastErrorMock,
    loading: toastLoadingMock,
    dismiss: toastDismissMock,
    custom: vi.fn(),
    warning: vi.fn(),
  },
}));

vi.mock('@/stores/storage/workspaces', () => ({
  useWorkspacesStore: () => ({
    handleDirectoryContentsChanged: handleDirectoryContentsChangedMock,
  }),
}));

const {
  APPLY_IMAGE_EDIT_COMMAND,
  DEFAULT_IMAGE_EDIT_QUALITY,
  IMAGE_EDIT_ACTIONS,
  applyImageEdit,
  getImageEditOperation,
  invalidateImageCaches,
  isImageEditAction,
} = await import('@/utils/image-edit');

function createEntry(path: string, ext: string | null): DirEntry {
  return {
    name: path.split(/[\\/]/).pop() ?? path,
    path,
    ext,
    size: 2048,
    item_count: null,
    modified_time: 1700000000000,
    accessed_time: 1700000000000,
    created_time: 1700000000000,
    mime: null,
    is_file: true,
    is_dir: false,
    is_symlink: false,
    is_hidden: false,
  };
}

const PHOTO = createEntry('C:/Users/test/photo.jpg', 'jpg');

describe('图片编辑封装', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    invokeMock.mockResolvedValue({
      path: PHOTO.path,
      width: 100,
      height: 80,
      bytes: 4096,
      format: 'jpg',
    });
  });

  it('动作 id 与后端 operation 的对应关系是固定的四个', () => {
    expect(IMAGE_EDIT_ACTIONS).toEqual({
      'rotate-image-left': 'rotate-left',
      'rotate-image-right': 'rotate-right',
      'flip-image-horizontal': 'flip-horizontal',
      'flip-image-vertical': 'flip-vertical',
    });

    expect(getImageEditOperation('rotate-image-left')).toBe('rotate-left');
    expect(getImageEditOperation('rotate-image-right')).toBe('rotate-right');
    expect(getImageEditOperation('flip-image-horizontal')).toBe('flip-horizontal');
    expect(getImageEditOperation('flip-image-vertical')).toBe('flip-vertical');

    expect(isImageEditAction('rotate-image-left')).toBe(true);
    expect(isImageEditAction('rename')).toBe(false);
  });

  it('调的是 apply_image_edit，参数是 { path, operation, quality }', async () => {
    const succeeded = await applyImageEdit({
      entry: PHOTO,
      operation: 'rotate-left',
    });

    expect(succeeded).toBe(true);
    expect(invokeMock).toHaveBeenCalledTimes(1);
    expect(invokeMock).toHaveBeenCalledWith(APPLY_IMAGE_EDIT_COMMAND, {
      path: 'C:/Users/test/photo.jpg',
      operation: 'rotate-left',
      quality: DEFAULT_IMAGE_EDIT_QUALITY,
    });
    expect(APPLY_IMAGE_EDIT_COMMAND).toBe('apply_image_edit');
  });

  it('四种 operation 各自原样传给后端', async () => {
    const operations = ['rotate-left', 'rotate-right', 'flip-horizontal', 'flip-vertical'] as const;

    for (const operation of operations) {
      invokeMock.mockClear();

      await applyImageEdit({
        entry: PHOTO,
        operation,
      });

      expect(invokeMock).toHaveBeenCalledWith('apply_image_edit', expect.objectContaining({
        operation,
      }));
    }
  });

  it('成功后弹成功 toast，并让父目录重读一遍', async () => {
    await applyImageEdit({
      entry: PHOTO,
      operation: 'flip-horizontal',
    });

    expect(toastSuccessMock).toHaveBeenCalledTimes(1);
    expect(toastErrorMock).not.toHaveBeenCalled();

    expect(handleDirectoryContentsChangedMock).toHaveBeenCalledTimes(1);
    const [paths] = handleDirectoryContentsChangedMock.mock.calls[0];
    expect(paths).toHaveLength(1);
    expect(String(paths[0])).toContain('Users/test');
  });

  it('成功后会让缩略图缓存失效（用既有缓存工具）', async () => {
    await applyImageEdit({
      entry: PHOTO,
      operation: 'rotate-right',
      thumbnailCache: {
        cancelImageThumbnail: cancelImageThumbnailMock,
        clearThumbnails: vi.fn(),
      },
    });

    expect(cancelImageThumbnailMock).toHaveBeenCalledWith(PHOTO);
  });

  it('失败走错误 toast，且不向上冒泡', async () => {
    invokeMock.mockRejectedValue(new Error('backend exploded'));

    const succeeded = await applyImageEdit({
      entry: PHOTO,
      operation: 'rotate-left',
    });

    expect(succeeded).toBe(false);
    expect(toastErrorMock).toHaveBeenCalledTimes(1);
    expect(toastSuccessMock).not.toHaveBeenCalled();
    // 失败时不刷新目录（文件没变）。
    expect(handleDirectoryContentsChangedMock).not.toHaveBeenCalled();
  });

  it('非图片条目直接拒绝，根本不调后端', async () => {
    const textFile = createEntry('C:/Users/test/notes.txt', 'txt');

    const succeeded = await applyImageEdit({
      entry: textFile,
      operation: 'rotate-left',
    });

    expect(succeeded).toBe(false);
    expect(invokeMock).not.toHaveBeenCalled();
    expect(toastErrorMock).toHaveBeenCalledTimes(1);
  });

  it('处理期间弹「处理中」toast，结束后关掉它', async () => {
    await applyImageEdit({
      entry: PHOTO,
      operation: 'rotate-left',
    });

    expect(toastLoadingMock).toHaveBeenCalledTimes(1);
    expect(toastDismissMock).toHaveBeenCalledWith('progress-1');
  });

  it('没有缓存实例时 invalidateImageCaches 不炸', () => {
    expect(() => invalidateImageCaches(PHOTO, null)).not.toThrow();
  });
});
