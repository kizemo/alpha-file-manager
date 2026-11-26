// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-19（批次 23）：shell 设备清单的共享缓存。
//
// 判别点是「缓存里的内容」与「后端给的內容」一致：
//   · 成功 ⇒ 缓存里是刚拉到的设备；
//   · 失败 ⇒ 缓存被**清空**（不是保留旧值）。
//
// 「失败时保留旧值」是这里最容易踩的坑：位置页显示的是旧清单，
// 重建出来的却是另一份，签名比较永远不相等 ⇒ 每轮盘符轮询都触发一次无谓刷新。

import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';

const { invokeMock } = vi.hoisted(() => {
  return {
    invokeMock: vi.fn<(command: string, payload?: unknown) => Promise<unknown>>(),
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import {
  fetchShellDevices,
  getCachedShellDevices,
  subscribeShellDevices,
} from '@/utils/shell-devices';

const DEVICE = {
  name: 'Galaxy S22',
  shellPath: '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9&pid_2765',
  isFolder: true,
};

describe('shell-devices', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('向后端要设备清单并写进共享缓存', async () => {
    invokeMock.mockResolvedValue([DEVICE]);

    const devices = await fetchShellDevices();

    expect(invokeMock).toHaveBeenCalledWith('get_shell_devices');
    expect(devices).toEqual([DEVICE]);
    expect(getCachedShellDevices()).toEqual([DEVICE]);
  });

  it('拉取失败时清空缓存而不是保留旧值', async () => {
    invokeMock.mockResolvedValueOnce([DEVICE]);
    await fetchShellDevices();

    expect(getCachedShellDevices()).toHaveLength(1);

    invokeMock.mockRejectedValueOnce(new Error('后端还没有这个 command'));

    const devices = await fetchShellDevices();

    expect(devices).toEqual([]);
    expect(getCachedShellDevices()).toEqual([]);
  });

  it('后端返回的不是数组时按空清单处理', async () => {
    invokeMock.mockResolvedValueOnce(undefined);

    await expect(fetchShellDevices()).resolves.toEqual([]);
    expect(getCachedShellDevices()).toEqual([]);
  });

  it('订阅者会收到设备清单变化', async () => {
    const listener = vi.fn();

    subscribeShellDevices(listener);
    invokeMock.mockResolvedValueOnce([DEVICE]);

    await fetchShellDevices();

    expect(listener).toHaveBeenCalledWith([DEVICE]);
  });
});
