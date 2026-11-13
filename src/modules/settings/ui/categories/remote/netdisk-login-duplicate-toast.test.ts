// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// §11.3 缺陷 B 的**集成**复现（handoff §20）。
//
// 「添加网盘」弹窗（`NetdiskAddDialog`）在设置页**常驻挂载**，
// 「远程连接表单」（`ConnectionForm`）在新建/编辑连接时挂载 ——
// 两者会同时挂在同一棵树上，而 `afm://quark-login` / `afm://baidu-login`
// 是**全局**频道，两边都听得见同一条事件。
// 两边都无脑响应 ⇒ 同一条「登录等待超时了，请重试。」逐字弹两遍
// （2026-11-11 真机实拍）。
//
// 单独给每个组件写单测，只能证明「它自己不该乱响时确实不响」，
// **证不了「两个一起挂时不会重复」** —— 所以这里必须同时挂两个。
//
// 守的方向：
// ① 弹窗发起的那一轮登录 → 只有弹窗弹提示，表单一个字都不吭声；
// ② 表单发起的那一轮登录 → 只有表单弹提示，弹窗一个字都不吭声。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { h, ref } from 'vue';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import ConnectionForm from '@/modules/settings/ui/categories/remote/connection-form.vue';
import NetdiskAddDialog from '@/modules/settings/ui/categories/remote/netdisk-add-dialog.vue';
import type { RemoteConnection } from '@/types/remote';

const { invokeMock, listenMock, toastMock, lazyStoreSaveMock, storageContents } = vi.hoisted(() => {
  const storageContents = new Map<string, unknown>();

  return {
    invokeMock: vi.fn(),
    listenMock: vi.fn(),
    toastMock: vi.fn(),
    lazyStoreSaveMock: vi.fn(),
    storageContents,
  };
});

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// 关键：**一个频道可以有多个监听者**。
// 真实运行时 Tauri 的 `listen` 就是广播给所有订阅者 ——
// 而单测里常见的「一频道一 handler」写法根本表达不出「两个组件同时听见」。
const loginHandlers = new Map<string, ((event: { payload: unknown }) => void)[]>();

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
  emit: vi.fn(),
  once: vi.fn(),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: (...args: unknown[]) => toastMock(...args),
}));

vi.mock('@tauri-apps/plugin-dialog', () => ({
  open: vi.fn(async () => null),
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

// 真的 Dialog 会把内容 Teleport 到 body，测试里抓不到 —— 换成就地渲染的替身。
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

const quarkConnection: RemoteConnection = {
  id: 'conn-quark',
  name: '夸克网盘',
  protocol: 'quark_pan',
  host: 'pan.quark.cn',
  port: 443,
  username: '',
  initialPath: '/',
  proxy: {
    kind: 'direct',
  },
  credentialKey: 'conn-quark',
};

interface NetdiskDialogExpose {
  startAdd: (next: 'baidu_pan' | 'quark_pan') => Promise<void>;
}

/**
 * 同时挂「远程连接表单」与「添加网盘」弹窗 —— 复刻设置页里的真实拓扑。
 */
function mountBoth() {
  const dialogRef = ref<NetdiskDialogExpose | null>(null);

  const wrapper = mount({
    setup() {
      return () => h('div', [
        h(ConnectionForm, { connection: quarkConnection }),
        h(NetdiskAddDialog, { ref: dialogRef }),
      ]);
    },
  }, {
    global: {
      plugins: [createI18nStubPlugin()],
      stubs: {
        Select: true,
        SelectTrigger: true,
        SelectContent: true,
        SelectItem: true,
        SelectValue: true,
        Checkbox: true,
      },
    },
  });

  return {
    wrapper,
    dialogRef,
  };
}

/** 往登录频道广播一条事件 —— 真实运行时所有订阅者都会收到。 */
function broadcastLogin(channel: string, payload: unknown): void {
  const handlers = loginHandlers.get(channel) ?? [];

  for (const handler of handlers) {
    handler({ payload });
  }
}

describe('内嵌登录事件：常驻弹窗与连接表单同时挂着', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    toastMock.mockReset();
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
    loginHandlers.clear();

    listenMock.mockImplementation(async (event: string, handler: (e: { payload: unknown }) => void) => {
      const handlers = loginHandlers.get(event) ?? [];
      handlers.push(handler);
      loginHandlers.set(event, handlers);

      return () => undefined;
    });

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_quark_auth_status') {
        return {
          status: 'ready',
          remainingSeconds: 3600,
          summary: '已授权',
        };
      }

      return undefined;
    });
  });

  it('弹窗发起的那一轮超时，只弹一条提示 —— 不是两遍一样的字', async () => {
    const { dialogRef } = mountBoth();
    await flushPromises();

    // 复现路径②：正在编辑一条夸克连接，同时点「添加网盘 → 夸克网盘」。
    await dialogRef.value?.startAdd('quark_pan');
    await flushPromises();

    broadcastLogin('afm://quark-login', {
      status: 'timeout',
      summary: null,
      message: null,
    });
    await flushPromises();

    const toastedTitles = toastMock.mock.calls.map(call => call[0]?.title);
    expect(toastedTitles).toEqual(['settings.remote.quark.embedded.status.timeout']);
  });

  it('表单发起的那一轮超时，也只有一条提示', async () => {
    const { wrapper } = mountBoth();
    await flushPromises();

    // 这一轮是表单自己发起的（弹窗全程没参与）。
    await wrapper.find('[data-test="quark-embedded-start"]').trigger('click');
    await flushPromises();

    broadcastLogin('afm://quark-login', {
      status: 'timeout',
      summary: null,
      message: null,
    });
    await flushPromises();

    const toastedTitles = toastMock.mock.calls.map(call => call[0]?.title);
    expect(toastedTitles).toEqual(['settings.remote.quark.embedded.status.timeout']);
  });
});
