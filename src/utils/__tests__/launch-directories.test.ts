// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it, vi } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';
import {
  getLaunchDirectoryCandidates,
  getLaunchShellPathCandidates,
  hasLaunchDirectoryCandidates,
  resolveLaunchTargetsFromArgs,
  resolvePathLaunchTarget,
  type LaunchContext,
} from '@/utils/launch-directories';
import { LOCATIONS_VIRTUAL_PATH } from '@/utils/virtual-locations';

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

describe('launch-directories', () => {
  function createLaunchContext(overrides?: Partial<LaunchContext>): LaunchContext {
    return {
      args: ['C:/Apps/Sigma File Manager.exe'],
      cwd: null,
      executableDir: 'C:/Apps',
      hadAbsorbedShellPaths: false,
      hadDelegatedShellPaths: false,
      ...overrides,
    };
  }

  it('filters out executable paths, flags, empty values, and duplicates', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        'C:/Users/aleks/Documents',
        '',
        '   ',
        '--sigma-autostart',
        'C:/Users/aleks/Documents',
        'C:/Users/aleks/Pictures',
      ],
    }))).toEqual([
      'C:/Users/aleks/Documents',
      'C:/Users/aleks/Pictures',
    ]);
  });

  it('deduplicates Windows paths without regard to case', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        'C:/Users/aleks/Documents',
        'c:/users/aleks/documents',
      ],
    }))).toEqual([
      'C:/Users/aleks/Documents',
    ]);
  });

  it('normalizes bare Windows drive roots from args', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        'C:',
      ],
    }))).toEqual([
      'C:/',
    ]);
  });

  it('normalizes quoted Windows drive roots from args', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        'C:"',
      ],
    }))).toEqual([
      'C:/',
    ]);
  });

  it('normalizes fully quoted paths from args', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        '"C:/Users/aleks/Documents"',
      ],
    }))).toEqual([
      'C:/Users/aleks/Documents',
    ]);
  });

  it('preserves apostrophes in path names', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        'C:/Users/O\'Connor/Documents',
      ],
    }))).toEqual([
      'C:/Users/O\'Connor/Documents',
    ]);
  });

  it('opens locations for absorbed shell launches without path arguments', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      cwd: 'C:/Users/aleks/Documents',
      hadAbsorbedShellPaths: true,
    }))).toEqual([
      LOCATIONS_VIRTUAL_PATH,
    ]);
  });

  it('does not fall back to cwd during regular startup without absorbed shell paths', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      cwd: 'C:/Users/aleks/Documents',
    }))).toEqual([]);
  });

  it('opens locations for absorbed shell launches regardless of cwd normalization', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      cwd: 'C:\\Users\\aleks\\Documents',
      hadAbsorbedShellPaths: true,
    }))).toEqual([
      LOCATIONS_VIRTUAL_PATH,
    ]);
  });

  it('opens locations when cwd matches the executable directory during absorbed shell launch', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      cwd: 'C:/Apps',
      hadAbsorbedShellPaths: true,
    }))).toEqual([
      LOCATIONS_VIRTUAL_PATH,
    ]);
  });

  it('opens locations for absorbed shell launches with Windows shell cwd paths', () => {
    expect(getLaunchDirectoryCandidates(createLaunchContext({
      cwd: 'C:/Windows/System32',
      executableDir: 'C:/Program Files/Sigma File Manager',
      hadAbsorbedShellPaths: true,
    }))).toEqual([
      LOCATIONS_VIRTUAL_PATH,
    ]);
  });

  it('keeps only resolved directory entries', async () => {
    const getDirEntry = vi.fn(async (path: string): Promise<DirEntry | null> => {
      if (path === 'C:/Users/aleks/Documents') {
        return createDirEntry(path);
      }

      if (path === 'C:/Users/aleks/file.txt') {
        return createDirEntry(path, {
          is_dir: false,
          is_file: true,
        });
      }

      return null;
    });

    await expect(resolveLaunchTargetsFromArgs(createLaunchContext({
      args: [
        'C:/Apps/Sigma File Manager.exe',
        'C:/Users/aleks/Documents',
        'C:/Users/aleks/file.txt',
        '--sigma-autostart',
        'C:/Users/aleks/missing',
      ],
    }), getDirEntry)).resolves.toEqual([
      {
        directoryPath: 'C:/Users/aleks/Documents',
        focusPath: null,
      },
      {
        directoryPath: 'C:/Users/aleks',
        focusPath: 'C:/Users/aleks/file.txt',
      },
    ]);
  });

  it('resolves directory paths from dir entry metadata', async () => {
    const getDirEntry = vi.fn(async () => createDirEntry('D:/Resolved/Documents'));

    await expect(resolvePathLaunchTarget('C:/Users/aleks/Documents', getDirEntry))
      .resolves.toEqual({
        directoryPath: 'D:/Resolved/Documents',
        focusPath: null,
      });
  });

  it('returns null when dir entry lookup fails', async () => {
    const getDirEntry = vi.fn(async () => null);

    await expect(resolvePathLaunchTarget('C:/Users/aleks/missing', getDirEntry))
      .resolves.toBeNull();
  });

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 设备（手机 / MTP）的启动参数。
  //
  // 故障链：手机传给 AFM 的 argv 是
  //   `::{20D04FE0-...}\?\usb#vid_22d9&pid_2765...`
  // 改动前 `normalizeLaunchPathCandidate` 把它 `\`→`/` 改坏 ⇒ 候选被判成空
  // ⇒ 兜底落到「位置」页 ⇒ 用户看到的是「双击手机，AFM 跳到位置页」。
  describe('shell 设备启动参数', () => {
    const devicePath = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9&pid_2765&mi_00'
      + '#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}\\SID-{10001,,1001925963776}';

    function createDeviceLaunchContext(overrides?: Partial<LaunchContext>): LaunchContext {
      return createLaunchContext({
        args: ['C:/Apps/Sigma File Manager.exe', devicePath],
        ...overrides,
      });
    }

    it('shell 路径候选原样保留，不被规范化改坏', () => {
      expect(getLaunchDirectoryCandidates(createDeviceLaunchContext())).toEqual([devicePath]);
    });

    it('存在 shell 候选时不再被「位置」页兜底顶掉', () => {
      // 改动前：shell 路径被改坏 ⇒ 候选为空 ⇒ `candidates.length === 0`
      // ⇒ 兜底把用户丢到位置页。
      const context = createDeviceLaunchContext({ hadAbsorbedShellPaths: true });

      expect(getLaunchDirectoryCandidates(context)).toEqual([devicePath]);
      expect(getLaunchDirectoryCandidates(context)).not.toContain(LOCATIONS_VIRTUAL_PATH);
      expect(hasLaunchDirectoryCandidates(context)).toBe(true);
    });

    it('确实一个候选都没有时才兜底到「位置」页', () => {
      expect(getLaunchDirectoryCandidates(createLaunchContext({
        args: ['C:/Apps/Sigma File Manager.exe'],
        hadAbsorbedShellPaths: true,
      }))).toEqual([
        LOCATIONS_VIRTUAL_PATH,
      ]);
    });

    it('getLaunchShellPathCandidates 只挑出 shell 路径', () => {
      expect(getLaunchShellPathCandidates(createDeviceLaunchContext({
        args: ['C:/Apps/Sigma File Manager.exe', 'C:/Users/aleks/Documents', devicePath],
      }))).toEqual([devicePath]);
    });

    it('resolvePathLaunchTarget 能解析 shell 路径（设备与普通目录一视同仁）', async () => {
      const getDirEntry = vi.fn(async (path: string): Promise<DirEntry | null> => createDirEntry(path));

      await expect(resolvePathLaunchTarget(devicePath, getDirEntry)).resolves.toEqual({
        directoryPath: devicePath,
        focusPath: null,
      });
    });

    it('设备打不开时解析不出目标（调用方据此给用户提示）', async () => {
      const getDirEntry = vi.fn(async () => null);

      await expect(resolvePathLaunchTarget(devicePath, getDirEntry)).resolves.toBeNull();
    });
  });
});
