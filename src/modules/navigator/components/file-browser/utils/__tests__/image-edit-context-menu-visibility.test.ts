// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
//
// FORK-MODIFICATION: 图片编辑四动作（右键「向左旋转 / 向右旋转 / 左右翻转 / 上下翻转」）
// 的可见性判定单测。

import { describe, expect, it } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';
import {
  CONTEXT_MENU_ACTION_IDS,
  EDITABLE_IMAGE_EXTENSIONS,
  IMAGE_ONLY_ACTIONS,
  isContextMenuActionVisible,
  isEditableImageEntry,
  isImageOnlyActionVisible,
} from '../context-menu-action-visibility';
import { LOCATIONS_VIRTUAL_PATH } from '@/utils/virtual-path-constants';

const IMAGE_ACTIONS = [
  'rotate-image-left',
  'rotate-image-right',
  'flip-image-horizontal',
  'flip-image-vertical',
] as const;

type EntryOptions = {
  ext?: string | null;
  isDir?: boolean;
};

function createEntry(path: string, options: EntryOptions = {}): DirEntry {
  const isDir = options.isDir === true;
  const ext = options.ext === undefined ? null : options.ext;

  return {
    name: path.split(/[\\/]/).pop() ?? path,
    path,
    ext,
    size: 1024,
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

function createImageEntry(path: string, ext: string): DirEntry {
  return createEntry(path, { ext });
}

describe('图片编辑四动作的可见性', () => {
  it('四个动作 id 都在菜单动作表里（菜单投影 ⇄ 注册表一致）', () => {
    for (const action of IMAGE_ACTIONS) {
      expect(CONTEXT_MENU_ACTION_IDS).toContain(action);
      expect(IMAGE_ONLY_ACTIONS.has(action)).toBe(true);
    }
  });

  it('单个图片文件时四个动作都可见', () => {
    const entries = [createImageEntry('C:/Users/test/photo.jpg', 'jpg')];

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(true);
    }
  });

  it('白名单里的每种扩展名都算可编辑图片', () => {
    for (const extension of EDITABLE_IMAGE_EXTENSIONS) {
      const entries = [createImageEntry(`C:/Users/test/a.${extension}`, extension)];

      expect(isEditableImageEntry(entries[0])).toBe(true);
      expect(isContextMenuActionVisible('rotate-image-left', entries, { platform: 'windows' })).toBe(true);
    }
  });

  it('SVG 不是可编辑图片（矢量图没有像素可旋转）', () => {
    const entries = [createImageEntry('C:/Users/test/logo.svg', 'svg')];

    expect(isEditableImageEntry(entries[0])).toBe(false);

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(false);
    }
  });

  it('非图片文件不显示', () => {
    const entries = [createEntry('C:/Users/test/notes.txt', { ext: 'txt' })];

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(false);
    }
  });

  it('目录不显示', () => {
    const entries = [createEntry('C:/Users/test/folder', { isDir: true })];

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(false);
    }
  });

  it('多选不显示（哪怕每一项都是图片）', () => {
    const entries = [
      createImageEntry('C:/Users/test/a.jpg', 'jpg'),
      createImageEntry('C:/Users/test/b.png', 'png'),
    ];

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(false);
    }
  });

  it('远程路径不显示', () => {
    const entries = [createImageEntry('sfm://remote/conn-a/pub/a.jpg', 'jpg')];

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(false);
    }
  });

  it('虚拟位置不显示', () => {
    const entries = [createImageEntry(LOCATIONS_VIRTUAL_PATH, 'jpg')];

    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, entries, { platform: 'windows' })).toBe(false);
    }
  });

  it('空选择不显示', () => {
    for (const action of IMAGE_ACTIONS) {
      expect(isContextMenuActionVisible(action, [], { platform: 'windows' })).toBe(false);
    }
  });

  it('非图片动作不受这一层判定影响', () => {
    const entries = [createEntry('C:/Users/test/notes.txt', { ext: 'txt' })];

    expect(isImageOnlyActionVisible('rename', entries)).toBe(true);
    expect(isImageOnlyActionVisible('rotate-image-left', entries)).toBe(false);
  });
});
