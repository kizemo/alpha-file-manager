// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 夸克网盘连接的**保存路径**护栏。
//
// 与 `remote-connections-baidu.test.ts` 同构，但夸克这条路径多一个岔口：
//
//   1. **凭据走的不是同一个 command** —— 夸克的凭据是一整段会话 Cookie
//      （核心是 `__pus`），必须交给后端解析，塞进通用的 `remote_save_credential`
//      等于把没解析过的垃圾也存成「已登录」。
//   2. **校验不能要求主机与端口** —— 夸克网盘没有自选服务器，
//      拿 FTP 那套规则去卡，用户根本填不出一个能保存的表单。
//   3. **没登录就不许保存** —— 内嵌登录把会话先暂存在后端的「待认领槽位」里，
//      保存时再去认领。认领不到就必须报「请先登录」，不能把一条
//      「看起来加成功了、实际连不上」的连接写进列表。
//
// 第 3 条最容易悄悄坏掉：后端抛回来的错误在界面上不是 `Error` 实例，
// `error instanceof Error` 为 false ⇒ 落进 'unknown'，用户只会看到
// 一句「保存连接失败。」，而不知道下一步该做什么。

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

function quarkForm(overrides: Partial<RemoteConnectionForm> = {}): RemoteConnectionForm {
  return {
    name: '夸克网盘',
    protocol: 'quark_pan',
    // 主机与端口**故意留空**：夸克的表单不该要求用户填它们。
    host: '',
    port: 0,
    username: '',
    initialPath: '/',
    localPath: '',
    proxy: {
      kind: 'direct',
    },
    // 夸克界面不摆手工输入框，正常走内嵌登录 ⇒ 这里默认是空的。
    password: '',
    clearPassword: false,
    ...overrides,
  };
}

describe('夸克网盘连接的保存', () => {
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

    await store.saveConnection(quarkForm({ password: '__pus=abc123; __puus=def456' }));

    const calls = invokeMock.mock.calls.map(call => call[0]);

    expect(calls).toContain('remote_quark_save_session');
    expect(calls).not.toContain('remote_save_credential');
  });

  it('会话命令拿到的是连接 id 与整段 Cookie', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(quarkForm({ password: '__pus=abc123; __puus=def456' }));

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_quark_save_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      connectionId: saved.id,
      cookie: '__pus=abc123; __puus=def456',
    });
  });

  it('主机与端口被写成夸克网盘的固定值', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(quarkForm({ password: '__pus=abc123' }));

    expect(saved.host).toBe('pan.quark.cn');
    expect(saved.port).toBe(443);
  });

  it('凭据键等于连接 id，界面上才能显示「已保存」', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(quarkForm({ password: '__pus=abc123' }));

    expect(saved.credentialKey).toBe(saved.id);
  });

  it('会话命令失败时，连接不会被写进列表', async () => {
    const store = useRemoteConnectionsStore();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_save_session') {
        throw new Error('格式不对');
      }

      return undefined;
    });

    await expect(store.saveConnection(quarkForm({ password: '__pus=abc123' }))).rejects.toThrow();

    expect(store.connections).toHaveLength(0);
    expect(storageContents.get('connections')).toBeUndefined();
  });

  it('没带 Cookie 时走内嵌登录的认领，不走会话命令', async () => {
    const store = useRemoteConnectionsStore();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_login_claim') {
        return {
          claimed: true,
          summary: '夸克会话已就绪',
        };
      }

      return undefined;
    });

    await store.saveConnection(quarkForm());

    const calls = invokeMock.mock.calls.map(call => call[0]);

    expect(calls).toContain('remote_quark_login_claim');
    expect(calls).not.toContain('remote_quark_save_session');
  });

  it('认领成功时凭据键等于连接 id', async () => {
    const store = useRemoteConnectionsStore();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_login_claim') {
        return {
          claimed: true,
          summary: '夸克会话已就绪',
        };
      }

      return undefined;
    });

    const saved = await store.saveConnection(quarkForm());

    expect(saved.credentialKey).toBe(saved.id);
  });

  it('认领不到会话时报「请先登录」，而不是后端的原始错误', async () => {
    const store = useRemoteConnectionsStore();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_login_claim') {
        // 后端序列化过来的错误**不是** Error 实例。
        // 直接往外抛的话，界面上 `error instanceof Error` 为 false ⇒ 落进
        // 'unknown'，用户只会看到「保存连接失败。」。
        throw { message: 'quark login window is not wired yet' };
      }

      return undefined;
    });

    await expect(store.saveConnection(quarkForm())).rejects.toThrow(/^quarkLoginRequired$/);

    expect(store.connections).toHaveLength(0);
    expect(storageContents.get('connections')).toBeUndefined();
  });

  it('后端说槽位是空的（claimed=false）时同样报「请先登录」', async () => {
    const store = useRemoteConnectionsStore();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_login_claim') {
        return {
          claimed: false,
          summary: '没有待认领的会话',
        };
      }

      return undefined;
    });

    await expect(store.saveConnection(quarkForm())).rejects.toThrow(/^quarkLoginRequired$/);
    expect(store.connections).toHaveLength(0);
  });

  it('授权状态是从后端拿的，不在前端自己猜', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_auth_status') {
        return {
          status: 'ready',
          summary: '夸克会话已就绪',
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();
    const status = await store.getQuarkAuthStatus('conn-quark');

    expect(status.status).toBe('ready');
    expect(invokeMock.mock.calls.some(call => call[0] === 'remote_quark_auth_status')).toBe(true);
  });

  it('删除会话走专门的命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.deleteQuarkSession('conn-quark');

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_quark_delete_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({ connectionId: 'conn-quark' });
  });

  it('测试连接时带上凭据键，cookie 框留空也能用存档里的会话去测', async () => {
    const store = useRemoteConnectionsStore();

    await store.testConnection({
      ...quarkForm(),
      id: 'conn-quark',
    });

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_test_connection');

    expect(call?.[1]).toMatchObject({
      request: {
        protocol: 'quark_pan',
        credentialKey: 'conn-quark',
      },
    });
  });

  it('没保存过的夸克连接直接说清楚缺哪一步，而不是丢给后端', async () => {
    const store = useRemoteConnectionsStore();

    const report = await store.testConnection(quarkForm());

    expect(report.outcome).toBe('missing_credential');
    expect(report.message).toContain('夸克');
    expect(invokeMock.mock.calls.some(call => call[0] === 'remote_test_connection')).toBe(false);
  });
});
