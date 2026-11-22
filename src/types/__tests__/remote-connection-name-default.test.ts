// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「新建连接时预填 provider 显示名」的判定是纯函数（批次 37）。
//
// 这里钉住三条不可省的行为：
//
// 1. 预填出来的是 **i18n 键对应的显示名**，不是硬编码中文 ——
//    同一个界面里按钮写着「阿里云盘」、输入框预填的是别的名字，
//    是最刺眼的一种不一致。
// 2. 编辑既有连接时**绝不覆盖**存下来的名字：那是用户自己起的。
// 3. FTP / SFTP 不是网盘，不预填 —— 给它们塞网盘名是错的。

import { describe, expect, it, vi } from 'vitest';
import {
  getGlobalSearchConnectionIds,
  resolveRemoteConnectionNameDefault,
} from '@/types/remote';
import type { RemoteProtocol } from '@/types/remote';

/** 三家网盘各自的 provider 显示名键（键已存在于语言包里，不新增）。 */
const NETDISK_CASES: [RemoteProtocol, string][] = [
  ['baidu_pan', 'settings.remote.netdisk.baidu'],
  ['quark_pan', 'settings.remote.netdisk.quark'],
  ['aliyun_pan', 'settings.remote.netdisk.aliyun'],
];

/** 不是网盘的那几个协议。 */
const REMOTE_CASES: RemoteProtocol[] = ['ftp', 'ftps_explicit', 'ftps_implicit', 'sftp'];

describe('resolveRemoteConnectionNameDefault', () => {
  it.each(NETDISK_CASES)('新建 %s 时预填 %s', (protocol, expectedKey) => {
    const translate = vi.fn((key: string) => `T:${key}`);

    const name = resolveRemoteConnectionNameDefault(protocol, undefined, translate);

    expect(name).toBe(`T:${expectedKey}`);
    // 走的是 i18n 键，不是硬编码的中文串 —— 换语言时预填的名字会跟着变。
    expect(translate).toHaveBeenCalledWith(expectedKey);
  });

  it.each(REMOTE_CASES)('新建 %s 时名称留空', (protocol) => {
    const translate = vi.fn((key: string) => `T:${key}`);

    const name = resolveRemoteConnectionNameDefault(protocol, undefined, translate);

    expect(name).toBe('');
    expect(translate).not.toHaveBeenCalled();
  });

  it('编辑既有连接时原样保留存下来的名字', () => {
    const translate = vi.fn((key: string) => `T:${key}`);

    const name = resolveRemoteConnectionNameDefault('aliyun_pan', '我的网盘', translate);

    expect(name).toBe('我的网盘');
    expect(translate).not.toHaveBeenCalled();
  });

  it('编辑既有连接且存下来的名字是空串时也不预填', () => {
    // 空串同样算「用户自己起的名」：预填覆盖它等于替用户改了一个他刚清空的名字。
    const translate = vi.fn((key: string) => `T:${key}`);

    const name = resolveRemoteConnectionNameDefault('baidu_pan', '', translate);

    expect(name).toBe('');
    expect(translate).not.toHaveBeenCalled();
  });

  it('传 null 等同于新建，照样预填', () => {
    const translate = vi.fn((key: string) => `T:${key}`);

    const name = resolveRemoteConnectionNameDefault('quark_pan', null, translate);

    expect(name).toBe('T:settings.remote.netdisk.quark');
  });
});

describe('getGlobalSearchConnectionIds', () => {
  it('只挑显式勾选（=== true）的那些连接', () => {
    const ids = getGlobalSearchConnectionIds([
      {
        id: 'a',
        includeInGlobalSearch: true,
      },
      {
        id: 'b',
      },
      {
        id: 'c',
        includeInGlobalSearch: false,
      },
      {
        id: 'd',
        includeInGlobalSearch: true,
      },
    ]);

    expect(ids).toEqual(['a', 'd']);
  });

  it('一条都没勾时是空数组（不是 undefined）', () => {
    const ids = getGlobalSearchConnectionIds([
      {
        id: 'a',
      },
    ]);

    expect(ids).toEqual([]);
  });
});
