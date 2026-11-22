// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「新建连接时预填 provider 显示名」与「加入全局检索」勾选框的**接线**（批次 37）。
//
// 钉住的是四条不可省的行为：
//
// 1. 新建网盘时名称栏预填 provider 的显示名（走 i18n 键，不是硬编码中文）。
// 2. 编辑既有连接时**绝不覆盖**存下来的名字。
// 3. FTP / SFTP 新建时名称栏留空 —— 它们不是网盘。
// 4. 「加入全局检索」默认不勾选，编辑时按存的值回填，勾了能真的存下去。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { defineComponent } from 'vue';
import { createI18n } from 'vue-i18n';
import { flushPromises, mount } from '@vue/test-utils';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import ConnectionForm from '@/modules/settings/ui/categories/remote/connection-form.vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';
import type {
  NetdiskProtocol,
  RemoteConnection,
  RemoteProtocol,
} from '@/types/remote';

const {
  invokeMock,
  lazyStoreSaveMock,
  storageContents,
} = vi.hoisted(() => ({
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

// 内嵌登录窗的进度事件。测试环境里没有 Tauri 运行时，桩掉即可。
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(async () => () => {}),
}));

/**
 * Checkbox 的替身。
 *
 * reka-ui 的 `CheckboxRoot` 在 jsdom 里不渲染可勾选的原生框，
 * 而 `Checkbox: true` 这种空壳又发不出 `update:modelValue` —— v-model 会断。
 * 这里自己画一个真 checkbox，`checked` 与 `modelValue` 同步。
 */
const CheckboxStub = defineComponent({
  props: {
    modelValue: {
      type: Boolean,
      default: false,
    },
  },
  emits: ['update:modelValue'],
  methods: {
    onChange(event: Event): void {
      const target = event.target as HTMLInputElement;
      this.$emit('update:modelValue', target.checked);
    },
  },
  template: '<input type="checkbox" :checked="modelValue" @change="onChange">',
});

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

type FormProps = {
  connection: RemoteConnection | null;
  initialProtocol?: RemoteProtocol;
  variant?: 'auto' | 'netdisk' | 'remote';
  netdiskProtocol?: NetdiskProtocol;
};

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
        Checkbox: CheckboxStub,
      },
    },
  });
}

function readSource(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), relativePath), 'utf8');
}

function netdiskConnection(overrides: Partial<RemoteConnection> = {}): RemoteConnection {
  return {
    id: 'c1',
    name: '我的网盘',
    protocol: 'aliyun_pan',
    host: 'openapi.alipan.com',
    port: 443,
    username: '',
    initialPath: '/',
    proxy: {
      kind: 'direct',
    },
    ...overrides,
  };
}

