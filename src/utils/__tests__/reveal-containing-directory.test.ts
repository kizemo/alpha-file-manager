// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (2026-11-05)：「打开文件所在目录」共享执行逻辑的回归测试。
//
// 这段逻辑原先内嵌在导航器的 `use-file-browser-actions.ts` 里，
// 2026-11-05 为「全局搜索结果面板的右键菜单」抽成共享 util
// （`utils/reveal-containing-directory.ts`）。抽出来之后，两条入口
// （导航器右键菜单 / 搜索结果右键菜单）必须走**同一条**定位路径，
// 所以这里把三条分支都钉住：
//
// ① AFM 是默认资源管理器 → 走应用内定位（不把用户踢到系统 Explorer）；
// ② 否则 → 走原生 Explorer 的 `revealInExplorer`；
// ③ 目录条目跳过；定位失败要回调 `onError`（界面层要弹错误提示）。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

const mocks = vi.hoisted(() => ({
  revealInExplorer: vi.fn(async () => undefined),
  openNavigatorRevealEntry: vi.fn(),
  isDefaultFileManagerSafe: vi.fn(async () => false),
  resolveRevealEntryTarget: vi.fn(async () => ({
    directoryPath: 'C:/Users/test',
    focusPath: 'C:/Users/test/a.txt',
    resolvedFromShortcut: false,
  })),
}));

vi.mock('@/utils/open-in-explorer', () => ({
  revealInExplorer: mocks.revealInExplorer,
}));

vi.mock('@/utils/open-navigator-directory', () => ({
  openNavigatorRevealEntry: mocks.openNavigatorRevealEntry,
}));

vi.mock('@/utils/default-file-manager', () => ({
  isDefaultFileManagerSafe: mocks.isDefaultFileManagerSafe,
}));

vi.mock('@/utils/resolve-reveal-entry-target', () => ({
  resolveRevealEntryTarget: mocks.resolveRevealEntryTarget,
}));

import { revealEntriesContainingDirectory } from '../reveal-containing-directory';

function createEntry(path: string, isDir: boolean): DirEntry {
  return {
    name: path.split(/[\\/]/).pop() ?? path,
    path,
    is_file: !isDir,
    is_dir: isDir,
    is_hidden: false,
    is_symlink: false,
    size: 0,
    item_count: null,
    created_time: 0,
    modified_time: 0,
    accessed_time: 0,
    ext: null,
    mime: null,
  };
}

const router = { push: vi.fn() } as never;

describe('revealEntriesContainingDirectory — 打开文件所在目录', () => {
  beforeEach(() => {
    mocks.revealInExplorer.mockClear();
    mocks.openNavigatorRevealEntry.mockClear();
    mocks.resolveRevealEntryTarget.mockClear();
    mocks.resolveRevealEntryTarget.mockResolvedValue({
      directoryPath: 'C:/Users/test',
      focusPath: 'C:/Users/test/a.txt',
      resolvedFromShortcut: false,
    });
    mocks.isDefaultFileManagerSafe.mockReset();
    mocks.isDefaultFileManagerSafe.mockResolvedValue(false);
  });

  it('走原生 Explorer 定位（AFM 不是默认资源管理器）', async () => {
    await revealEntriesContainingDirectory([createEntry('C:/Users/test/a.txt', false)], {
      router,
    });

    expect(mocks.revealInExplorer).toHaveBeenCalledWith('C:/Users/test/a.txt');
    expect(mocks.openNavigatorRevealEntry).not.toHaveBeenCalled();
  });

  it('AFM 是默认资源管理器时改走应用内定位', async () => {
    mocks.isDefaultFileManagerSafe.mockResolvedValue(true);

    await revealEntriesContainingDirectory([createEntry('C:/Users/test/a.txt', false)], {
      router,
      filterQuery: 'abc',
    });

    expect(mocks.openNavigatorRevealEntry).toHaveBeenCalledWith(
      router,
      'C:/Users/test/a.txt',
      {
        filterQuery: 'abc',
        reuseCurrentPaneOnly: true,
      },
    );
    expect(mocks.revealInExplorer).not.toHaveBeenCalled();
  });

  it('目录条目跳过（目录的父目录就是所在目录本身）', async () => {
    await revealEntriesContainingDirectory([createEntry('C:/Users/test/folder', true)], {
      router,
    });

    expect(mocks.revealInExplorer).not.toHaveBeenCalled();
    expect(mocks.openNavigatorRevealEntry).not.toHaveBeenCalled();
  });

  it('快捷方式 → 传给 Explorer 的是**目标**路径，不是 .lnk 自己', async () => {
    mocks.resolveRevealEntryTarget.mockResolvedValueOnce({
      directoryPath: 'D:/Program Files/BaiduNetdisk',
      focusPath: 'D:/Program Files/BaiduNetdisk/BaiduNetdisk.exe',
      resolvedFromShortcut: true,
    });

    await revealEntriesContainingDirectory([
      createEntry('C:/ProgramData/Start Menu/02/baidu.lnk', false),
    ], {
      router,
    });

    expect(mocks.revealInExplorer).toHaveBeenCalledWith(
      'D:/Program Files/BaiduNetdisk/BaiduNetdisk.exe',
    );
  });

  it('快捷方式解析不出目标 → 只传所在目录，绝不把 .lnk 交给 Explorer', async () => {
    mocks.resolveRevealEntryTarget.mockResolvedValueOnce({
      directoryPath: 'C:/ProgramData/Start Menu/02',
      focusPath: 'C:/ProgramData/Start Menu/02/baidu.lnk',
      resolvedFromShortcut: false,
    });

    await revealEntriesContainingDirectory([
      createEntry('C:/ProgramData/Start Menu/02/baidu.lnk', false),
    ], {
      router,
    });

    expect(mocks.revealInExplorer).toHaveBeenCalledWith('C:/ProgramData/Start Menu/02');
  });

  it('定位失败要回调 onError（界面层弹提示）', async () => {
    mocks.revealInExplorer.mockRejectedValueOnce(new Error('explorer is gone'));

    const onError = vi.fn();

    await revealEntriesContainingDirectory([createEntry('C:/Users/test/a.txt', false)], {
      router,
      onError,
    });

    expect(onError).toHaveBeenCalledTimes(1);
    expect((onError.mock.calls[0][0] as Error).message).toBe('explorer is gone');
  });
});
