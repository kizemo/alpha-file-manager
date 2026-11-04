// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L3 —— 启动参数里的 select 请求。
//
// 判别点：select 参数**不能**混进「打开目录」候选里；
// select 目标的语义是「父目录 + 选中条目」，不是「进入该目录」。
//

import { describe, expect, it } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';
import {
  getLaunchDirectoryCandidates,
  getLaunchSelectRequest,
  hasLaunchDirectoryCandidates,
  resolveSelectLaunchTarget,
  type LaunchContext,
} from '@/utils/launch-directories';

function createLaunchContext(args: string[]): LaunchContext {
  return {
    args: ['C:/Apps/Alpha File Manager.exe', ...args],
    cwd: null,
    executableDir: 'C:/Apps',
    hadAbsorbedShellPaths: false,
    hadDelegatedShellPaths: false,
  };
}

function createDirEntry(path: string, overrides?: Partial<DirEntry>): DirEntry {
  return {
    name: path.split(/[\\/]/).pop() ?? path,
    ext: null,
    path,
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
    ...overrides,
  };
}

describe('启动参数里的 select 请求', () => {
  it('认得启动器转发过来的规范形式 --sigma-select=<路径>', () => {
    expect(getLaunchSelectRequest(
      createLaunchContext(['--sigma-select=C:/Users/aleks/report.pdf']),
    )).toEqual({ selectPath: 'C:/Users/aleks/report.pdf' });
  });

  it('认得 Explorer 的内联形式 /select,<路径>', () => {
    expect(getLaunchSelectRequest(
      createLaunchContext(['/select,C:/Users/aleks/report.pdf']),
    )).toEqual({ selectPath: 'C:/Users/aleks/report.pdf' });
  });

  it('认得两个参数的形式 /select <路径>', () => {
    expect(getLaunchSelectRequest(
      createLaunchContext(['/select', 'C:/Users/aleks/report.pdf']),
    )).toEqual({ selectPath: 'C:/Users/aleks/report.pdf' });
  });

  it('select 参数不会混进「打开目录」候选里', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext([
      '/select,C:/Users/aleks/report.pdf',
      'C:/Users/aleks/Documents',
    ]))).toEqual([
      'C:/Users/aleks/Documents',
    ]);
  });

  it('两个参数形式下，被选中的那个路径也不会混进候选里', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext([
      '/select',
      'C:/Users/aleks/report.pdf',
    ]))).toEqual([]);
  });

  it('只有 select 请求时也算「有启动目标」（否则启动页会盖掉结果）', () => {
    expect(hasLaunchDirectoryCandidates(
      createLaunchContext(['/select,C:/Users/aleks/report.pdf']),
    )).toBe(true);
  });

  it('没有 select 请求时维持原样', () => {
    expect(getLaunchSelectRequest(
      createLaunchContext(['C:/Users/aleks/Documents']),
    )).toBeNull();

    expect(getLaunchDirectoryCandidates(
      createLaunchContext(['C:/Users/aleks/Documents']),
    )).toEqual([
      'C:/Users/aleks/Documents',
    ]);
  });

  it('select 目标解析成「父目录 + 选中条目」（哪怕选中的是目录）', async () => {
    async function getDirEntry(path: string) {
      return createDirEntry(path, {
        is_dir: true,
        is_file: false,
      });
    }

    await expect(resolveSelectLaunchTarget('C:/Users/aleks/Documents', getDirEntry))
      .resolves.toEqual({
        directoryPath: 'C:/Users/aleks',
        focusPath: 'C:/Users/aleks/Documents',
      });
  });

  it('select 目标解析不出来时返回 null（不硬造一个目标）', async () => {
    async function getDirEntry() {
      return null;
    }

    await expect(resolveSelectLaunchTarget('C:/Users/aleks/missing.txt', getDirEntry))
      .resolves.toBeNull();
  });
});
