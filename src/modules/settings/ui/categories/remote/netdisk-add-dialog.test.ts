// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「添加网盘」弹窗的接线测试（用户 2026-11-11）。
//
// 盯的是四件事：
// 1. 点「百度网盘 / 夸克网盘」**直接**开内嵌登录窗 —— 登录窗是 Rust 侧独立窗口，
//    前端只发起动作。
// 2. 登录成功（`ready`）才弹录入表单，且表单里必须有「已登入」提示与三个字段。
// 3. 保存真的走 store（落盘一条连接），登录态由后端认领，不是前端传的密码。
// 4. 取消要把后端「待认领槽位」里的登录态丢掉 —— 否则它会污染下一次添加。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import NetdiskAddDialog from '@/modules/settings/ui/categories/remote/netdisk-add-dialog.vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';

const { invokeMock, listeners, lazyStoreSaveMock, storageContents } = vi.hoisted(() => {
  const listeners = new Map<string, (event: { payload: unknown }) => void>();

  return {
    invokeMock: vi.fn(),
    listeners,
    lazyStoreSaveMock: vi.fn(),
    storageContents: new Map<string, unknown>(),
  };
});

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

// 登录进度是 Rust 侧推到 `afm://baidu-login` / `afm://quark-login` 频道的。
// 这里替身把监听器留下来，测试自己往里塞事件。
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);

    return () => {
      listeners.delete(event);
    };
  }),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: vi.fn(),
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataDir: '/tmp/user-data',
      appUserDataRemoteConnectionsPath: '/tmp/user-data/user-remote-connections.json',
    },
  }),
}));

// 对话框组件整个换成「就地渲染」的替身：真的 Dialog 会把内容 Teleport 到 body，
// 测试里用 `wrapper.find` 就抓不到了。
vi.mock('@/components/ui/dialog', () => ({
  Dialog: {
    props: ['open'],
    template: '<div v-if="open"><slot /></div>',
  },
  DialogContent: { template: '<div><slot /></div>' },
  DialogHeader: { template: '<div><slot /></div>' },
  DialogTitle: { template: '<div><slot /></div>' },
  DialogFooter: { template: '<div><slot /></div>' },
}));

type NetdiskProtocol = 'baidu_pan' | 'quark_pan';

function createI18nStubPlugin() {
  // eslint-disable-next-line @typescript-eslint/no-require-imports
  const { createI18n } = require('vue-i18n');
  return createI18n({
    legacy: false,
    locale: 'en',
    fallbackWarn: false,
    missingWarn: false,
    messages: {} as Record<string, unknown>,
  });
}

async function mountedDialog() {
  const wrapper = mount(NetdiskAddDialog, {
    global: {
      plugins: [createI18nStubPlugin()],
    },
  });

  await flushPromises();

  return wrapper;
}

/** 组件对外暴露的入口。 */
function startAdd(
  wrapper: Awaited<ReturnType<typeof mountedDialog>>,
  protocol: NetdiskProtocol,
): Promise<void> {
  const vm = wrapper.vm as unknown as {
    startAdd: (next: NetdiskProtocol) => Promise<void>;
  };

  return vm.startAdd(protocol);
}

/** 往登录频道塞一条事件，模拟 Rust 侧推来的进度。 */
function emitLogin(channel: string, payload: unknown): void {
  const handler = listeners.get(channel);

  if (!handler) {
    throw new Error(`没有订阅 ${channel}`);
  }

  handler({ payload });
}

