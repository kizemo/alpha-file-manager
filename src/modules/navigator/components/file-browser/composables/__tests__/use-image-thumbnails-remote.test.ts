// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-21（批次 32）：远程网盘图片缩略图。
//
// 之前远程条目也走本地缩略图管线（`generate_image_thumbnail`），
// 而 `sfm://remote/...` 不是文件系统路径 ⇒ 后端必然失败 ⇒ 界面上只有图标。
//
// 现在：服务端（阿里云盘）随列表一起回的缩略图直链直接透传给 `<img src>`，
// 不进本地管线、不做占位图（远程条目的原图字节读不到）。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

const mockInvoke = vi.hoisted(() => vi.fn());

vi.mock('@tauri-apps/api/core', () => ({
  convertFileSrc: (path: string) => `asset://${path}`,
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

import { useImageThumbnails } from '../use-image-thumbnails';

const REMOTE_PATH = 'sfm://remote/conn-a/photos/IMG_0001.jpg';
const REMOTE_THUMBNAIL = 'https://example.invalid/thumb.jpg?sign=abc';

function createRemoteEntry(overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    name: 'IMG_0001.jpg',
    ext: 'jpg',
    path: REMOTE_PATH,
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
    thumbnail_url: REMOTE_THUMBNAIL,
    ...overrides,
  };
}

describe('远程网盘图片缩略图', () => {
  beforeEach(() => {
    mockInvoke.mockReset();
  });

  it('直接返回服务端给的缩略图直链，不走后端缩略图管线', () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();

    expect(thumbnails.getImageThumbnail(entry)).toBe(REMOTE_THUMBNAIL);
    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('远程条目不做占位图（原图字节读不到）', async () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();

    expect(thumbnails.getImageThumbnailPlaceholder(entry)).toBeUndefined();

    await new Promise<void>((resolve) => {
      setTimeout(resolve, 0);
    });

    expect(mockInvoke).not.toHaveBeenCalled();
  });

  it('远程条目有直链时不显示回退图标', () => {
    const thumbnails = useImageThumbnails();
    const entry = createRemoteEntry();

    expect(thumbnails.shouldShowImageThumbnailFallback(entry)).toBe(false);
  });

  it('同名远程条目各用自己的直链，不会串图', () => {
    const thumbnails = useImageThumbnails();
    const first = createRemoteEntry({
      entry_id: 'aliyun:file-a',
      thumbnail_url: 'https://example.invalid/a.jpg',
    });
    const second = createRemoteEntry({
      entry_id: 'aliyun:file-b',
      thumbnail_url: 'https://example.invalid/b.jpg',
    });

    expect(thumbnails.getImageThumbnail(first)).toBe('https://example.invalid/a.jpg');
    expect(thumbnails.getImageThumbnail(second)).toBe('https://example.invalid/b.jpg');
  });
});
