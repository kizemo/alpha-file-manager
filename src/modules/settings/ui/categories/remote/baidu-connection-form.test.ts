// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 百度网盘连接表单的**形态**护栏。
//
// 百度网盘没有服务器地址、没有端口、没有用户名，凭据是一整段 Cookie。
// 表单若还照着 FTP 的样子把主机、端口、用户名摆出来，
// 用户会以为要填一个填了也没用的地址 —— 而保存时后端又会把它们忽略掉，
// 于是「我明明填了主机为什么连不上」变成一个查不到答案的问题。
//
// 2026-11-09 起，Cookie 不再让用户手工抓取：内嵌登录窗口会自己取到凭据。
// 于是这张表单的护栏从「教用户怎么粘 Cookie」改成两条：
// ① 手工 Cookie 的输入框与讲解必须彻底消失（留着就是给用户一条会失败的退路）；
// ② 内嵌登录取到凭据后必须给出「可以保存」的反馈，否则用户不知道什么时候该点保存。

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

// 内嵌登录的进度靠事件推送。测试里把回调抓住，才能主动喂一个状态进去。
vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
  emit: vi.fn(),
  once: vi.fn(),
}));

// 「取到凭据可以保存了」的成功反馈走 toast，这里只断言它被叫到、叫什么。
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

const baiduConnection: RemoteConnection = {
  id: 'conn-baidu',
  name: '百度网盘',
  protocol: 'baidu_pan',
  host: 'pan.baidu.com',
  port: 443,
  username: '',
  initialPath: '/',
  proxy: {
    kind: 'direct',
  },
  credentialKey: 'conn-baidu',
};

/** 把组件注册的登录事件回调抓住，之后由测试主动喂状态。 */
let pushLoginStatus: ((status: string, summary?: string) => void) | null = null;

function stubLoginEventSource() {
  pushLoginStatus = null;
  listenMock.mockImplementation(async (event: string, handler: (payload: unknown) => void) => {
    // 这张表单现在订阅**两个**事件（百度 + 夸克）。只抓「最后注册的那个」
    // 会在夸克监听器后注册时抓错对象 —— 必须按事件名分别记。
    if (event !== 'afm://baidu-login') {
      return () => undefined;
    }

    pushLoginStatus = (status: string, summary?: string) => {
      handler({
        payload: {
          status,
          summary,
        },
      });
    };

    return () => undefined;
  });
}

describe('百度网盘的连接表单', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    toastMock.mockReset();
    stubLoginEventSource();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_baidu_auth_status') {
        return {
          status: 'ready',
          remainingSeconds: 1200,
          summary: '已保存 BDUSS 与 STOKEN',
        };
      }

      return undefined;
    });
  });

  it('不摆主机、端口与用户名 —— 它们对百度网盘没有意义', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="field-host"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-port"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-username"]').exists()).toBe(false);
  });

  // 手工抓 Cookie 这条路已经废弃：用户照着步骤粘进去的凭据经常拿不全，
  // 反而比内嵌登录更容易失败，所以表单里一个入口都不该留。
  it('不再出现手工 Cookie 的输入框与讲解', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="field-cookie"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="hint-cookie"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="baidu-cookie-help"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="baidu-cookie-help-toggle"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="baidu-open-login"]').exists()).toBe(false);
    // 凭据由内嵌登录取，网盘表单同样不该出现 FTP 那种口令输入框。
    expect(wrapper.find('[data-test="field-password"]').exists()).toBe(false);
  });

  it('内嵌登录是网盘表单唯一的凭据入口', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="baidu-embedded"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="baidu-embedded-start"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="baidu-embedded-hint"]').text())
      .toContain('settings.remote.baidu.embedded.hint');
  });

  it('网盘表单不摆协议下拉 —— 形态已经定了，没有第二个协议可选', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="field-protocol"]').exists()).toBe(false);
  });

  // 内嵌窗口取到凭据后会自动关闭，之后不会再有任何动静。
  // 如果这一刻不给反馈，用户只能猜「到底成没成」——这正是 2026-11-09 用户反馈的第 1 条。
  it('取到凭据时给出「可以保存了」的反馈，且同一状态只播一次', async () => {
    mountForm(baiduConnection);
    await flushPromises();

    pushLoginStatus?.('ready', '已取到 BDUSS 与 STOKEN');
    await flushPromises();

    expect(toastMock).toHaveBeenCalledTimes(1);
    expect(toastMock.mock.calls[0][0]).toMatchObject({
      title: 'settings.remote.baidu.embedded.saved',
      description: '已取到 BDUSS 与 STOKEN',
    });

    pushLoginStatus?.('ready');
    await flushPromises();
    expect(toastMock).toHaveBeenCalledTimes(1);
  });

  it('还没取到凭据时不放成功反馈', async () => {
    mountForm(baiduConnection);
    await flushPromises();

    pushLoginStatus?.('waiting');
    pushLoginStatus?.('closed');
    await flushPromises();

    // closed / timeout / error 各自有一条失败提示，这是 3.13.2 特意加的可见反馈。
    // 这条用例守的是「没取到凭据就别报成功」，所以只禁 saved 那一条，不是禁全部 toast。
    const toastedTitles = toastMock.mock.calls.map(call => call[0]?.title);
    expect(toastedTitles).not.toContain('settings.remote.baidu.embedded.saved');
  });

  it('显示授权状态卡，并提供删除会话的入口', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="baidu-status"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="baidu-status-dot"]').attributes('data-status')).toBe('ready');
    expect(wrapper.find('[data-test="baidu-delete-session"]').exists()).toBe(true);
  });

  it('普通协议不出现内嵌登录块，凭据照旧是密码框', async () => {
    const wrapper = mountForm({
      id: 'c2',
      name: 'NAS',
      protocol: 'ftp',
      host: 'ftp.example.com',
      port: 21,
      username: 'u',
      initialPath: '/',
      proxy: {
        kind: 'direct',
      },
      credentialKey: 'c2',
    });
    await flushPromises();

    expect(wrapper.find('[data-test="baidu-embedded"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="field-password"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="baidu-status"]').exists()).toBe(false);
  });

  it('普通协议连接照旧摆出主机与端口', async () => {
    const wrapper = mountForm({
      id: 'c1',
      name: '备份',
      protocol: 'sftp',
      host: 'sftp.example.com',
      port: 22,
      username: 'me',
      initialPath: '/',
      proxy: {
        kind: 'direct',
      },
    });
    await flushPromises();

    expect(wrapper.find('[data-test="field-host"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="field-port"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="field-password"]').attributes('data-baidu')).toBeUndefined();
    expect(wrapper.find('[data-test="baidu-status"]').exists()).toBe(false);
  });
});
