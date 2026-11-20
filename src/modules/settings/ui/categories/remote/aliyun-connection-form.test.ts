// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘的连接表单：走的是**扫码登录**（批次 29）。
//
// 开放平台那套 OAuth 授权窗实测不可用（`POST /token/get` 给的 41 字符 token
// 不能当 Bearer 用），所以表单里**没有**「应用 ID / 密钥 / 回调地址」三个输入框，
// 改成「扫二维码」+「粘贴 access_token」兜底。
//
// 守六件事：
// ① 阿里云盘表单**不摆**百度的内嵌登录块，也不摆夸克的那一块 ——
//    provider 判定错一次，界面上就会出现「加了阿里云盘却显示百度表单」；
// ② 表单里**没有**开放平台那三个输入框；
// ③ 点「显示二维码」走后端的取票命令，并把二维码渲染成图片；
// ④ 轮询到「已扫码」时给出「可以保存了」的反馈，且同一状态只播一次；
// ⑤ 粘贴 access_token 走后端专门的存会话命令；
// ⑥ **频道互不污染** —— 百度 / 夸克频道的事件不许触发阿里的反馈。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import ConnectionForm from '@/modules/settings/ui/categories/remote/connection-form.vue';
import type { RemoteConnection } from '@/types/remote';

const { invokeMock, listenMock, toastMock, qrToDataURLMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
  listenMock: vi.fn(),
  toastMock: vi.fn(),
  qrToDataURLMock: vi.fn(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => invokeMock(...args),
}));

vi.mock('@tauri-apps/api/event', () => ({
  listen: (...args: unknown[]) => listenMock(...args),
  emit: vi.fn(),
  once: vi.fn(),
}));

// jsdom 里没有 canvas ⇒ `qrcode` 自己画不出图，这里替掉。
vi.mock('qrcode', () => ({
  default: {
    toDataURL: (...args: unknown[]) => qrToDataURLMock(...args),
  },
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
  host: 'api.aliyundrive.com',
  port: 443,
  username: '',
  initialPath: '/',
  proxy: {
    kind: 'direct',
  },
  credentialKey: 'conn-aliyun',
};

/** 按事件名分别记回调 —— 表单同时订阅百度 / 夸克频道。 */
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
    qrToDataURLMock.mockReset();
    stubLoginEventSource();

    qrToDataURLMock.mockResolvedValue('data:image/png;base64,AAAA');

    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_aliyun_auth_status') {
        return {
          status: 'ready',
          summary: '已授权：测试用户',
          nickName: '测试用户',
        };
      }

      if (command === 'remote_aliyun_qr_start') {
        return {
          codeContent: 'the-code-content',
          ck: 'the-ck',
          t: 1700000000000,
        };
      }

      if (command === 'remote_aliyun_qr_poll') {
        return {
          status: 'confirmed',
          message: '扫码成功，可以保存这条连接了。',
          finalStatus: true,
        };
      }

      if (command === 'remote_aliyun_save_session') {
        return {
          status: 'ready',
          summary: '已授权：测试用户',
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

  it('扫码面板在，开放平台那三个输入框不在', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    // 扫码登录：二维码 + 粘贴 access_token 兜底。
    expect(wrapper.find('[data-test="aliyun-embedded-start"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="aliyun-paste-token"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="aliyun-paste-token-hint"]').text())
      .toContain('settings.remote.aliyun.field.pasteTokenHint');
    expect(wrapper.find('[data-test="aliyun-embedded-hint"]').text())
      .toContain('settings.remote.aliyun.embedded.hint');

    // 开放平台那三个输入框**必须不在** —— OAuth 实测不可用。
    expect(wrapper.find('[data-test="aliyun-client-id"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="aliyun-client-secret"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="aliyun-redirect-uri"]').exists()).toBe(false);
  });

  it('点「显示二维码」走后端取票，并把二维码渲染成图片', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_qr_start');

    expect(call).toBeDefined();
    expect(qrToDataURLMock).toHaveBeenCalledWith('the-code-content');
    expect(wrapper.find('[data-test="aliyun-qr-image"]').attributes('src'))
      .toBe('data:image/png;base64,AAAA');
  });

  it('轮询走后端的命令，并带上二维码票据', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_qr_poll');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      ck: 'the-ck',
      t: 1700000000000,
    });
  });

  it('扫码成功时给出「可以保存了」的反馈，且同一状态只播一次', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    expect(toastMock).toHaveBeenCalledTimes(1);
    expect(toastMock.mock.calls[0][0]).toMatchObject({
      title: 'settings.remote.aliyun.embedded.saved',
    });
    expect(wrapper.find('[data-test="aliyun-embedded-status"]').attributes('data-status')).toBe('ready');
  });

  it('二维码过期时状态显示为超时，且不再继续轮询', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_aliyun_qr_start') {
        return {
          codeContent: 'the-code-content',
          ck: 'the-ck',
          t: 1700000000000,
        };
      }

      if (command === 'remote_aliyun_qr_poll') {
        return {
          status: 'expired',
          message: '二维码已过期。',
          finalStatus: true,
        };
      }

      return undefined;
    });

    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-test="aliyun-embedded-status"]').attributes('data-status')).toBe('timeout');
    expect(invokeMock.mock.calls.filter(entry => entry[0] === 'remote_aliyun_qr_poll')).toHaveLength(1);
  });

  it('粘贴 access_token 走后端专门的存会话命令', async () => {
    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-paste-token"]').setValue('the-token');
    await flushPromises();

    await wrapper.find('[data-test="aliyun-save-token"]').trigger('click');
    await flushPromises();

    const call = invokeMock.mock.calls.find(entry => entry[0] === 'remote_aliyun_save_session');

    expect(call).toBeDefined();
    expect(call?.[1]).toMatchObject({
      connectionId: 'conn-aliyun',
      accessToken: 'the-token',
    });
  });

  it('取消扫码会走后端的取消命令', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'remote_aliyun_qr_start') {
        return {
          codeContent: 'the-code-content',
          ck: 'the-ck',
          t: 1700000000000,
        };
      }

      if (command === 'remote_aliyun_qr_poll') {
        return {
          status: 'new',
          finalStatus: false,
        };
      }

      return undefined;
    });

    const wrapper = mountForm({ connection: aliyunConnection });
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-start"]').trigger('click');
    await flushPromises();

    await wrapper.find('[data-test="aliyun-embedded-cancel"]').trigger('click');
    await flushPromises();

    expect(invokeMock.mock.calls.some(entry => entry[0] === 'remote_aliyun_login_cancel')).toBe(true);
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
    expect(wrapper.find('[data-test="aliyun-status-nick-name"]').text()).toBe('测试用户');
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
