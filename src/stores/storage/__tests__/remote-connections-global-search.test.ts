// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「加入全局检索」这个字段的落盘与归一化（批次 37）。
//
// 钉住的是三件事：
// 1. 勾了要**真的写进盘上的那份记录**（不是只在内存里对）。
// 2. 老记录里根本没有这个键 —— 读回来必须是 `false`，
//    而不是 `undefined`：老用户的网盘不该莫名其妙地被塞进全局检索
//    （那要走网络、吃网盘 API 配额）。
// 3. 取消勾选要真的写回 `false`，不能是「把键删掉」。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import type { RemoteConnection, RemoteConnectionForm } from '@/types/remote';

/**
 * 存储桩带一个**真实的** map，并且把内容交给测试去读。
 *
 * 只断言 `store.connections` 抓不到落盘缺陷 —— 本项目已经因为
 * 「内存里对、盘上是 undefined」吃过一次亏，返工代价是整轮验收。
 */
const {
  lazyStoreSaveMock,
  storageContents,
} = vi.hoisted(() => ({
  lazyStoreSaveMock: vi.fn(),
  storageContents: new Map<string, unknown>(),
}));

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    async save(): Promise<void> {
      await lazyStoreSaveMock();
    }

    async get<T>(key: string): Promise<T | undefined> {
      return storageContents.get(key) as T | undefined;
    }

    async set(key: string, value: unknown): Promise<void> {
      storageContents.set(key, value);
    }
  },
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataDir: '/tmp/user-data',
      appUserDataRemoteConnectionsPath: '/tmp/user-data/user-remote-connections.json',
    },
  }),
}));

const invokeMock = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

function form(overrides: Partial<RemoteConnectionForm> = {}): RemoteConnectionForm {
  return {
    name: '备份服务器',
    protocol: 'ftp',
    host: 'ftp.example.com',
    port: 21,
    username: 'anonymous',
    initialPath: '/pub',
    localPath: '',
    proxy: {
      kind: 'direct',
    },
    password: '',
    clearPassword: false,
    ...overrides,
  };
}

/** 从存储桩里读回真正落盘的那份记录。 */
function storedConnections(): RemoteConnection[] {
  return (storageContents.get('connections') as RemoteConnection[]) ?? [];
}

describe('「加入全局检索」的落盘与归一化', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      // 启动引导里**没有**这个文件 → store 走 `loadStorageData()` 的真实读盘路径。
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      return undefined;
    });
  });

  it('勾选后落盘的那条记录里就是 true', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();

    const saved = await store.saveConnection(form({ includeInGlobalSearch: true }));

    expect(saved.includeInGlobalSearch).toBe(true);
    expect(storedConnections()[0].includeInGlobalSearch).toBe(true);
  });

  it('不勾选时落盘显式写 false，而不是「没有这个键」', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();

    const saved = await store.saveConnection(form());

    expect(saved.includeInGlobalSearch).toBe(false);
    expect(storedConnections()[0].includeInGlobalSearch).toBe(false);
  });

  it('老记录里没有这个键时，读回来是 false', async () => {
    // 批次 37 之前存的连接 —— 盘上根本没有 `includeInGlobalSearch`。
    storageContents.set('connections', [{
      id: 'c1',
      name: '老连接',
      protocol: 'ftp',
      host: 'a.example.com',
      port: 21,
      username: '',
      initialPath: '/',
      proxy: {
        kind: 'direct',
      },
    }]);

    const store = useRemoteConnectionsStore();
    await store.init();

    expect(store.connections).toHaveLength(1);
    expect(store.connections[0].includeInGlobalSearch).toBe(false);
  });

  it('取消勾选后存回 false', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();
    await store.saveConnection(form({
      id: 'c1',
      includeInGlobalSearch: true,
    }));

    const updated = await store.saveConnection(form({
      id: 'c1',
      includeInGlobalSearch: false,
    }));

    expect(updated.includeInGlobalSearch).toBe(false);
    expect(storedConnections()[0].includeInGlobalSearch).toBe(false);
  });

  it('重启后从 storage 读回的勾选状态还在', async () => {
    const first = useRemoteConnectionsStore();
    await first.init();
    await first.saveConnection(form({
      id: 'c1',
      includeInGlobalSearch: true,
    }));

    // 模拟重启：全新的 pinia + 同样的 storage 桩。
    setActivePinia(createPinia());
    const second = useRemoteConnectionsStore();
    await second.init();

    expect(second.connections).toHaveLength(1);
    expect(second.connections[0].includeInGlobalSearch).toBe(true);
  });

  it('globalSearchConnections 只列已勾选的那些连接', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();
    await store.saveConnection(form({
      id: 'c1',
      includeInGlobalSearch: true,
    }));
    await store.saveConnection(form({ id: 'c2' }));
    await store.saveConnection(form({
      id: 'c3',
      includeInGlobalSearch: true,
    }));

    expect(store.globalSearchConnections.map(connection => connection.id)).toEqual(['c1', 'c3']);
  });

  it('一条都没勾时 globalSearchConnections 是空的', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();
    await store.saveConnection(form({ id: 'c1' }));

    expect(store.globalSearchConnections).toEqual([]);
  });
});