describe('添加网盘弹窗', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    listeners.clear();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_baidu_login_claim') {
        return {
          claimed: true,
          summary: 'BDUSS(len=32)',
        };
      }

      if (command === 'remote_quark_login_claim') {
        return {
          claimed: true,
          summary: '__pus(len=16)',
        };
      }

      return undefined;
    });
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
  });

  it('开始添加时直接开内嵌登录窗，登录成功前不弹录入表单', async () => {
    const wrapper = await mountedDialog();

    await startAdd(wrapper, 'baidu_pan');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('remote_baidu_login_start');
    expect(wrapper.find('[data-test="signed-in"]').exists()).toBe(false);
  });

  it('登录成功后弹录入表单，带「已登入」提示与三个字段', async () => {
    const wrapper = await mountedDialog();

    await startAdd(wrapper, 'baidu_pan');
    emitLogin('afm://baidu-login', {
      status: 'ready',
      hasStoken: false,
      summary: 'BDUSS(len=32)',
      message: null,
    });
    await flushPromises();

    expect(wrapper.find('[data-test="signed-in"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="field-name"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="field-local-path"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="field-initial-path"]').exists()).toBe(true);
    // 弹窗里要有取消按钮（用户 2026-11-11 明确要求）。
    expect(wrapper.find('[data-test="cancel"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="save"]').exists()).toBe(true);
  });

  it('夸克登录成功走同一条路', async () => {
    const wrapper = await mountedDialog();

    await startAdd(wrapper, 'quark_pan');
    emitLogin('afm://quark-login', {
      status: 'ready',
      summary: '__pus(len=16)',
      message: null,
    });
    await flushPromises();

    expect(wrapper.find('[data-test="signed-in"]').exists()).toBe(true);
  });

  it('保存把网盘连接写进 store，登录态由后端认领', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();

    const wrapper = await mountedDialog();
    await startAdd(wrapper, 'baidu_pan');
    emitLogin('afm://baidu-login', {
      status: 'ready',
      hasStoken: false,
      summary: 'BDUSS(len=32)',
      message: null,
    });
    await flushPromises();

    await wrapper.find('[data-test="field-name"]').setValue('我的网盘');
    await wrapper.find('[data-test="field-local-path"]').setValue('C:/Users/test/Downloads');
    await wrapper.find('[data-test="save"]').trigger('click');
    await flushPromises();

    expect(store.connections).toHaveLength(1);
    expect(store.connections[0].name).toBe('我的网盘');
    expect(store.connections[0].protocol).toBe('baidu_pan');
    expect(store.connections[0].localPath).toBe('C:/Users/test/Downloads');
    // 登录态是后端认领的 —— 前端不传密码。
    expect(invokeMock).toHaveBeenCalledWith('remote_baidu_login_claim', expect.anything());
  });

  it('名称为空时不保存', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();

    const wrapper = await mountedDialog();
    await startAdd(wrapper, 'baidu_pan');
    emitLogin('afm://baidu-login', {
      status: 'ready',
      hasStoken: false,
      summary: 'BDUSS(len=32)',
      message: null,
    });
    await flushPromises();

    await wrapper.find('[data-test="save"]').trigger('click');
    await flushPromises();

    expect(store.connections).toHaveLength(0);
    expect(wrapper.find('[data-test="form-error"]').exists()).toBe(true);
  });

  it('取消会丢掉待认领的登录态，且不落盘', async () => {
    const store = useRemoteConnectionsStore();
    await store.init();

    const wrapper = await mountedDialog();
    await startAdd(wrapper, 'quark_pan');
    emitLogin('afm://quark-login', {
      status: 'ready',
      summary: '__pus(len=16)',
      message: null,
    });
    await flushPromises();

    await wrapper.find('[data-test="cancel"]').trigger('click');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('remote_quark_login_cancel');
    expect(store.connections).toHaveLength(0);
  });

  it('登录窗被关掉时不弹录入表单', async () => {
    const wrapper = await mountedDialog();

    await startAdd(wrapper, 'baidu_pan');
    emitLogin('afm://baidu-login', {
      status: 'closed',
      hasStoken: false,
      summary: null,
      message: null,
    });
    await flushPromises();

    expect(wrapper.find('[data-test="signed-in"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-name"]').exists()).toBe(false);
  });
});
