// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 百度网盘连接的**保存路径**护栏。
//
// 这条路径与 FTP / SFTP 有两处根本不同，断了都不会报错，只会「看起来保存成功」：
//
// 1. **凭据走的不是同一个 command**：百度的凭据是一整段 Cookie，
//    必须交给后端解析（BDUSS / STOKEN），塞进通用的 `remote_save_credential`
//    等于把没解析过的垃圾也存成「已登录」。
// 2. **校验不能要求主机与端口**：百度网盘没有自选服务器，
//    拿 FTP 那套规则去卡，用户根本填不出一个能保存的表单。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import type { RemoteConnectionForm } from '@/types/remote';

const { invokeMock, lazyStoreSaveMock, storageContents } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
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

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataDir: '/tmp/user-data',
      appUserDataRemoteConnectionsPath: '/tmp/user-data/user-remote-connections.json',
    },
  }),
}));

function baiduForm(overrides: Partial<RemoteConnectionForm> = {}): RemoteConnectionForm {
  return {
    name: '百度网盘',
    protocol: 'baidu_pan',
    // 主机与端口**故意留空**：百度的表单不该要求用户填它们。
    host: '',
    port: 0,
    username: '',
    initialPath: '/',
    localPath: '',
    proxy: {
      kind: 'direct',
    },
    password: 'BDUSS=abc123; STOKEN=def456',
    clearPassword: false,
    ...overrides,
  };
}

describe('百度网盘连接的保存', () => {
  beforeEach(async () => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      return undefined;
    });
    lazyStoreSaveMock.mockReset();
    storageContents.clear();

    // storage 必须先就绪 —— 否则 `saveConnection` 会停在写盘那一步抛
    // 「连接列表尚未加载完成」，而这条用例想验的是凭据**怎么写**。
    const store = useRemoteConnectionsStore();
    await store.init();
  });

  it('凭据走专门的会话命令，不走通用的口令命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.saveConnection(baiduForm());

    const calls = invokeMock.mock.calls.map(call => call[0]);

    expect(calls).toContain('remote_baidu_save_session');
    expect(calls).not.toContain('remote_save_credential');
  });

  it('会话命令拿到的是连接 id 与整段 Cookie', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(baiduForm());

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_baidu_save_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      connectionId: saved.id,
      cookie: 'BDUSS=abc123; STOKEN=def456',
    });
  });

  it('主机与端口被写成百度网盘的固定值', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(baiduForm());

    expect(saved.host).toBe('pan.baidu.com');
    expect(saved.port).toBe(443);
  });

  it('凭据键等于连接 id，界面上才能显示「已保存」', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(baiduForm());

    expect(saved.credentialKey).toBe(saved.id);
  });

  it('会话命令失败时，连接不会被写进列表', async () => {
    const store = useRemoteConnectionsStore();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_baidu_save_session') {
        throw new Error('格式不对');
      }

      return undefined;
    });

    await expect(store.saveConnection(baiduForm())).rejects.toThrow();

    expect(store.connections).toHaveLength(0);
    expect(storageContents.get('connections')).toBeUndefined();
  });

  it('授权状态是从后端拿的，不在前端自己猜', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_baidu_auth_status') {
        return {
          status: 'ready',
          summary: '已保存 BDUSS 与 STOKEN',
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();
    const status = await store.getBaiduAuthStatus('conn-baidu');

    expect(status.status).toBe('ready');
    expect(invokeMock.mock.calls.some(call => call[0] === 'remote_baidu_auth_status')).toBe(true);
  });

  it('测试连接时带上凭据键，cookie 框留空也能用存档里的会话去测', async () => {
    const store = useRemoteConnectionsStore();

    await store.testConnection({
      ...baiduForm({ password: '' }),
      id: 'conn-baidu',
    });

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_test_connection');

    expect(call?.[1]).toMatchObject({
      request: {
        protocol: 'baidu_pan',
        credentialKey: 'conn-baidu',
      },
    });
  });
});
