// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 用户 2026-11-08 需求 2：位置页（主页）的「网盘」「远程连接」不再跳设置页，
// 改为**按连接列出名称**，点击在导航页视图里打开对应网盘/服务器。
//
// 这些用例守的是三件事：
// ① 有连接时列的是连接名，并且按网盘/远程分成两组、空组不渲染；
// ② 一条连接都没有时才有「去设置」入口，且那是空状态独有的；
// ③ 点击走的是 `openRemoteConnection`（与导航栏悬停面板同一条路径），
//    两个回调分别落到 router 与 workspaces —— 不是在本组件里自己 if 一次。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { flushPromises, mount } from '@vue/test-utils';
import RemoteEntriesSection from '@/modules/home/components/remote-entries-section.vue';
import type { RemoteConnection } from '@/types/remote';

const harness = vi.hoisted(() => ({
  openSettingsTab: vi.fn(),
  openRemoteConnection: vi.fn(),
  push: vi.fn(),
  openNewTabGroupWithPaths: vi.fn(),
  connections: [] as unknown[],
}));

vi.mock('@/modules/settings/utils/open-settings', () => ({
  openSettingsTab: (tab: string) => harness.openSettingsTab(tab),
}));

// 真正打开远端要跨进程、要凭据，这里只关心「点了之后调用了谁、参数是什么」。
vi.mock('@/modules/remote/utils/open-remote-connection', () => ({
  openRemoteConnection: (...args: unknown[]) => harness.openRemoteConnection(...args),
}));

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: harness.push }),
}));

vi.mock('@/stores/storage/workspaces', () => ({
  useWorkspacesStore: () => ({ openNewTabGroupWithPaths: harness.openNewTabGroupWithPaths }),
}));

// 连接列表直接由用例给定。用假 store 而不是真 store，
// 是为了不把 `@tauri-apps/plugin-store` 与用户数据目录拖进这条单测。
vi.mock('@/stores/storage/remote-connections', () => ({
  useRemoteConnectionsStore: () => ({ connections: harness.connections }),
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

function mountedSection() {
  return mount(RemoteEntriesSection, {
    global: {
      plugins: [createI18nStubPlugin()],
    },
  });
}

const netdiskConnection = {
  id: 'netdisk-1',
  name: '我的网盘',
  protocol: 'baidu_pan',
} as RemoteConnection;

const remoteConnection = {
  id: 'remote-1',
  name: '生产服务器',
  protocol: 'sftp',
} as RemoteConnection;

describe('位置页的网盘 / 远程连接列表', () => {
  beforeEach(() => {
    harness.openSettingsTab.mockReset();
    harness.openSettingsTab.mockResolvedValue(undefined);
    harness.openRemoteConnection.mockReset();
    harness.openRemoteConnection.mockResolvedValue(undefined);
    harness.push.mockReset();
    harness.push.mockResolvedValue(undefined);
    harness.openNewTabGroupWithPaths.mockReset();
    harness.openNewTabGroupWithPaths.mockResolvedValue(undefined);
    harness.connections = [];
  });

  it('一条连接都没有时，给提示与设置入口', async () => {
    const wrapper = mountedSection();
    await flushPromises();

    expect(wrapper.find('[data-test="empty"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="open-settings"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="group-netdisk"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="group-remote"]').exists()).toBe(false);

    await wrapper.find('[data-test="open-settings"]').trigger('click');
    await flushPromises();

    expect(harness.openSettingsTab).toHaveBeenCalledWith('remote');
  });

  it('有连接时按名称列出来，并分网盘 / 远程两组', async () => {
    harness.connections = [netdiskConnection, remoteConnection];

    const wrapper = mountedSection();
    await flushPromises();

    expect(wrapper.find('[data-test="empty"]').exists()).toBe(false);

    const netdiskEntry = wrapper.find('[data-test="netdisk-netdisk-1"]');
    const remoteEntry = wrapper.find('[data-test="remote-remote-1"]');

    expect(netdiskEntry.exists()).toBe(true);
    expect(netdiskEntry.text()).toContain('我的网盘');
    expect(remoteEntry.exists()).toBe(true);
    expect(remoteEntry.text()).toContain('生产服务器');
  });

  it('只有远程连接时不渲染空的网盘分组', async () => {
    harness.connections = [remoteConnection];

    const wrapper = mountedSection();
    await flushPromises();

    expect(wrapper.find('[data-test="group-netdisk"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="group-remote"]').exists()).toBe(true);
  });

  it('点连接走 openRemoteConnection，两个回调分别落到 router 与 workspaces', async () => {
    harness.connections = [netdiskConnection];

    const wrapper = mountedSection();
    await flushPromises();

    await wrapper.find('[data-test="netdisk-netdisk-1"]').trigger('click');
    await flushPromises();

    expect(harness.openRemoteConnection).toHaveBeenCalledTimes(1);

    const [connection, actions] = harness.openRemoteConnection.mock.calls[0] as [
      RemoteConnection,
      {
        navigateToNavigator: () => Promise<unknown>;
        openPanes: (paths: string[]) => Promise<void>;
      },
    ];

    expect(connection).toBe(netdiskConnection);

    await actions.navigateToNavigator();
    expect(harness.push).toHaveBeenCalledWith({ name: 'navigator' });

    await actions.openPanes(['sfm://remote/netdisk-1/']);
    expect(harness.openNewTabGroupWithPaths).toHaveBeenCalledWith(['sfm://remote/netdisk-1/']);
  });
});
