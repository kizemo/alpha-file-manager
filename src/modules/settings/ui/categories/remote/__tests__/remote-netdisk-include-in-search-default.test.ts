// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「添加网盘」时的「加入全局检索」选项（用户需求第 1 条）。
//
// 用户要的是：**添加云盘时能选要不要加检索，默认不加**。
// 功能本身在批次 37 就有了，这里只补回归测试，把「默认不添加」这个口径钉死 ——
// 以后谁把初值改成 `true`（或者干脆把复选框藏起来），这批用例立刻红。
//
// 钉住的是六件事：
//
// 1. 百度网盘：录入弹窗里的复选框**存在、可见、未勾选**；
// 2. 夸克网盘：同上；
// 3. 阿里云盘：内联表单（`connection-form.vue`）里的复选框**存在、可见、未勾选**；
// 4. 默认值要落到**数据层**：不勾选直接保存，落盘的连接 `includeInGlobalSearch` 是 `false`；
// 5. 反向：用户手动勾了，保存出来是 `true`，**不会被默认值逻辑覆盖回 `false`**；
// 6. store 层：新建连接对象没带这个字段时，存出来的就是 `false`。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import NetdiskAddDialog from '@/modules/settings/ui/categories/remote/netdisk-add-dialog.vue';
import ConnectionForm from '@/modules/settings/ui/categories/remote/connection-form.vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import type {
  NetdiskProtocol,
  RemoteConnection,
  RemoteConnectionForm,
} from '@/types/remote';

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

// 内嵌登录窗的进度事件。测试环境里没有 Tauri 运行时，桩掉即可；
// 这里把监听器留下来，测试自己往里塞「登录成功」。
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

/**
 * 把 i18n 打回键名，这样断言可以直接对键，不受翻译影响。
 */
function createI18nStubPlugin() {
  return createI18n({
    legacy: false,
    locale: 'en',
    fallbackWarn: false,
    missingWarn: false,
    messages: {},
  });
}

/**
 * 复选框当前是不是勾上了。
 *
 * 原生 `<input type="checkbox">` 看 `checked`；reka-ui 的复选框是个
 * `role="checkbox"` 的按钮，看 `aria-checked`。两种都判得动。
 */
function isChecked(element: Element): boolean {
  if (element instanceof HTMLInputElement) {
    return element.checked;
  }

  return element.getAttribute('aria-checked') === 'true';
}

/** 挂载「添加网盘」的录入弹窗。 */
function mountDialog() {
  return mount(NetdiskAddDialog, {
    global: {
      plugins: [createI18nStubPlugin()],
    },
  });
}

type FormProps = {
  connection: RemoteConnection | null;
  variant?: 'auto' | 'netdisk' | 'remote';
  netdiskProtocol?: NetdiskProtocol;
};

/** 挂载内联连接表单（阿里云盘 / OneDrive 走的就是这条路径）。 */
function mountForm(props: FormProps) {
  return mount(ConnectionForm, {
    props,
    global: {
      plugins: [createI18nStubPlugin()],
      stubs: {
        Select: true,
        SelectTrigger: true,
        SelectContent: true,
        SelectItem: true,
        SelectValue: true,
      },
    },
  });
}

/**
 * 走完「点 provider → 内嵌登录窗 → 登录成功 → 弹出录入弹窗」这条链路。
 *
 * 弹窗是**登录成功之后**才开的，所以这里把 `afm://xxx-login` 频道的
 * 「登录成功」事件塞进去，才拿得到录入表单。
 */
async function openNetdiskForm(protocol: NetdiskProtocol) {
  const wrapper = mountDialog();
  await flushPromises();

  const vm = wrapper.vm as unknown as {
    startAdd: (next: NetdiskProtocol) => Promise<void>;
  };

  await vm.startAdd(protocol);
  await flushPromises();

  const channel = protocol === 'baidu_pan' ? 'afm://baidu-login' : 'afm://quark-login';
  const handler = listeners.get(channel);

  if (!handler) {
    throw new Error(`没有订阅 ${channel}`);
  }

  handler({
    payload: {
      status: 'ready',
      summary: 'token(len=16)',
      message: null,
    },
  });
  await flushPromises();

  return wrapper;
}

