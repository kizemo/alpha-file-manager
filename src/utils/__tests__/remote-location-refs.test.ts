// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  afterEach, describe, expect, it, vi,
} from 'vitest';
import {
  getRemoteLocationRefs,
  setRemoteLocationRefs,
  subscribeRemoteLocationRefs,
} from '@/utils/remote-location-refs';
import type { RemoteConnection } from '@/types/remote';

// FORK-MODIFICATION 2026-11-08：位置页的远程条目变化通知。
//
// 背景：位置页的条目是「读目录时顺手拼」的，触发重读的原本只有 `watch(sharedDrives)`。
// 连接列表变了（新增/改名/删除）位置页不会自己重读，要等下一次切页或刷新。
// 这里用注册表的订阅机制补上这条路 —— 注册表是纯工具模块，不 import store。

function createConnection(overrides: Partial<RemoteConnection> = {}): RemoteConnection {
  return {
    id: 'conn-remote',
    name: '示例服务器',
    protocol: 'sftp',
    host: 'example.test',
    port: 22,
    username: 'demo',
    initialPath: '/',
    proxy: { kind: 'direct' },
    ...overrides,
  };
}

describe('remote-location-refs 订阅机制', () => {
  const disposers: Array<() => void> = [];

  afterEach(() => {
    while (disposers.length > 0) {
      disposers.pop()?.();
    }

    // 注册表是模块级状态，不清掉会污染后面所有用例。
    setRemoteLocationRefs([]);
  });

  it('连接集合变化时通知订阅者', () => {
    const listener = vi.fn();
    disposers.push(subscribeRemoteLocationRefs(listener));

    setRemoteLocationRefs([createConnection({ id: 'conn-a' })]);
    setRemoteLocationRefs([createConnection({ id: 'conn-a' }), createConnection({ id: 'conn-b' })]);

    expect(listener).toHaveBeenCalledTimes(2);
  });

  it('内容没变时不通知（避免无意义的目录重读）', () => {
    const listener = vi.fn();
    disposers.push(subscribeRemoteLocationRefs(listener));

    setRemoteLocationRefs([createConnection({ id: 'conn-a' })]);
    setRemoteLocationRefs([createConnection({ id: 'conn-a' })]);
    setRemoteLocationRefs([createConnection({ id: 'conn-a' })]);

    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('连接改名算内容变化', () => {
    const listener = vi.fn();
    disposers.push(subscribeRemoteLocationRefs(listener));

    setRemoteLocationRefs([createConnection({
      id: 'conn-a',
      name: '旧名字',
    })]);
    setRemoteLocationRefs([createConnection({
      id: 'conn-a',
      name: '新名字',
    })]);

    expect(listener).toHaveBeenCalledTimes(2);
    expect(getRemoteLocationRefs()[0]?.name).toBe('新名字');
  });

  it('退订之后不再收到通知', () => {
    const listener = vi.fn();
    const unsubscribe = subscribeRemoteLocationRefs(listener);

    setRemoteLocationRefs([createConnection({ id: 'conn-a' })]);
    unsubscribe();
    setRemoteLocationRefs([createConnection({ id: 'conn-a' }), createConnection({ id: 'conn-b' })]);

    expect(listener).toHaveBeenCalledTimes(1);
  });

  it('某个监听者抛错不影响其它监听者，也不打断写入路径', () => {
    const bad = vi.fn(() => {
      throw new Error('boom');
    });
    const good = vi.fn();
    disposers.push(subscribeRemoteLocationRefs(bad));
    disposers.push(subscribeRemoteLocationRefs(good));

    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => {});
    disposers.push(() => consoleError.mockRestore());

    expect(() => setRemoteLocationRefs([createConnection({ id: 'conn-a' })])).not.toThrow();
    expect(bad).toHaveBeenCalledTimes(1);
    expect(good).toHaveBeenCalledTimes(1);
  });

  it('非法 id 的连接被跳过，不进注册表', () => {
    const listener = vi.fn();
    disposers.push(subscribeRemoteLocationRefs(listener));

    setRemoteLocationRefs([createConnection({ id: 'conn-a' })]);
    setRemoteLocationRefs([createConnection({ id: '' })]);

    expect(getRemoteLocationRefs()).toHaveLength(0);
    expect(listener).toHaveBeenCalledTimes(2);
  });
});
