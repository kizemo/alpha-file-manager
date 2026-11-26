// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 用户 2026-11-08：网盘打开后，标签页上摆的应该是「百度网盘」，
// 而不是连接 id 0736c40a… 与 `sfm://remote/0736c40a…`。
//
// 断言的是**标题里不能出现连接 id** —— 这正是用户看到的那个回归。

import { describe, expect, it } from 'vitest';
import { buildRemoteTabLabels } from '@/modules/tab-bar/utils/remote-tab-labels';
import { parseRemotePath } from '@/utils/remote-paths';

const CONNECTION_ID = '0736c40a51db4fd19f711c0b6934f103';

function labelsOf(path: string, connectionName?: string) {
  const parsed = parseRemotePath(path);

  if (!parsed) {
    throw new Error(`not a remote path: ${path}`);
  }

  return buildRemoteTabLabels(parsed, connectionName);
}

describe('远程标签页的标题', () => {
  it('连接根显示连接名，不显示连接 id', () => {
    const labels = labelsOf(`sfm://remote/${CONNECTION_ID}`, '百度网盘');

    expect(labels.title).toBe('百度网盘');
    expect(labels.title).not.toContain(CONNECTION_ID);
  });

  it('子目录显示「连接名 / 目录名」', () => {
    const labels = labelsOf(`sfm://remote/${CONNECTION_ID}/prompts`, '百度网盘');

    expect(labels.title).toBe('百度网盘 / prompts');
    expect(labels.title).not.toContain(CONNECTION_ID);
  });

  it('副标题是远端自己的路径，不是 sfm:// 串', () => {
    const labels = labelsOf(`sfm://remote/${CONNECTION_ID}/prompts`, '百度网盘');

    expect(labels.subtitle).toBe('/prompts');
    expect(labels.subtitle).not.toContain('sfm://');
    expect(labels.subtitle).not.toContain(CONNECTION_ID);
  });

  it('连接根的副标题是根', () => {
    const labels = labelsOf(`sfm://remote/${CONNECTION_ID}`, '百度网盘');

    expect(labels.subtitle).toBe('/');
  });

  it('查不到连接名时标题为空，交由调用方退化', () => {
    const labels = labelsOf(`sfm://remote/${CONNECTION_ID}`, undefined);

    expect(labels.title).toBeNull();
    expect(labels.subtitle).toBe('/');
  });
});