describe('远程连接表单：名称预填与「加入全局检索」', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_aliyun_auth_status') {
        return {
          status: 'missing',
        };
      }

      return undefined;
    });
  });

  describe('新建时的名称预填', () => {
    it('新建阿里云盘时预填阿里云盘的显示名', async () => {
      const wrapper = mountForm({
        connection: null,
        variant: 'netdisk',
        netdiskProtocol: 'aliyun_pan',
      });
      await flushPromises();

      const nameInput = wrapper.find('[data-test="field-name"]');
      expect(nameInput.exists()).toBe(true);
      expect((nameInput.element as HTMLInputElement).value).toBe('settings.remote.netdisk.aliyun');
    });

    it.each([
      ['baidu_pan', 'settings.remote.netdisk.baidu'],
      ['quark_pan', 'settings.remote.netdisk.quark'],
    ] as [NetdiskProtocol, string][])('新建 %s 时预填 %s', async (netdiskProtocol, expectedKey) => {
      const wrapper = mountForm({
        connection: null,
        variant: 'netdisk',
        netdiskProtocol,
      });
      await flushPromises();

      const nameInput = wrapper.find('[data-test="field-name"]');
      expect((nameInput.element as HTMLInputElement).value).toBe(expectedKey);
    });

    it('新建 FTP 时名称栏留空', async () => {
      const wrapper = mountForm({
        connection: null,
        variant: 'remote',
        initialProtocol: 'ftp',
      });
      await flushPromises();

      const nameInput = wrapper.find('[data-test="field-name"]');
      expect((nameInput.element as HTMLInputElement).value).toBe('');
    });

    it('编辑既有网盘连接时保留存下来的名字', async () => {
      const wrapper = mountForm({
        connection: netdiskConnection(),
      });
      await flushPromises();

      const nameInput = wrapper.find('[data-test="field-name"]');
      expect((nameInput.element as HTMLInputElement).value).toBe('我的网盘');
    });

    it('两张新建表单都走同一个预填判定', () => {
      // 百度 / 夸克走的是「添加网盘」弹窗（`netdisk-add-dialog.vue`），
      // 阿里云盘走的是内联表单（`connection-form.vue`）——
      // 两条路径都必须预填，所以这里直接读源码钉住。
      const formSource = readSource('src/modules/settings/ui/categories/remote/connection-form.vue');
      const dialogSource = readSource('src/modules/settings/ui/categories/remote/netdisk-add-dialog.vue');

      expect(formSource).toContain('resolveRemoteConnectionNameDefault');
      expect(dialogSource).toContain('resolveRemoteConnectionNameDefault');
    });
  });

  describe('「加入全局检索」勾选框', () => {
    it('新建时这个勾选框存在，且默认不勾', async () => {
      const wrapper = mountForm({
        connection: null,
        variant: 'netdisk',
        netdiskProtocol: 'aliyun_pan',
      });
      await flushPromises();

      const checkbox = wrapper.find('[data-test="field-include-in-search"]');
      expect(checkbox.exists()).toBe(true);
      expect((checkbox.element as HTMLInputElement).checked).toBe(false);
    });

    it('编辑既有连接时按存下来的值回填', async () => {
      const wrapper = mountForm({
        connection: netdiskConnection({
          includeInGlobalSearch: true,
        }),
      });
      await flushPromises();

      const checkbox = wrapper.find('[data-test="field-include-in-search"]');
      expect((checkbox.element as HTMLInputElement).checked).toBe(true);
    });

    it('老记录没勾过时读回来是未勾选', async () => {
      const wrapper = mountForm({
        connection: netdiskConnection(),
      });
      await flushPromises();

      const checkbox = wrapper.find('[data-test="field-include-in-search"]');
      expect((checkbox.element as HTMLInputElement).checked).toBe(false);
    });

    it('勾选后保存，落盘的连接带 includeInGlobalSearch: true', async () => {
      const store = useRemoteConnectionsStore();
      await store.init();

      const wrapper = mountForm({
        connection: null,
        variant: 'remote',
        initialProtocol: 'ftp',
      });
      await flushPromises();

      await wrapper.find('[data-test="field-name"]').setValue('我的 FTP');
      await wrapper.find('[data-test="field-host"]').setValue('ftp.example.com');
      await wrapper.find('[data-test="field-include-in-search"]').setValue(true);
      await wrapper.find('form').trigger('submit');
      await flushPromises();

      const stored = (storageContents.get('connections') as RemoteConnection[]) ?? [];
      expect(stored).toHaveLength(1);
      expect(stored[0].includeInGlobalSearch).toBe(true);
    });

    it('不勾选时保存，落盘的连接带 includeInGlobalSearch: false', async () => {
      const store = useRemoteConnectionsStore();
      await store.init();

      const wrapper = mountForm({
        connection: null,
        variant: 'remote',
        initialProtocol: 'ftp',
      });
      await flushPromises();

      await wrapper.find('[data-test="field-name"]').setValue('我的 FTP');
      await wrapper.find('[data-test="field-host"]').setValue('ftp.example.com');
      await wrapper.find('form').trigger('submit');
      await flushPromises();

      const stored = (storageContents.get('connections') as RemoteConnection[]) ?? [];
      expect(stored).toHaveLength(1);
      expect(stored[0].includeInGlobalSearch).toBe(false);
    });

    it('保存后表单回到「新建」，勾选框归零', async () => {
      const store = useRemoteConnectionsStore();
      await store.init();

      const wrapper = mountForm({
        connection: null,
        variant: 'remote',
        initialProtocol: 'ftp',
      });
      await flushPromises();

      await wrapper.find('[data-test="field-name"]').setValue('我的 FTP');
      await wrapper.find('[data-test="field-host"]').setValue('ftp.example.com');
      await wrapper.find('[data-test="field-include-in-search"]').setValue(true);
      await wrapper.find('form').trigger('submit');
      await flushPromises();

      const checkbox = wrapper.find('[data-test="field-include-in-search"]');
      expect((checkbox.element as HTMLInputElement).checked).toBe(false);
      const nameInput = wrapper.find('[data-test="field-name"]');
      expect((nameInput.element as HTMLInputElement).value).toBe('');
    });
  });
});
