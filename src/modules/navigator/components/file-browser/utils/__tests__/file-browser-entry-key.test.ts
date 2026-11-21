// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-21（批次 32）：条目唯一键。
//
// 远程网盘的条目 `path` 由「父目录路径 + 条目名」拼成，而同一个目录里
// **允许存在同名条目** —— 阿里云盘真机实测（`.superpowers/sdd/b32/b32-probe-walk.txt`）：
// 「手机备份/照片备份」下有约 90 组同名图片。
// 两个不同条目的 `path` 因此逐字相同，选中判定按 path 比较时
// 点一个会同时高亮两个（用户 2026-11-21 报的 bug）。

import { describe, expect, it } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';
import { getFileBrowserEntryKey } from '../file-browser-entry-key';

function createEntry(overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    name: 'IMG_0001.jpg',
    ext: 'jpg',
    path: 'sfm://remote/conn-a/photos/IMG_0001.jpg',
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

describe('getFileBrowserEntryKey', () => {
  it('gives different keys to same-named entries that live in the same folder', () => {
    const first = createEntry({ entry_id: 'aliyun:file-a' });
    const second = createEntry({ entry_id: 'aliyun:file-b' });

    // 前提：两个条目的 path 逐字相同。
    expect(first.path).toBe(second.path);
    expect(getFileBrowserEntryKey(first)).not.toBe(getFileBrowserEntryKey(second));
  });

  it('gives the same key to the same entry', () => {
    const first = createEntry({ entry_id: 'aliyun:file-a' });
    const second = createEntry({ entry_id: 'aliyun:file-a' });

    expect(getFileBrowserEntryKey(first)).toBe(getFileBrowserEntryKey(second));
  });

  it('falls back to the path for local entries', () => {
    const entry = createEntry({
      path: 'C:/media/photo.jpg',
      entry_id: null,
    });

    expect(getFileBrowserEntryKey(entry)).toBe('path:C:/media/photo.jpg');
  });

  it('keeps the provider namespace so ids from different providers never collide', () => {
    const aliyun = createEntry({ entry_id: 'aliyun:file-a' });
    const other = createEntry({ entry_id: 'quark:file-a' });

    expect(getFileBrowserEntryKey(aliyun)).not.toBe(getFileBrowserEntryKey(other));
  });
});
