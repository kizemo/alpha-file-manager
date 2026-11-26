// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION (2026-11-07)：快捷方式「打开文件所在目录」的落点解析。
//
// 判别点：
//   - 指向**文件**的快捷方式 → 必须定位到**目标所在的目录**并选中目标
//     （2026-11-03 的实现只认目录目标，这是本轮修的主 bug：用户点了没反应）；
//   - 断链 / 后端解析不出 / 调用失败 → 回落成「打开 .lnk 自己的父目录 + 选中 .lnk」，
//     **不报错**。
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
    resolveTarget: vi.fn(async () => null),
    ...overrides,
  };
}

describe('resolveRevealEntryTarget', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('指向文件的快捷方式 → 打开**目标所在的目录**并选中目标', async () => {
    const deps = createDeps({
      resolveTarget: vi.fn(async () => ({
        directoryPath: 'D:/Program Files/UGREEN NAS',
        focusPath: 'D:/Program Files/UGREEN NAS/UGREEN NAS.exe',
        resolvedFromShortcut: true,
      })),
    });

    await expect(
      resolveRevealEntryTarget('C:/ProgramData/Start Menu/UGREEN NAS.lnk', deps),
    ).resolves.toEqual({
      directoryPath: 'D:/Program Files/UGREEN NAS',
      focusPath: 'D:/Program Files/UGREEN NAS/UGREEN NAS.exe',
      resolvedFromShortcut: true,
    });

    expect(deps.resolveTarget).toHaveBeenCalledWith(
      'C:/ProgramData/Start Menu/UGREEN NAS.lnk',
    );
  });

  it('指向目录的快捷方式 → 打开目标所在的目录并选中目标目录', async () => {
    const deps = createDeps({
      resolveTarget: vi.fn(async () => ({
        directoryPath: 'C:/Users/aleks',
        focusPath: 'C:/Users/aleks/Projects',
        resolvedFromShortcut: true,
      })),
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk', deps),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks',
      focusPath: 'C:/Users/aleks/Projects',
      resolvedFromShortcut: true,
    });
  });

  it('后端解析不出（断链）→ 回落成「打开 .lnk 自己的父目录 + 选中 .lnk」，不报错', async () => {
    const deps = createDeps({
      resolveTarget: vi.fn(async () => null),
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk', deps),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks/Desktop',
      focusPath: 'C:/Users/aleks/Desktop/Projects.lnk',
      resolvedFromShortcut: false,
    });
  });

  it('后端命令报错 → 保守回落，不把异常抛给调用方', async () => {
    invokeMock.mockRejectedValue(new Error('backend down'));

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk'),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks/Desktop',
      focusPath: 'C:/Users/aleks/Desktop/Projects.lnk',
      resolvedFromShortcut: false,
    });
  });

  it('默认依赖走后端命令：一次往返就拿到落点', async () => {
    invokeMock.mockResolvedValue({
      directoryPath: 'C:/Users/aleks',
      focusPath: 'C:/Users/aleks/report.pdf',
      resolvedFromShortcut: false,
    });

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/report.pdf'),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks',
      focusPath: 'C:/Users/aleks/report.pdf',
      resolvedFromShortcut: false,
    });

    expect(invokeMock).toHaveBeenCalledWith('resolve_reveal_entry_target', {
      path: 'C:/Users/aleks/report.pdf',
    });
  });

  it('后端返回空 → 同样回落，不抛错', async () => {
    invokeMock.mockResolvedValue(null);

    await expect(
      resolveRevealEntryTarget('C:/Users/aleks/Desktop/Projects.lnk'),
    ).resolves.toEqual({
      directoryPath: 'C:/Users/aleks/Desktop',
      focusPath: 'C:/Users/aleks/Desktop/Projects.lnk',
      resolvedFromShortcut: false,
    });
  });
});
