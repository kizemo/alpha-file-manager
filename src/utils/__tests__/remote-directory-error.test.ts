// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-11 (batch 9, T3 / R8)。
// 第 11 条真机 FAIL 的复现判据：断网时横幅逐字打印
// 「无法访问网络 — 访问夸克网盘失败：error sending request for url
//  (https://drive-pc.quark.cn/1/clouddrive/file/sort?…)」——
// reqwest 的原始 Display 被当成「错误说明」抛给用户，既不可行动也泄漏内部 API 路径。
// 后端 `transport_error` 已改为按分类给中文文案，这里是前端的兜底：
// 只要 `message` 还是原始传输串，就必须换成分类文案。

import { describe, expect, it } from 'vitest';
import { describeRemoteError } from '@/utils/remote-directory';

describe('describeRemoteError 的分类文案兜底（第 11 条）', () => {
  it('unreachable 的原始 reqwest 串换成分类文案，不含 URL', () => {
    const text = describeRemoteError({
      kind: 'unreachable',
      message:
        '访问夸克网盘失败：error sending request for url (https://drive-pc.quark.cn/1/clouddrive/file/sort?pfid=0&name=)',
    });

    expect(text).toContain('无法连接夸克网盘');
    expect(text).toContain('网络');
    expect(text).not.toContain('error sending request');
    expect(text).not.toContain('drive-pc.quark.cn');
    expect(text).not.toContain('clouddrive');
  });

  /**
   * FORK-MODIFICATION 2026-11-11：分类必须与后端 `RemoteErrorKind` 的**真实**集合一致。
   * 本项目没有 `timeout` / `redirect` 分类（`classify_transport` 只回
   * unreachable / disconnected / protocol / auth / unknown…），超时并入 `unreachable`。
   * 上一版用例照抄了不存在的 kind，等于测了一条运行时不会到达的分支。
   */
  it('连接中断与协议异常各自给出可行动的文案', () => {
    expect(
      describeRemoteError({
        kind: 'disconnected',
        message: '访问夸克网盘失败：error decoding response body',
      }),
    ).toContain('连接中断');

    expect(
      describeRemoteError({
        kind: 'protocol',
        message: 'error too many redirects',
      }),
    ).toContain('无法识别的响应');
  });

  it('超时原始串归入 unreachable 文案，不再冒充新分类', () => {
    const text = describeRemoteError({
      kind: 'unreachable',
      message: '访问夸克网盘失败：operation timed out',
    });

    expect(text).toContain('无法连接夸克网盘');
    expect(text).not.toContain('operation timed out');
  });

  it('后端已经给了中文文案时原样透传，不做二次改写', () => {
    const message = '夸克网盘会话已失效，请重新授权。';

    expect(describeRemoteError({
      kind: 'auth',
      message,
    })).toBe(message);
  });

  it('auth 的原始串换成「重新授权」，与 T2 的 412 分类闭环', () => {
    const text = describeRemoteError({
      kind: 'auth',
      message: 'error sending request for url (https://drive-pc.quark.cn/x)',
    });

    expect(text).toContain('重新授权');
  });

  it('message 缺失时用分类文案，不再打印 [object Object]', () => {
    expect(
      describeRemoteError({
        kind: 'unreachable',
        message: '   ',
      }),
    ).toContain('无法连接夸克网盘');
  });

  it('本地错误与未知分类原样处理（不许把本地链路也改写）', () => {
    expect(describeRemoteError(new Error('本地错误'))).toBe('本地错误');
    expect(describeRemoteError('一条字符串')).toBe('一条字符串');
    expect(
      describeRemoteError({
        kind: 'not_found',
        message: '网盘里找不到这个条目',
      }),
    ).toBe('网盘里找不到这个条目');
  });
});
