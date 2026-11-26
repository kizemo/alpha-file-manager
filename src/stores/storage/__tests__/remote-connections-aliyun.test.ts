// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘连接的**保存路径**护栏。
//
// 与夸克那份同构，但阿里云盘这条路径有本质区别：
//
//   1. **凭据不是口令** —— 阿里云盘的凭据是一整段会话（access_token），
//      走通用的 `remote_save_credential` 等于把空口令存成「已登录」。
//   2. **保存时走的是「认领」** —— 扫码或「粘贴 access_token」都会话先暂存在
//      后端的待认领槽位里，点保存时（那时才有连接 id）才认领并写进 DPAPI 密文。
//   3. **认领不到就必须报「请先登录」** —— 不能把一条
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

function aliyunForm(overrides: Partial<RemoteConnectionForm> = {}): RemoteConnectionForm {
  return {
    name: '阿里云盘',
    protocol: 'aliyun_pan',
    // 主机与端口**故意留空**：阿里云盘的表单不该要求用户填它们。
    host: '',
    port: 0,
    username: '',
    initialPath: '/',
    localPath: '',
    proxy: {
      kind: 'direct',
    },
    // 阿里云盘没有手工凭据入口，正常走扫码登录 ⇒ 这里默认是空的。
    password: '',
    clearPassword: false,
    ...overrides,
  };
}

describe('阿里云盘连接的保存', () => {
  beforeEach(async () => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      // 默认「扫码已经换到会话」—— 大多数用例验的是**保存路径怎么写**。
      if (command === 'remote_aliyun_login_claim') {
        return {
          claimed: true,
          summary: '阿里云盘会话已就绪',
        };
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

  it('凭据走扫码会话的认领，不走通用的口令命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.saveConnection(aliyunForm());

    const calls = invokeMock.mock.calls.map(call => call[0]);

    expect(calls).toContain('remote_aliyun_login_claim');
    expect(calls).not.toContain('remote_save_credential');
  });

  it('认领命令拿到的是连接 id', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(aliyunForm());

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_login_claim');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({ connectionId: saved.id });
  });

  it('主机与端口被写成阿里云盘的固定值', async () => {
    const store = useRemoteConnectionsStore();

    const saved = await store.saveConnection(aliyunForm());

    expect(saved.host).toBe('api.aliyundrive.com');
    expect(saved.port).toBe(443);
  });

  it('认领成功时凭据键等于连接 id，界面上才能显示「已保存」', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_aliyun_login_claim') {
        return {
          claimed: true,
          summary: '阿里云盘会话已就绪',
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();
    const saved = await store.saveConnection(aliyunForm());

    expect(saved.credentialKey).toBe(saved.id);
  });

  it('认领不到会话时报「请先登录」，而不是后端的原始错误', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_aliyun_login_claim') {
        // 后端序列化过来的错误**不是** Error 实例。
        // 直接往外抛的话，界面上 `error instanceof Error` 为 false ⇒ 落进
        // 'unknown'，用户只会看到「保存连接失败。」。
        throw { message: 'aliyun login window is not wired yet' };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();

    await expect(store.saveConnection(aliyunForm())).rejects.toThrow(/^aliyunLoginRequired$/);
    expect(store.connections).toHaveLength(0);
    expect(storageContents.get('connections')).toBeUndefined();
  });

  it('后端说槽位是空的（claimed=false）时同样报「请先登录」', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_aliyun_login_claim') {
        return {
          claimed: false,
          summary: '没有待认领的会话',
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();

    await expect(store.saveConnection(aliyunForm())).rejects.toThrow(/^aliyunLoginRequired$/);
    expect(store.connections).toHaveLength(0);
  });

  it('勾了「移除已保存的密码」时走删会话，而不是认领', async () => {
    const store = useRemoteConnectionsStore();

    await store.saveConnection(aliyunForm({ clearPassword: true }));

    const calls = invokeMock.mock.calls.map(call => call[0]);

    expect(calls).toContain('remote_aliyun_delete_session');
    expect(calls).not.toContain('remote_aliyun_login_claim');
    expect(calls).not.toContain('remote_save_credential');
  });

  it('删除会话命令拿到的是连接 id', async () => {
    const store = useRemoteConnectionsStore();

    await store.saveConnection(aliyunForm({ clearPassword: true }));

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_delete_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({ connectionId: expect.any(String) });
  });

  it('授权状态是从后端拿的，不在前端自己猜', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_aliyun_auth_status') {
        return {
          status: 'ready',
          summary: '阿里云盘会话已就绪',
          nickName: '测试用户',
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();
    const status = await store.getAliyunAuthStatus('conn-aliyun');

    expect(status.status).toBe('ready');
    expect(status.nickName).toBe('测试用户');

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_auth_status');
    expect(call?.[1]).toMatchObject({ connectionId: 'conn-aliyun' });
  });

  it('删会话走专门的命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.deleteAliyunSession('conn-aliyun');

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_delete_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({ connectionId: 'conn-aliyun' });
  });

  it('取二维码走后端专门的命令', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_aliyun_qr_start') {
        return {
          codeContent: 'the-code-content',
          ck: 'the-ck',
          t: 1700000000000,
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();
    const ticket = await store.startAliyunQr();

    expect(ticket.codeContent).toBe('the-code-content');
    expect(ticket.ck).toBe('the-ck');
    expect(ticket.t).toBe(1700000000000);
  });

  it('轮询扫码走后端专门的命令，并带上票据', async () => {
    const store = useRemoteConnectionsStore();

    await store.pollAliyunQr('the-ck', 1700000000000);

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_qr_poll');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      ck: 'the-ck',
      t: 1700000000000,
    });
  });

  it('粘贴 access_token 走后端专门的存会话命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.saveAliyunSession('conn-aliyun', 'the-token');

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_save_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      connectionId: 'conn-aliyun',
      accessToken: 'the-token',
    });
  });

  it('放弃登录走专门的命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.cancelAliyunLogin();

    expect(invokeMock.mock.calls.some(call => call[0] === 'remote_aliyun_login_cancel')).toBe(true);
  });

  it('待认领状态走专门的命令', async () => {
    const store = useRemoteConnectionsStore();

    await store.getAliyunLoginPending();

    expect(invokeMock.mock.calls.some(call => call[0] === 'remote_aliyun_login_pending')).toBe(true);
  });

  it('测试连接时带上凭据键', async () => {
    const store = useRemoteConnectionsStore();

    await store.testConnection({
      ...aliyunForm(),
      id: 'conn-aliyun',
    });

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_test_connection');

    expect(call?.[1]).toMatchObject({
      request: {
        protocol: 'aliyun_pan',
        credentialKey: 'conn-aliyun',
        host: 'api.aliyundrive.com',
        port: 443,
      },
    });
  });

  it('没保存过的阿里云盘连接直接说清楚缺哪一步，而不是丢给后端', async () => {
    const store = useRemoteConnectionsStore();

    const report = await store.testConnection(aliyunForm());

    expect(report.outcome).toBe('missing_credential');
    expect(report.message).toContain('阿里云盘');
    expect(invokeMock.mock.calls.some(call => call[0] === 'remote_test_connection')).toBe(false);
  });
});
