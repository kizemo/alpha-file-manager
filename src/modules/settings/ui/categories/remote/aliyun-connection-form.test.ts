// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘的连接表单：与百度 / 夸克**不同构** —— 走的是开放平台的 OAuth 授权，
// 授权页要带应用 ID / 密钥 / 回调地址，拿不到就开不了授权窗。
//
// 守五件事：
// ① 阿里云盘表单**不摆**百度的内嵌登录块，也不摆夸克的那一块 ——
//    provider 判定错一次，界面上就会出现「加了阿里云盘却显示百度表单」；
// ② 登录期参数（clientId / clientSecret / redirectUri）只在表单里，
//    不进连接表；
// ③ 没填应用 ID 时「登录」按钮是禁用的 —— 没有它授权页开不起来；
// ④ `afm://aliyun-login` 的 ready 事件必须给出「可以保存了」的反馈，
//    且同一状态只播一次；
// ⑤ **频道互不污染** —— 百度 / 夸克频道的事件不许触发阿里的反馈
//    （表单同时订阅三个频道，最容易在这里串味）。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import ConnectionForm from '@/modules/settings/ui/categories/remote/connection-form.vue';
import type { RemoteConnection } from '@/types/remote';

const { invokeMock, listenMock, toastMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  toastMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
  emit: vi.fn(),
  once: vi.fn(),
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: (...args: unknown[]) => toastMock(...args),
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataDir: '/tmp/user-data',
      appUserDataRemoteConnectionsPath: '/tmp/user-data/user-remote-connections.json',
    },
  }),
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

function mountForm(props: Record<string, unknown> = {}) {
  return mount(ConnectionForm, {
    props: {
      connection: null,
      ...props,
    },
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
}

const aliyunConnection: RemoteConnection = {
  id: 'conn-aliyun',
  name: '阿里云盘',
  protocol: 'aliyun_pan',
  host: 'openapi.alipan.com',
  port: 443,
  username: '',
  initialPath: '/',
  proxy: {
    kind: 'direct',
  },
  credentialKey: 'conn-aliyun',
};

/** 按事件名分别记回调 —— 表单同时订阅百度 / 夸克 / 阿里三个频道。 */
const loginHandlers = new Map<string, (payload: unknown) => void>();

function stubLoginEventSource() {
  loginHandlers.clear();
  listenMock.mockImplementation(async (event: string, handler: (payload: unknown) => void) => {
    loginHandlers.set(event, handler);

    return () => undefined;
  });
}

function pushLoginStatus(event: string, status: string, summary?: string) {
  loginHandlers.get(event)?.({
    payload: {
      status,
      summary,
    },
  });
}

describe('阿里云盘的连接表单', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    toastMock.mockReset();
    stubLoginEventSource();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_aliyun_auth_status') {
        return {
          status: 'ready',
          summary: '已授权：测试用户',
          clientId: 'cid-999',
        };
      }

      return undefined;
    });
  });

  it('新建时按上游选的 provider 显示阿里的登录块', async () => {
    const wrapper = mountForm({
      variant: 'netdisk',
      netdiskProtocol: 'aliyun_pan',
    });
    await flushPromises();

    expect(wrapper.find('[data-test="aliyun-embedded"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="baidu-embedded"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="quark-embedded"]').exists()).toBe(false);
  });

  it('不摆主机、端口与用户名 —— 它们对阿里云盘没有意义', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    expect(wrapper.find('[data-test="field-host"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-port"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-username"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-password"]').exists()).toBe(false);
  });

  it('登录期参数三个输入框都在，且只存在于表单里', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    expect(wrapper.find('[data-test="aliyun-client-id"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="aliyun-client-secret"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="aliyun-redirect-uri"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="aliyun-embedded-hint"]').text())
      .toContain('settings.remote.aliyun.embedded.hint');
    expect(wrapper.find('[data-test="aliyun-mode-hint"]').text())
      .toContain('settings.remote.aliyun.field.modeHint');
  });

  it('没填应用 ID 时「登录」按钮是禁用的', async () => {
    // 用「新建中」的表单：已有连接会由后端把 clientId 带回来，按钮自然是可点的。
    const wrapper = mountForm({
      variant: 'netdisk',
      netdiskProtocol: 'aliyun_pan',
    });
    await flushPromises();

    expect(wrapper.find('[data-test="aliyun-embedded-start"]').attributes('disabled')).toBeDefined();
  });

  it('发起登录时把三个登录期参数交给后端', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-client-id"]').setValue('cid-123');
    await wrapper.find('[data-test="aliyun-client-secret"]').setValue('secret-abc');
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_login_start');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      clientId: 'cid-123',
      clientSecret: 'secret-abc',
      redirectUri: '',
    });
  });

  it('取到凭据时给出「可以保存了」的反馈，且同一状态只播一次', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-client-id"]').setValue('cid-123');
    await flushPromises();
    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    pushLoginStatus('afm://aliyun-login', 'ready', '已换到访问凭证');
    await flushPromises();

    expect(toastMock).toHaveBeenCalledTimes(1);
    expect(toastMock.mock.calls[0][0]).toMatchObject({
      title: 'settings.remote.aliyun.embedded.saved',
      description: '已换到访问凭证',
    });

    pushLoginStatus('afm://aliyun-login', 'ready');
    await flushPromises();
    expect(toastMock).toHaveBeenCalledTimes(1);
  });

  it('别的组件发起的登录事件不在这里弹提示', async () => {
    mountForm({ connection: aliyunConnection });
    await flushPromises();

    pushLoginStatus('afm://aliyun-login', 'ready', '已换到访问凭证');
    await flushPromises();

    expect(toastMock).not.toHaveBeenCalled();
  });

  it('百度与夸克频道的事件不会污染阿里的反馈', async () => {
    mountForm({ connection: aliyunConnection });
    await flushPromises();

    pushLoginStatus('afm://baidu-login', 'ready', '已取到 BDUSS');
    pushLoginStatus('afm://quark-login', 'ready', '已取到 __pus');
    await flushPromises();

    const toastedTitles = toastMock.mock.calls.map(call => call[0]?.title);
    expect(toastedTitles).not.toContain('settings.remote.aliyun.embedded.saved');
  });

  it('显示阿里授权状态卡，并提供删除会话的入口', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    expect(wrapper.find('[data-test="aliyun-status"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="aliyun-status-dot"]').attributes('data-status')).toBe('ready');
    expect(wrapper.find('[data-test="aliyun-status-client-id"]').text()).toBe('cid-999');
    expect(wrapper.find('[data-test="aliyun-delete-session"]').exists()).toBe(true);
  });

  it('删除会话走专门的命令', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-delete-session"]').trigger('click');
    await flushPromises();

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_delete_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({ connectionId: 'conn-aliyun' });
  });

  it('新建中的连接没有授权状态卡 —— 还没存档', async () => {
    const wrapper = mountForm({
      variant: 'netdisk',
      netdiskProtocol: 'aliyun_pan',
    });
    await flushPromises();

    expect(wrapper.find('[data-test="aliyun-status"]').exists()).toBe(false);
  });
});
