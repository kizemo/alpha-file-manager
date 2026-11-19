// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach,
  describe,
  expect,
  it,
  vi,
} from 'vitest';
import { shallowMount } from '@vue/test-utils';
import AddressBar from '../address-bar.vue';
import { clearShellPathChainCache } from '@/utils/shell-path-chain';

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn<(command: string, args: unknown) => Promise<unknown>>(),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/localization', () => ({
  i18n: {
    global: {
      locale: { value: 'en' },
      t: (key: string) => key,
    },
  },
  messages: {},
}));

vi.mock('@/stores/runtime/shortcuts', () => ({
  useShortcutsStore: () => ({
    getShortcutLabel: () => '',
  }),
}));

vi.mock('@/stores/runtime/platform', () => ({
  usePlatformStore: () => ({
    currentPlatform: 'windows',
    isWindows: true,
  }),
}));

vi.mock('@/stores/storage/remote-connections', () => ({
  useRemoteConnectionsStore: () => ({
    connections: [],
  }),
}));

vi.mock('@/modules/home/composables/use-drives', () => ({
  useDrives: () => ({
    drives: { value: [] },
  }),
}));

vi.mock('@/composables/use-shell-devices', () => ({
  useShellDevices: () => ({
    shellDevices: { value: [] },
  }),
}));

vi.mock('@/composables/use-drop-target-registry', () => ({
  registerDropContainer: () => {},
  unregisterDropContainer: () => {},
}));

vi.mock('@/components/ui/toaster', () => ({
  toast: Object.assign(() => {}, {
    success: () => {},
    error: () => {},
    warning: () => {},
    info: () => {},
  }),
  ToastStatic: { template: '<div />' },
}));

vi.mock('../composables/use-open-copied-path', () => ({
  useOpenCopiedPath: () => ({
    openCopiedPath: () => {},
  }),
}));

// FORK-MODIFICATION 2026-11-19（批次 24）：地址栏消费后端祖先链的真机夹具。
const phoneRoot = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\\\\\?\\usb#vid_22d9&pid_2765&mi_00#6&337f5e7c&1&0000#{6ac27878-a6fa-4155-ba85-f98f491d4f33}';
const storageCrumb = `${phoneRoot}\\SID-{10001,,1001925963776}`;
const folderCrumb = `${storageCrumb}\\{66A9CB84-2262-AF6E-0000-000000000000}`;

const phoneChain = [
  {
    path: phoneRoot,
    name: '一加 Ace 3',
  },
  {
    path: storageCrumb,
    name: '内部共享存储空间',
  },
  {
    path: folderCrumb,
    name: '.7934039a',
  },
];

async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await new Promise<void>(resolve => setTimeout(resolve, 0));
}

/**
 * `DirEntryInteractive` 必须是**渲染插槽的**桩：
 * `shallowMount` 的默认桩不渲染默认插槽，`.address-bar__part` 会一个都找不到 —— 那是假绿。
 */
function mountAddressBar(currentPath: string) {
  return shallowMount(AddressBar, {
    props: { currentPath },
    global: {
      stubs: {
        DirEntryInteractive: {
          name: 'DirEntryInteractive',
          template: '<div class="dir-entry-stub"><slot /></div>',
        },
      },
    },
  });
}

function crumbLabels(wrapper: ReturnType<typeof mountAddressBar>): (string | undefined)[] {
  return wrapper.findAll('.address-bar__part').map(part => part.attributes('aria-label'));
}

describe('address-bar：shell 祖先链面包屑', () => {
  beforeEach(() => {
    invokeMock.mockReset();
    invokeMock.mockResolvedValue([]);
    // 链缓存是模块级的：不清掉，后面那条用例会被前一条的夹具同步命中，压根不发请求。
    clearShellPathChainCache();
  });

  it('链到位后逐级显示，点中间一级导航到该级自己的 path', async () => {
    invokeMock.mockImplementation((command: string) => (command === 'resolve_shell_path_chain'
      ? Promise.resolve(phoneChain)
      : Promise.resolve([])));

    const wrapper = mountAddressBar(folderCrumb);

    await settle();

    expect(crumbLabels(wrapper)).toEqual([
      'locations',
      '一加 Ace 3',
      '内部共享存储空间',
      '.7934039a',
    ]);

    const parts = wrapper.findAll('.address-bar__part');

    expect(parts).toHaveLength(4);
    // shell 路径不能被任何「规范化」动过：title 里就是那一级的原始链项。
    expect(parts[1]?.attributes('title')).toBe(phoneRoot);
    expect(parts[2]?.attributes('title')).toBe(storageCrumb);

    await parts[1]?.trigger('click');
    await parts[2]?.trigger('click');

    // 每一级跳的是**它自己**那一级的原始 path，前端没有做任何切分或补全。
    expect(wrapper.emitted('navigate')).toEqual([
      [phoneRoot],
      [storageCrumb],
    ]);

    // 最后一级就是当前页，点它不该再发一次导航。
    await parts[3]?.trigger('click');

    expect(wrapper.emitted('navigate')).toHaveLength(2);
  });

  it('祖先链请求还没回来时先渲染两段式，绝不比改动前更差', async () => {
    invokeMock.mockImplementation((command: string) => (command === 'resolve_shell_path_chain'
      // 永不 settle：这就是「第一次双击手机、后端还在解析」的那一刻。
      ? new Promise<unknown>(() => {})
      : Promise.resolve([])));

    const wrapper = mountAddressBar(folderCrumb);

    await settle();

    expect(invokeMock).toHaveBeenCalledWith('resolve_shell_path_chain', { path: folderCrumb });
    expect(crumbLabels(wrapper)).toEqual([
      'locations',
      'fileBrowser.device',
    ]);
  });
});
