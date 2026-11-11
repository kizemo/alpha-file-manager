// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 夸克网盘的连接表单：与百度同构，但走自己的事件频道与自己的文案。
//
// 守四件事：
// ① 网盘表单不摆主机/端口/用户名 —— 它们对夸克网盘没有意义；
// ② 内嵌登录是唯一的凭据入口；
// ③ `afm://quark-login` 的 ready 事件必须给出「可以保存了」的反馈，
//    且同一状态只播一次；
// ④ **两个频道互不污染** —— 百度频道的事件不许触发夸克的反馈
//    （表单同时订阅两个频道，最容易在这里串味）。

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

function mountForm(connection: RemoteConnection | null) {
  return mount(ConnectionForm, {
    props: {
      connection,
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

/** 按事件名分别记回调 —— 表单同时订阅百度与夸克两个频道。 */
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

describe('夸克网盘的连接表单', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    toastMock.mockReset();
    stubLoginEventSource();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_quark_auth_status') {
        return {
          status: 'ready',
          remainingSeconds: 3600,
          summary: '已授权：银城501',
        };
      }

      return undefined;
    });
  });

  it('不摆主机、端口与用户名 —— 它们对夸克网盘没有意义', async () => {
    const wrapper = mountForm(quarkConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="field-host"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-port"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-username"]').exists()).toBe(false);
  });

  it('内嵌登录是夸克网盘表单唯一的凭据入口', async () => {
    const wrapper = mountForm(quarkConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="quark-embedded"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="quark-embedded-start"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="quark-embedded-hint"]').text())
      .toContain('settings.remote.quark.embedded.hint');
    // 手工 Cookie 这条路已经废弃，夸克侧同样一个入口都不留。
    expect(wrapper.find('[data-test="field-cookie"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-password"]').exists()).toBe(false);
  });

  it('取到凭据时给出「可以保存了」的反馈，且同一状态只播一次', async () => {
    mountForm(quarkConnection);
    await flushPromises();

    pushLoginStatus('afm://quark-login', 'ready', '已取到 __pus');
    await flushPromises();

    expect(toastMock).toHaveBeenCalledTimes(1);
    expect(toastMock.mock.calls[0][0]).toMatchObject({
      title: 'settings.remote.quark.embedded.saved',
      description: '已取到 __pus',
    });

    pushLoginStatus('afm://quark-login', 'ready');
    await flushPromises();
    expect(toastMock).toHaveBeenCalledTimes(1);
  });

  it('百度频道的事件不会污染夸克的反馈', async () => {
    mountForm(quarkConnection);
    await flushPromises();

    pushLoginStatus('afm://baidu-login', 'ready', '已取到 BDUSS');
    await flushPromises();

    const toastedTitles = toastMock.mock.calls.map(call => call[0]?.title);
    expect(toastedTitles).not.toContain('settings.remote.quark.embedded.saved');
  });

  it('显示夸克授权状态卡，并提供删除会话的入口', async () => {
    const wrapper = mountForm(quarkConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="quark-status"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="quark-status-dot"]').attributes('data-status')).toBe('ready');
    expect(wrapper.find('[data-test="quark-delete-session"]').exists()).toBe(true);
  });
});
