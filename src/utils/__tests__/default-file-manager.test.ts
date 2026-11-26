// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L2 的负对照 ——
// 「读不到自己是不是默认资源管理器」时必须**保守地**按「不是」处理，
// 而不是把异常抛出去让整个动作失败。
//

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(command: string, payload?: unknown) => Promise<unknown>>(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

import {
  isDefaultFileManager,
  isDefaultFileManagerSafe,
} from '@/utils/default-file-manager';

describe('isDefaultFileManagerSafe', () => {
  beforeEach(() => {
    invokeMock.mockReset();
  });

  it('读到「是默认资源管理器」时如实返回 true', async () => {
    invokeMock.mockResolvedValue(true);

    await expect(isDefaultFileManagerSafe()).resolves.toBe(true);
  });

  it('读取失败 → 保守地按「不是默认资源管理器」处理，不抛异常', async () => {
    invokeMock.mockRejectedValue(new Error('backend down'));

    await expect(isDefaultFileManagerSafe()).resolves.toBe(false);
  });

  it('底层命令仍然按原样抛错（错误没有被悄悄吞掉）', async () => {
    invokeMock.mockRejectedValue(new Error('backend down'));

    await expect(isDefaultFileManager()).rejects.toThrow('backend down');
  });
});
