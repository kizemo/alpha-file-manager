// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION（2026-11-08，用户需求 1）：「位置」页按「分区 / 网盘 / 远程连接」分区。
//
// 判别点全在「条目有没有带 `location_group` 标记」：
//   - 普通目录里的条目永远不带标记 → 走旧的「目录 / 图片 / 视频 / 其它」口径；
//   - 「位置」页的条目全带标记 → 走新的「分区 / 网盘 / 远程连接」口径。
// 两套口径互不干扰，所以这里每组断言都配一条对照项。

import { describe, expect, it } from 'vitest';
import type { DirEntry, LocationGroup } from '@/types/dir-entry';
import {
  getFileBrowserSectionEntryOrder,
  getFileBrowserVisualEntryOrder,
  groupFileBrowserLocationEntries,
  hasLocationGroupEntries,
} from '../file-browser-entry-groups';
import { createFileBrowserVirtualRowModel } from '../utils/file-browser-virtual-rows';

function createEntry(
  name: string,
  locationGroup?: LocationGroup,
  overrides: Partial<DirEntry> = {},
): DirEntry {
  return {
    name,
    ext: null,
    path: `C:/items/${name}`,
    size: 0,
    item_count: null,
    modified_time: 0,
    accessed_time: 0,
    created_time: 0,
    mime: null,
    is_file: false,
    is_dir: true,
    is_symlink: false,
    is_hidden: false,
    ...(locationGroup ? { location_group: locationGroup } : {}),
    ...overrides,
  };
}

describe('file browser location groups', () => {
  describe('hasLocationGroupEntries', () => {
    it('detects entries carrying a location group marker', () => {
      expect(hasLocationGroupEntries([createEntry('C:', 'drives')])).toBe(true);
    });

    it('ignores ordinary directory entries (positive control)', () => {
      expect(hasLocationGroupEntries([createEntry('folder')])).toBe(false);
      expect(hasLocationGroupEntries([])).toBe(false);
    });
  });

  describe('groupFileBrowserLocationEntries', () => {
    it('splits entries into drives, netdisk and remote', () => {
      const entries = [
        createEntry('Local Disk (C:)', 'drives'),
        createEntry('Local Disk (D:)', 'drives'),
        createEntry('百度网盘', 'netdisk'),
        createEntry('速豹堂', 'remote'),
      ];

      const groups = groupFileBrowserLocationEntries(entries);

      expect(groups.drives.map(entry => entry.name)).toEqual(['Local Disk (C:)', 'Local Disk (D:)']);
      expect(groups.netdisk.map(entry => entry.name)).toEqual(['百度网盘']);
      expect(groups.remote.map(entry => entry.name)).toEqual(['速豹堂']);
    });

    it('falls back to the drives group for unmarked entries', () => {
      // 没带标记的条目只可能出现在「位置」页里，而那里默认就是分区组，
      // 不该凭空多出一个「未分组」区。
      const groups = groupFileBrowserLocationEntries([
        createEntry('Local Disk (C:)', 'drives'),
        createEntry('未标记条目'),
      ]);

      expect(groups.drives.map(entry => entry.name)).toEqual(['Local Disk (C:)', '未标记条目']);
      expect(groups.netdisk).toEqual([]);
      expect(groups.remote).toEqual([]);
    });
  });

  describe('getFileBrowserSectionEntryOrder', () => {
    it('orders location entries as drives, netdisk, remote', () => {
      const entries = [
        createEntry('速豹堂', 'remote'),
        createEntry('百度网盘', 'netdisk'),
        createEntry('Local Disk (C:)', 'drives'),
      ];

      expect(getFileBrowserSectionEntryOrder(entries).map(entry => entry.name)).toEqual([
        'Local Disk (C:)',
        '百度网盘',
        '速豹堂',
      ]);
    });

    it('keeps the legacy type grouping for ordinary directories (positive control)', () => {
      const entries = [
        createEntry('notes.txt', undefined, {
          is_dir: false,
          is_file: true,
          ext: 'txt',
        }),
        createEntry('photo.png', undefined, {
          is_dir: false,
          is_file: true,
          ext: 'png',
        }),
        createEntry('clips', undefined, { is_dir: true }),
      ];

      expect(getFileBrowserSectionEntryOrder(entries).map(entry => entry.name)).toEqual([
        'clips',
        'photo.png',
        'notes.txt',
      ]);
    });

    it('applies the same order to the list view visual order', () => {
      const entries = [
        createEntry('速豹堂', 'remote'),
        createEntry('Local Disk (C:)', 'drives'),
        createEntry('百度网盘', 'netdisk'),
      ];

      expect(getFileBrowserVisualEntryOrder(entries, 'list').map(entry => entry.name)).toEqual([
        'Local Disk (C:)',
        '百度网盘',
        '速豹堂',
      ]);
    });
  });

  describe('virtual row model', () => {
    it('builds drives and netdisk sections in the grid view', () => {
      const entries = [
        createEntry('Local Disk (C:)', 'drives'),
        createEntry('Local Disk (D:)', 'drives'),
        createEntry('百度网盘', 'netdisk'),
      ];

      const model = createFileBrowserVirtualRowModel({
        entries,
        layout: 'grid',
        columnCount: 1,
        gridGap: 8,
        thumbnailSize: 'medium',
      });

      expect(model.sections.map(section => section.key)).toEqual(['drives', 'netdisk']);
      // 两个分区 → 两个段头行；每个分区一行（columnCount = 1）。
      expect(model.rowCount).toBe(5);
    });

    it('drops empty groups instead of rendering an empty section header', () => {
      const entries = [
        createEntry('Local Disk (C:)', 'drives'),
        createEntry('百度网盘', 'netdisk'),
      ];

      const model = createFileBrowserVirtualRowModel({
        entries,
        layout: 'list',
        columnCount: 1,
        gridGap: 8,
        thumbnailSize: 'medium',
      });

      expect(model.sections.map(section => section.key)).toEqual(['drives', 'netdisk']);
      expect(model.sections.map(section => section.entryCount)).toEqual([1, 1]);
    });

    it('renders no section header when only the drives group exists', () => {
      // 用户需求：「如果没有进行配置，则留空（也不要显示标题）」。
      const entries = [
        createEntry('Local Disk (C:)', 'drives'),
        createEntry('Local Disk (D:)', 'drives'),
      ];

      const model = createFileBrowserVirtualRowModel({
        entries,
        layout: 'grid',
        columnCount: 1,
        gridGap: 8,
        thumbnailSize: 'medium',
      });

      expect(model.sections).toHaveLength(1);
      expect(model.sections[0]?.headerRowIndex).toBe(-1);
      expect(model.rowCount).toBe(2);
    });
  });
});
