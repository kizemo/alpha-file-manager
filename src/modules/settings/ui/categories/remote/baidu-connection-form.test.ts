// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 百度网盘连接表单的**形态**护栏。
//
// 百度网盘没有服务器地址、没有端口、没有用户名，凭据是一整段 Cookie。
// 表单若还照着 FTP 的样子把主机、端口、用户名摆出来，
// 用户会以为要填一个填了也没用的地址 —— 而保存时后端又会把它们忽略掉，
// 于是「我明明填了主机为什么连不上」变成一个查不到答案的问题。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import ConnectionForm from '@/modules/settings/ui/categories/remote/connection-form.vue';
import type { RemoteConnection } from '@/types/remote';

const { invokeMock, openUrlMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  openUrlMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

// 帮助块里那颗「打开登录页」按钮走的是系统浏览器，测试里只能断言它被叫到、叫什么。
vi.mock('@tauri-apps/plugin-opener', () => ({
  openUrl: (...args: unknown[]) => openUrlMock(...args),
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

describe('百度网盘的连接表单', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    openUrlMock.mockReset();
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

  it('凭据那一栏是 Cookie，不是密码', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    const field = wrapper.find('[data-test="field-password"]');

    expect(field.exists()).toBe(true);
    expect(field.attributes('data-baidu')).toBe('cookie');
    // Cookie 要能看见 —— 藏成密码点没法核对复制对没复制全。
    expect(field.attributes('type')).toBe('text');
  });

  it('给出「去哪儿复制 Cookie」的引导', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="hint-cookie"]').exists()).toBe(true);
  });

  // 一句话引导不够用：用户还得知道「BDUSS 少了会怎样」「能不能只粘一段」。
  // 2026-11-08 把详细步骤直接放进表单，省得他另开浏览器去搜教程。
  it('展开说明后给出步骤与注意事项，且带登录页外链', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    const help = wrapper.find('[data-test="baidu-cookie-help"]');

    expect(help.exists()).toBe(true);

    const toggle = wrapper.find('[data-test="baidu-cookie-help-toggle"]');

    expect(toggle.exists()).toBe(true);
    await toggle.trigger('click');
    await flushPromises();

    expect(toggle.attributes('aria-expanded')).toBe('true');
    const body = wrapper.find('.remote-connection-form__cookie-help-body');

    expect(body.exists()).toBe(true);
    // 五条步骤 + 三条注意，少一条说明就不成立（步骤是离散键，防止漏翻）。
    expect(body.findAll('li')).toHaveLength(8);
    expect(help.text()).toContain('settings.remote.baidu.help.openLogin');
  });

  it('点「打开百度网盘登录页」用系统浏览器打开 pan.baidu.com', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    await wrapper.find('[data-test="baidu-cookie-help-toggle"]').trigger('click');
    await flushPromises();

    const link = wrapper.find('[data-test="baidu-open-login"]');

    expect(link.exists()).toBe(true);
    await link.trigger('click');
    expect(openUrlMock).toHaveBeenCalledWith('https://pan.baidu.com/');
  });

  it('FTP/SFTP 之类普通协议不出现这段百度专属说明', async () => {
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

    expect(wrapper.find('[data-test="baidu-cookie-help"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="hint-cookie"]').exists()).toBe(false);
  });

  it('显示授权状态卡，并提供删除会话的入口', async () => {
    const wrapper = mountForm(baiduConnection);
    await flushPromises();

    expect(wrapper.find('[data-test="baidu-status"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="baidu-status-dot"]').attributes('data-status')).toBe('ready');
    expect(wrapper.find('[data-test="baidu-delete-session"]').exists()).toBe(true);
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