/** 弹窗里那个复选框。 */
function includeInSearchCheckbox(wrapper: ReturnType<typeof mountDialog>) {
  return wrapper.find('[data-test="field-include-in-search"]');
}

describe('「加入全局检索」：默认不添加', () => {
  beforeEach(async () => {
    setActivePinia(createPinia());
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
    listeners.clear();
    invokeMock.mockReset();
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

      if (command === 'remote_aliyun_login_claim') {
        return {
          claimed: true,
          summary: 'access_token(len=16)',
        };
      }

      if (command === 'remote_aliyun_auth_status') {
        return {
          status: 'missing',
        };
      }

      return undefined;
    });

    const store = useRemoteConnectionsStore();
    await store.init();
  });

  describe('百度网盘 / 夸克网盘（录入弹窗）', () => {
    it('百度网盘：录入弹窗里的「加入全局检索」复选框存在、可见、默认未勾选', async () => {
      const wrapper = await openNetdiskForm('baidu_pan');

      // 弹窗确实开着（不是关着的壳）。
      expect(wrapper.find('[data-test="signed-in"]').exists()).toBe(true);

      const checkbox = includeInSearchCheckbox(wrapper);
      expect(checkbox.exists()).toBe(true);
      expect(checkbox.isVisible()).toBe(true);
      expect(isChecked(checkbox.element)).toBe(false);

      // 选项是给用户看的：文案在，而且标签指着这个复选框。
      const label = wrapper.findAll('label')
        .find(item => item.text().includes('settings.remote.field.includeInSearch'));
      expect(label).toBeDefined();
      expect(wrapper.find('label[for="netdisk-add-include-in-search"]').exists()).toBe(true);
    });

    it('夸克网盘：录入弹窗里的「加入全局检索」复选框存在、可见、默认未勾选', async () => {
      const wrapper = await openNetdiskForm('quark_pan');

      expect(wrapper.find('[data-test="signed-in"]').exists()).toBe(true);

      const checkbox = includeInSearchCheckbox(wrapper);
      expect(checkbox.exists()).toBe(true);
      expect(checkbox.isVisible()).toBe(true);
      expect(isChecked(checkbox.element)).toBe(false);

      const label = wrapper.findAll('label')
        .find(item => item.text().includes('settings.remote.field.includeInSearch'));
      expect(label).toBeDefined();
      expect(wrapper.find('label[for="netdisk-add-include-in-search"]').exists()).toBe(true);
    });

    it('百度网盘：不勾选直接保存，落盘的连接 includeInGlobalSearch 是 false（默认值落到数据层）', async () => {
      const store = useRemoteConnectionsStore();
      const wrapper = await openNetdiskForm('baidu_pan');

      await wrapper.find('[data-test="field-name"]').setValue('我的百度网盘');
      await wrapper.find('[data-test="save"]').trigger('click');
      await flushPromises();

      expect(store.connections).toHaveLength(1);
      expect(store.connections[0].includeInGlobalSearch).toBe(false);

      const stored = (storageContents.get('connections') as RemoteConnection[]) ?? [];
      expect(stored).toHaveLength(1);
      expect(stored[0].includeInGlobalSearch).toBe(false);
    });

    it('百度网盘：手动勾上以后保存，落盘的连接 includeInGlobalSearch 是 true', async () => {
      const store = useRemoteConnectionsStore();
      const wrapper = await openNetdiskForm('baidu_pan');

      await wrapper.find('[data-test="field-name"]').setValue('我的百度网盘');
      await includeInSearchCheckbox(wrapper).trigger('click');
      await flushPromises();

      expect(isChecked(includeInSearchCheckbox(wrapper).element)).toBe(true);

      await wrapper.find('[data-test="save"]').trigger('click');
      await flushPromises();

      expect(store.connections).toHaveLength(1);
      expect(store.connections[0].includeInGlobalSearch).toBe(true);
    });

    it('夸克网盘：手动勾上以后保存，落盘的连接 includeInGlobalSearch 是 true', async () => {
      const store = useRemoteConnectionsStore();
      const wrapper = await openNetdiskForm('quark_pan');

      await wrapper.find('[data-test="field-name"]').setValue('我的夸克网盘');
      await includeInSearchCheckbox(wrapper).trigger('click');
      await flushPromises();

      expect(isChecked(includeInSearchCheckbox(wrapper).element)).toBe(true);

      await wrapper.find('[data-test="save"]').trigger('click');
      await flushPromises();

      expect(store.connections).toHaveLength(1);
      expect(store.connections[0].includeInGlobalSearch).toBe(true);
    });
  });

  describe('阿里云盘（内联表单）', () => {
    it('阿里云盘：表单里的「加入全局检索」复选框存在、可见、默认未勾选', async () => {
      const wrapper = mountForm({
        connection: null,
        variant: 'netdisk',
        netdiskProtocol: 'aliyun_pan',
      });
      await flushPromises();

      const checkbox = wrapper.find('[data-test="field-include-in-search"]');
      expect(checkbox.exists()).toBe(true);
      expect(checkbox.isVisible()).toBe(true);
      expect(isChecked(checkbox.element)).toBe(false);

      const label = wrapper.findAll('label')
        .find(item => item.text().includes('settings.remote.field.includeInSearch'));
      expect(label).toBeDefined();
      expect(wrapper.find('label[for="remote-include-in-search"]').exists()).toBe(true);
    });

    it('阿里云盘：不勾选直接保存，落盘的连接 includeInGlobalSearch 是 false（默认值落到数据层）', async () => {
      const store = useRemoteConnectionsStore();
      const wrapper = mountForm({
        connection: null,
        variant: 'netdisk',
        netdiskProtocol: 'aliyun_pan',
      });
      await flushPromises();

      await wrapper.find('form').trigger('submit');
      await flushPromises();

      expect(store.connections).toHaveLength(1);
      expect(store.connections[0].protocol).toBe('aliyun_pan');
      expect(store.connections[0].includeInGlobalSearch).toBe(false);

      const stored = (storageContents.get('connections') as RemoteConnection[]) ?? [];
      expect(stored).toHaveLength(1);
      expect(stored[0].includeInGlobalSearch).toBe(false);
    });

    it('阿里云盘：手动勾上以后保存，落盘的连接 includeInGlobalSearch 是 true', async () => {
      const store = useRemoteConnectionsStore();
      const wrapper = mountForm({
        connection: null,
        variant: 'netdisk',
        netdiskProtocol: 'aliyun_pan',
      });
      await flushPromises();

      await wrapper.find('[data-test="field-include-in-search"]').trigger('click');
      await flushPromises();

      expect(isChecked(wrapper.find('[data-test="field-include-in-search"]').element)).toBe(true);

      await wrapper.find('form').trigger('submit');
      await flushPromises();

      expect(store.connections).toHaveLength(1);
      expect(store.connections[0].includeInGlobalSearch).toBe(true);
    });
  });

  describe('store 层的新建连接对象', () => {
    it('新建连接没带这个字段时，存出来的连接 includeInGlobalSearch 是 false', async () => {
      const store = useRemoteConnectionsStore();

      const form: RemoteConnectionForm = {
        name: '我的 FTP',
        protocol: 'ftp',
        host: 'ftp.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        localPath: '',
        proxy: {
          kind: 'direct',
        },
        password: '',
        clearPassword: false,
      };

      const saved = await store.saveConnection(form);

      expect(saved.includeInGlobalSearch).toBe(false);
      expect(store.connections).toHaveLength(1);
      expect(store.connections[0].includeInGlobalSearch).toBe(false);
    });
  });
});
