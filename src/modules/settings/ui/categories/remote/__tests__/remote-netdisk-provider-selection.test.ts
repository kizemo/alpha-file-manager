// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 「添加网盘」里 provider 按钮的**选中态**（用户 2026-11-24）。
//
// 钉住的是四件不可省的事：
//
// 1. 默认状态下所有 provider 按钮都**没有**选中标记；
// 2. 点过「百度网盘」之后，**只有**百度那颗带选中标记，夸克那颗不带；
// 3. 选中态跟着最后一次点击走（点夸克 ⇒ 亮点挪到夸克）；
// 4. 选中态**没有**改掉点击行为 —— 点百度照样立刻开内嵌登录窗，
//    不是改成「先选中、再点确认」的两步式。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import ConnectionsSection from '@/modules/settings/ui/categories/remote/connections-section.vue';

const { invokeMock, lazyStoreSaveMock, storageContents } = vi.hoisted(() => ({
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

// `@/modules/settings` 的 barrel 会一路拖到 router，而 router 在测试环境里
// `routes is not iterable` 直接崩。换成只含 `SettingsItem` 的替身
// （与 `remote-category.test.ts` 同一处理）。
vi.mock('@/modules/settings', () => ({
  SettingsItem: {
    props: ['title', 'description', 'icon'],
    template: `
      <section class="settings-view-item">
        <slot name="title-suffix" />
        <slot />
      </section>
    `,
  },
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

function mountSection() {
  return mount(ConnectionsSection, {
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

/**
 * provider 按钮的 `data-test` 名。
 *
 * 断言与实现各写一份的话，早晚有一颗会漏 —— 这里列一张表，
 * 「每颗按钮默认都没选中」那条断言直接遍历它。
 */
const PROVIDER_BUTTONS = [
  'add-netdisk-baidu_pan',
  'add-netdisk-quark_pan',
  'add-netdisk-aliyun_pan',
] as const;

describe('「添加网盘」的 provider 选中态', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      return undefined;
    });
  });

  it('默认状态下所有 provider 按钮都没有选中标记', async () => {
    const wrapper = mountSection();
    await flushPromises();

    for (const dataTest of PROVIDER_BUTTONS) {
      const button = wrapper.find(`[data-test="${dataTest}"]`);

      expect(button.exists()).toBe(true);
      expect(button.attributes('data-selected')).toBe('false');
      expect(button.attributes('aria-pressed')).toBe('false');
      expect(button.classes()).not.toContain('remote-connections__add-netdisk-option--selected');
      expect(button.classes()).toContain('sigma-ui-button--outline');
    }
  });

  it('点过「百度网盘」之后，只有百度那颗带选中标记', async () => {
    const wrapper = mountSection();
    await flushPromises();

    await wrapper.find('[data-test="add-netdisk-baidu_pan"]').trigger('click');
    await flushPromises();

    const baidu = wrapper.find('[data-test="add-netdisk-baidu_pan"]');
    expect(baidu.attributes('data-selected')).toBe('true');
    expect(baidu.attributes('aria-pressed')).toBe('true');
    expect(baidu.classes()).toContain('remote-connections__add-netdisk-option--selected');
    expect(baidu.classes()).toContain('sigma-ui-button--secondary');

    const quark = wrapper.find('[data-test="add-netdisk-quark_pan"]');
    expect(quark.attributes('data-selected')).toBe('false');
    expect(quark.attributes('aria-pressed')).toBe('false');
    expect(quark.classes()).not.toContain('remote-connections__add-netdisk-option--selected');
    expect(quark.classes()).toContain('sigma-ui-button--outline');
  });

  it('选中态跟着最后一次点击走', async () => {
    const wrapper = mountSection();
    await flushPromises();

    await wrapper.find('[data-test="add-netdisk-baidu_pan"]').trigger('click');
    await flushPromises();

    await wrapper.find('[data-test="add-netdisk-quark_pan"]').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-test="add-netdisk-quark_pan"]').attributes('data-selected')).toBe('true');
    expect(wrapper.find('[data-test="add-netdisk-baidu_pan"]').attributes('data-selected')).toBe('false');
  });

  it('选中态没有改掉点击行为：点百度照样开内嵌登录窗', async () => {
    const wrapper = mountSection();
    await flushPromises();

    invokeMock.mockClear();

    await wrapper.find('[data-test="add-netdisk-baidu_pan"]').trigger('click');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('remote_baidu_login_start');
  });

  it('OneDrive 开关关闭时，那颗按钮仍然不渲染', async () => {
    const wrapper = mountSection();
    await flushPromises();

    expect(wrapper.find('[data-test="add-netdisk-onedrive_pan"]').exists()).toBe(false);
  });
});
