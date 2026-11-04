// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L1 —— 快捷方式按**目标**定位。
//
// 判别点：`.lnk` 指向的目录还在 → 必须定位到**目标**；
// 目标没了 / 解析不出来 → 回落成「选中 .lnk 自己」，**不报错**。
//

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(command: string, payload: unknown) => Promise<unknown>>(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import {
  resolveRevealEntryTarget,
  type RevealEntryTargetDeps,
} from '@/utils/resolve-reveal-entry-target';

function createDeps(overrides?: Partial<RevealEntryTargetDeps>): RevealEntryTargetDeps {
  return {
    resolveShortcutTarget: vi.fn(async () => null),
    pathExists: vi.fn(async () => false),
    ...overrides,
  };
}

describe('resolveRevealEntryTarget', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('指向目录的快捷方式 → 打开**目标的父目录**并选中目标', async () => {
    const deps = createDeps({
      resolveShortcutTarget: vi.fn(async () => 'C:/Users/aleks/Projects'),
      pathExists: vi.fn(async () => true),
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk', deps),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks',
      focusPath: 'C:/Users/aleks/Projects',
    });

    expect(deps.resolveShortcutTarget).toHaveBeenCalledWith('C:/Users/aleks/Desktop/Projects.lnk');
  });

  it('目标已被删除 → 回落成「打开 .lnk 自己的父目录 + 选中 .lnk」，不报错', async () => {
    const deps = createDeps({
      resolveShortcutTarget: vi.fn(async () => 'C:/Users/aleks/Gone'),
      pathExists: vi.fn(async () => false),
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk', deps),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks/Desktop',
      focusPath: 'C:/Users/aleks/Desktop/Projects.lnk',
    });
  });

  it('解析不出目标 → 同样回落，且不去白跑一次存在性检查', async () => {
    const deps = createDeps({
      resolveShortcutTarget: vi.fn(async () => null),
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk', deps),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks/Desktop',
      focusPath: 'C:/Users/aleks/Desktop/Projects.lnk',
    });

    expect(deps.pathExists).not.toHaveBeenCalled();
  });

  it('普通文件 → 不查快捷方式，直接打开父目录 + 选中自己', async () => {
    const deps = createDeps();

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/report.pdf', deps),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks',
      focusPath: 'C:/Users/aleks/report.pdf',
    });

    expect(deps.resolveShortcutTarget).not.toHaveBeenCalled();
  });

  it('默认依赖走后端命令：解析成功且目标还在时定位目标', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'resolve_windows_directory_shortcut') {
        return 'C:/Users/aleks/Projects';
      }

      if (command === 'path_exists') {
        return true;
      }

      return null;
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk'),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks',
      focusPath: 'C:/Users/aleks/Projects',
    });
  });

  it('后端命令报错 → 保守回落，不把异常抛给调用方', async () => {
    invokeMock.mockRejectedValue(new Error('backend down'));

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk'),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks/Desktop',
      focusPath: 'C:/Users/aleks/Desktop/Projects.lnk',
    });
  });
});
