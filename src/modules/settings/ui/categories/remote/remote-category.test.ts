// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 这个测试盯的是**接线**，不是 store 逻辑（store 逻辑在
// `src/stores/storage/__tests__/remote-connections.test.ts`）。
//
// 三条不可省：
// 1. **分类真的被注册进设置页**：只建了组件、忘了在 `settingsTabs` 与
//    `categoryComponentMap` 里挂上，用户根本看不到这个页签，而两边都不会报错。
// 2. **保存走的是 store 而不是组件自建的对象**：组件里另存一份就会出现
//    「界面有、盘上无」。
// 3. **删除按钮点的是那条连接**：列表里两条同名连接时，点错一条是肉眼看不出来的。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { flushPromises, mount } from '@vue/test-utils';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import RemoteCategory from '@/modules/settings/ui/categories/remote/index.vue';
import { useRemoteConnectionsStore } from '@/stores/storage/remote-connections';

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

// 整个 barrel 换成替身：`@/modules/settings` 的 index 把 pages/settings.vue 与
// nav.vue 都导出进来，会一路拖到 router，而 router 在测试环境里
// `routes is not iterable` 直接崩。与 thumbnail-cache.test.ts 同一处理。
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
 *
 * 刻意**不**从 `@/stores/runtime/settings` 导入 `settingsTabs` 来断言注册 ——
 * 那个模块会把 router / workspaces 一整条依赖图拖进测试环境，
 * 在没有完整应用上下文时直接 `routes is not iterable` 崩掉。
 * 注册表是纯数据，读源码验更稳，且验的仍然是真实的那份文件。
 */
function readSource(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), relativePath), 'utf8');
}

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

function mountedCategory() {
  return mount(RemoteCategory, {
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
 * 切到页面顶部的某个分区标签（用户 2026-11-08）。
 *
 * ⚠️ 必须用 mousedown 而不是 click：reka-ui 的页签是在 `mousedown.left` 上
 * 切换的（真实浏览器里 mousedown 本来就先于 click 到达），
 * 只派发 click 不会切页签。
 */
async function switchSection(
  wrapper: ReturnType<typeof mountedCategory>,
  section: 'netdisk' | 'remote',
): Promise<void> {
  await wrapper.find(`[data-test="tab-${section}"]`).trigger('mousedown');
  await flushPromises();
}

describe('远程设置分类', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    invokeMock.mockReset();
    invokeMock.mockImplementation(async (command: string) => {
      // 启动引导里没有这个文件 → store 走 `loadStorageData()` 的真实读盘路径。
      // 返回 undefined 会让 `bootstrap[fileName]` 抛，init 静默失败、storage 永远是 null。
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      // FORK-MODIFICATION 2026-10-18：已信任主机分区也挂在这个分类下。
      // 不显式给空数组的话默认实现返回 undefined，分区会走错误分支 ——
      // 让本文件测连接列表的用例顺带被一个无关分区拖进错误态。
      if (command === 'remote_list_known_hosts') {
        return [];
      }

      return undefined;
    });
    lazyStoreSaveMock.mockReset();
    storageContents.clear();
  });

  describe('接线', () => {
    it('页签注册进了设置页的分类表', () => {
      const source = readSource('src/stores/runtime/settings.ts');

      expect(source).toContain('name: \'remote\'');
      expect(source).toContain('labelKey: \'settingsTabs.remote\'');
    });

    it('分类组件挂到了页签映射表上', () => {
      const source = readSource('src/modules/settings/ui/view.vue');

      expect(source).toContain('RemoteCategory');
      expect(source).toMatch(/remote:\s*RemoteCategory/);
    });
  });

  it('页签指向的组件存在，能真的渲染出来', async () => {
    const wrapper = mountedCategory();
    await flushPromises();

    expect(wrapper.find('[data-test="empty"]').exists()).toBe(true);
    // 网盘分区不再有「添加」按钮（用户 2026-11-11）：入口改成「添加网盘」区块。
    expect(wrapper.find('[data-test="add"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="add-netdisk"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="add-netdisk-baidu_pan"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="add-netdisk-quark_pan"]').exists()).toBe(true);
  });

  it('已保存的连接出现在列表里，并标出「已保存密码」', async () => {
    storageContents.set('connections', [{
      id: 'c1',
      name: '备份服务器',
      protocol: 'sftp',
      host: 'sftp.example.com',
      port: 22,
      username: 'me',
      initialPath: '/',
      proxy: {
        kind: 'direct',
      },
      credentialKey: 'c1',
    }]);

    const wrapper = mountedCategory();
    await flushPromises();

    expect(wrapper.find('[data-test="empty"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="entry-c1"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="has-password"]').exists()).toBe(true);
    expect(wrapper.text()).toContain('sftp.example.com:22');
  });

  it('列表里标出本地目录，没填的连接不标', async () => {
    storageContents.set('connections', [
      {
        id: 'c1',
        name: '有本地目录',
        protocol: 'ftp',
        host: 'a.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        localPath: 'C:/Users/test/Downloads',
        proxy: {
          kind: 'direct',
        },
      },
      {
        id: 'c2',
        name: '没有本地目录',
        protocol: 'ftp',
        host: 'b.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        proxy: {
          kind: 'direct',
        },
      },
    ]);

    const wrapper = mountedCategory();
    await flushPromises();

    // 本地目录是「会摆双屏」的唯一可见标志：填了必须看得见，没填不能留空位。
    expect(wrapper.find('[data-test="entry-c1"] [data-test="local-path"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="entry-c1"] [data-test="local-path"]').text())
      .toContain('C:/Users/test/Downloads');
    expect(wrapper.find('[data-test="entry-c2"] [data-test="local-path"]').exists()).toBe(false);
  });

  it('删除按钮删掉的是它自己那一条', async () => {
    storageContents.set('connections', [
      {
        id: 'c1',
        name: '甲',
        protocol: 'ftp',
        host: 'a.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        proxy: {
          kind: 'direct',
        },
      },
      {
        id: 'c2',
        name: '乙',
        protocol: 'ftp',
        host: 'b.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        proxy: {
          kind: 'direct',
        },
      },
    ]);

    const wrapper = mountedCategory();
    await flushPromises();

    await wrapper.find('[data-test="delete-c2"]').trigger('click');
    await flushPromises();

    const remaining = storageContents.get('connections') as { id: string }[];
    expect(remaining.map(connection => connection.id)).toEqual(['c1']);
    expect(wrapper.find('[data-test="entry-c2"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="entry-c1"]').exists()).toBe(true);
  });

  it('删除会顺带清掉该连接的密文', async () => {
    storageContents.set('connections', [{
      id: 'c1',
      name: '甲',
      protocol: 'ftp',
      host: 'a.example.com',
      port: 21,
      username: '',
      initialPath: '/',
      proxy: {
        kind: 'direct',
      },
      credentialKey: 'c1',
    }]);

    const wrapper = mountedCategory();
    await flushPromises();
    invokeMock.mockClear();

    await wrapper.find('[data-test="delete-c1"]').trigger('click');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledWith('remote_delete_credential', {
      key: 'c1',
    });
  });

  it('表单里有「测试连接」按钮', async () => {
    const wrapper = mountedCategory();
    await flushPromises();

    // 「测试连接」只存在于远程连接表单（网盘走 Cookie 登录，没有可测的协议端点），
    // 所以先切到「远程连接」分区。
    await switchSection(wrapper, 'remote');

    await wrapper.find('[data-test="add"]').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-test="test-connection"]').exists()).toBe(true);
  });

  it('测试连接按 outcome 渲染主文案，并把后端 message 原样展示', async () => {
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_test_connection') {
        return {
          outcome: 'auth_rejected',
          // message 是给用户看的、会改措辞。断言它能通过 = 断言界面在解析文案。
          message: 'AUTH_REJECTED_MESSAGE_MARKER',
          passwordChecked: true,
          tlsVerified: false,
        };
      }

      return undefined;
    });

    const wrapper = mountedCategory();
    await flushPromises();

    // 「测试连接」只存在于远程连接表单（网盘走 Cookie 登录，没有可测的协议端点）。
    await switchSection(wrapper, 'remote');

    await wrapper.find('[data-test="add"]').trigger('click');
    await flushPromises();

    await wrapper.find('[data-test="test-connection"]').trigger('click');
    await flushPromises();

    const result = wrapper.find('[data-test="test-result"]');
    expect(result.exists()).toBe(true);
    expect(result.attributes('data-outcome')).toBe('auth_rejected');
    expect(result.text()).toContain('settings.remote.test.outcome.auth_rejected');
    // 后端 message 现在作为副行原样展示：用户看到的诊断必须来自后端，
    // 不能再被固定文案盖住（2026-11-09 反馈：误报「服务器拒绝了用户名和密码」）。
    // 「不拿 message 做逻辑判断」这条设计意图仍然成立：主文案走 outcome。
    expect(result.text()).toContain('AUTH_REJECTED_MESSAGE_MARKER');
  });

  it('通过了但没核验密码时，必须额外提示', async () => {
    // 匿名 FTP / 免密 SFTP 同样会回 verified。少了这条提示，
    // 「连接成功」就会被读成「密码没问题」。
    invokeMock.mockImplementation(async (command: string) => {
      if (command === 'get_startup_storage_bootstrap') {
        return {};
      }

      if (command === 'remote_test_connection') {
        return {
          outcome: 'verified',
          message: 'ok',
          passwordChecked: false,
          tlsVerified: false,
        };
      }

      return undefined;
    });

    const wrapper = mountedCategory();
    await flushPromises();

    // 「测试连接」只存在于远程连接表单（网盘走 Cookie 登录，没有可测的协议端点）。
    await switchSection(wrapper, 'remote');

    await wrapper.find('[data-test="add"]').trigger('click');
    await flushPromises();

    await wrapper.find('[data-test="test-connection"]').trigger('click');
    await flushPromises();

    expect(wrapper.find('[data-test="test-result"]').attributes('data-outcome')).toBe('verified');
    expect(wrapper.find('[data-test="test-note-password"]').exists()).toBe(true);
  });

  it('组件与 store 共用同一份连接状态，不是各存一份', async () => {
    const wrapper = mountedCategory();
    await flushPromises();

    const store = useRemoteConnectionsStore();
    await store.saveConnection({
      name: '来自 store',
      protocol: 'ftp',
      host: 'store.example.com',
      port: 21,
      username: '',
      initialPath: '/',
      localPath: '',
      proxy: {
        kind: 'direct',
      },
      password: '',
      clearPassword: false,
    });
    await flushPromises();

    expect(wrapper.find('[data-test="empty"]').exists()).toBe(false);
    expect(wrapper.text()).toContain('来自 store');
  });

  // 用户 2026-10-18：有存档密码的连接要显示 `***`，和没存密码的一眼分得开。
  // 断言的是「星号」而不是某句文案 —— 文案会随语言变，星号不会。
  describe('存档密码标记', () => {
    it('有密码的显示星号，没密码的整块不显示', async () => {
      storageContents.set('connections', [
        {
          id: 'with-pw',
          name: '有密码',
          protocol: 'sftp',
          host: 'a.example.com',
          port: 22,
          username: '',
          initialPath: '/',
          proxy: {
            kind: 'direct',
          },
          credentialKey: 'with-pw',
        },
        {
          id: 'no-pw',
          name: '没密码',
          protocol: 'sftp',
          host: 'b.example.com',
          port: 22,
          username: '',
          initialPath: '/',
          proxy: {
            kind: 'direct',
          },
        },
      ]);

      const wrapper = mountedCategory();
      await flushPromises();

      const marked = wrapper.find('[data-test="entry-with-pw"] [data-test="has-password"]');
      expect(marked.exists()).toBe(true);
      expect(marked.text()).toBe('***');
      expect(wrapper.find('[data-test="entry-no-pw"] [data-test="has-password"]').exists())
        .toBe(false);
    });

    it('星号是掩码不是文案，可读名称仍挂在 title 上', async () => {
      storageContents.set('connections', [{
        id: 'c1',
        name: '备份',
        protocol: 'sftp',
        host: 'sftp.example.com',
        port: 22,
        username: '',
        initialPath: '/',
        proxy: {
          kind: 'direct',
        },
        credentialKey: 'c1',
      }]);

      const wrapper = mountedCategory();
      await flushPromises();

      // i18n 打回键名，所以 title 就是键本身 —— 正好证明文案来自翻译表，
      // 而不是把「已保存密码」硬编码进模板。
      expect(wrapper.find('[data-test="has-password"]').attributes('title'))
        .toBe('settings.remote.password.saved');
    });
  });

  // 用户 2026-10-18：导航栏悬停面板上每条连接右侧的设置按钮，点它直接进
  // 那条连接的编辑界面。
  describe('导航栏发来的编辑请求', () => {
    const connections = [
      {
        id: 'c1',
        name: '甲',
        protocol: 'ftp' as const,
        host: 'a.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        proxy: {
          kind: 'direct' as const,
        },
      },
      {
        id: 'c2',
        name: '乙',
        protocol: 'ftp' as const,
        host: 'b.example.com',
        port: 21,
        username: '',
        initialPath: '/',
        proxy: {
          kind: 'direct' as const,
        },
      },
    ];

    it('跳转到本分类时挂载着组件也会打开对应表单', async () => {
      storageContents.set('connections', connections);
      const store = useRemoteConnectionsStore();
      // 「先 request 再跳转」的那一路：组件是跳转后才挂载的。
      store.requestRemoteEdit('c2');

      const wrapper = mountedCategory();
      await flushPromises();

      const nameInput = wrapper.find('[data-test="field-name"]');
      expect(nameInput.exists()).toBe(true);
      expect((nameInput.element as HTMLInputElement).value).toBe('乙');
    });

    it('已经在本分类时再来一次请求也会打开', async () => {
      storageContents.set('connections', connections);
      const wrapper = mountedCategory();
      await flushPromises();
      expect(wrapper.find('[data-test="field-name"]').exists()).toBe(false);

      const store = useRemoteConnectionsStore();
      store.requestRemoteEdit('c1');
      await flushPromises();

      const nameInput = wrapper.find('[data-test="field-name"]');
      expect(nameInput.exists()).toBe(true);
      expect((nameInput.element as HTMLInputElement).value).toBe('甲');
    });

    it('请求指向一条已被删掉的连接时不开表单', async () => {
      storageContents.set('connections', connections);
      const store = useRemoteConnectionsStore();
      store.requestRemoteEdit('已经不存在的连接');

      const wrapper = mountedCategory();
      await flushPromises();

      // 凭空弹一个空表单，用户会以为多了一条连接。
      expect(wrapper.find('[data-test="field-name"]').exists()).toBe(false);
    });

    it('请求被消费掉，再次进入本分类不会重复弹表单', async () => {
      storageContents.set('connections', connections);
      const store = useRemoteConnectionsStore();
      store.requestRemoteEdit('c1');

      const first = mountedCategory();
      await flushPromises();
      expect(first.find('[data-test="field-name"]').exists()).toBe(true);
      first.unmount();

      const second = mountedCategory();
      await flushPromises();
      expect(second.find('[data-test="field-name"]').exists()).toBe(false);
    });

    it('导航栏发来的网盘新建请求走「添加网盘」新流程，不开内联表单', async () => {
      storageContents.set('connections', connections);
      const store = useRemoteConnectionsStore();
      // 悬停面板里点「添加网盘」：请求里带着协议。
      store.requestRemoteCreate('baidu_pan');

      const wrapper = mountedCategory();
      await flushPromises();

      // 新流程（用户 2026-11-11）：直接开内嵌登录窗，登录成功后再弹录入表单。
      // **不开**原来那张内联表单 —— 那张表单是「编辑 / 远程连接」用的形态。
      expect(wrapper.find('[data-test="field-name"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="field-host"]').exists()).toBe(false);
      // 登录窗确实被请求开了。
      expect(invokeMock).toHaveBeenCalledWith('remote_baidu_login_start');
    });

    it('不带协议的新建请求退回 FTP，服务器地址栏照常出现', async () => {
      storageContents.set('connections', connections);
      const store = useRemoteConnectionsStore();
      store.requestRemoteCreate();

      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="field-name"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="field-host"]').exists()).toBe(true);
    });

    it('新建请求被消费掉，再次进入本分类不会重复弹表单', async () => {
      storageContents.set('connections', connections);
      const store = useRemoteConnectionsStore();
      store.requestRemoteCreate();

      const first = mountedCategory();
      await flushPromises();
      expect(first.find('[data-test="field-name"]').exists()).toBe(true);
      first.unmount();

      const second = mountedCategory();
      await flushPromises();
      expect(second.find('[data-test="field-name"]').exists()).toBe(false);
    });
  });

  // 用户 2026-11-08：「添加连接页」的两个标签页上提到了页面顶部 ——
  // 整个分类按「网盘 / 远程连接」分栏，列表、添加按钮与表单全部跟着标签走。
  describe('分区标签与表单形态', () => {
    it('两个标签常驻在页面顶部，不用先点「添加」', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="remote-section-tabs"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="tab-netdisk"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="tab-remote"]').exists()).toBe(true);
    });

    // 用户 2026-11-09：内嵌窗口登录已验证可用，Cookie 讲解与输入框按要求删除。
    // 断言方向随之翻转 —— 这里守的是「Cookie 栏不得回来」。
    it('默认落在网盘页：没有服务器地址栏，也没有 Cookie 输入框', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="field-cookie"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="field-host"]').exists()).toBe(false);
      // 网盘分区不再有内联表单（用户 2026-11-11）：入口是「添加网盘」区块。
      expect(wrapper.find('[data-test="field-name"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="add-netdisk-baidu_pan"]').exists()).toBe(true);
    });

    it('切到「远程连接」标签，服务器地址栏出现、Cookie 栏消失', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      // ⚠️ 必须用 mousedown 而不是 click：reka-ui 的页签是在 `mousedown.left`
      // 上切换的（真实浏览器里 mousedown 本来就先于 click 到达），
      // 只派发 click 不会切页签。
      await switchSection(wrapper, 'remote');

      // 「添加」按钮现在只挂在远程连接分区（用户 2026-11-11）。
      await wrapper.find('[data-test="add"]').trigger('click');
      await flushPromises();

      expect(wrapper.find('[data-test="field-host"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="field-cookie"]').exists()).toBe(false);
    });

    it('导航栏请求指向网盘时，落地的就是网盘页', async () => {
      const store = useRemoteConnectionsStore();
      store.requestRemoteCreate('baidu_pan');

      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="field-cookie"]').exists()).toBe(false);
      expect(wrapper.find('[data-test="field-host"]').exists()).toBe(false);
      // 落地的是网盘分区：入口是「添加网盘」区块，远程连接那颗「添加」按钮不在。
      expect(wrapper.find('[data-test="add-netdisk"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="add"]').exists()).toBe(false);
    });

    it('导航栏请求指向 FTP 时，落地的就是远程连接页', async () => {
      const store = useRemoteConnectionsStore();
      store.requestRemoteCreate('ftp');

      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="field-host"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="field-cookie"]').exists()).toBe(false);
    });
  });

  // 用户 2026-11-11：两处 UI 改造。
  describe('分区介绍与添加网盘', () => {
    it('标题下面直接是选项卡，介绍下沉到当前分区内部', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      // 顶部不再是 SettingsItem 的 description，而是分区内部的一段介绍。
      expect(wrapper.find('[data-test="section-description"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="section-description"]').text())
        .toBe('settings.remote.nav.netdiskDescription');

      await switchSection(wrapper, 'remote');

      expect(wrapper.find('[data-test="section-description"]').text())
        .toBe('settings.remote.nav.remoteDescription');
    });

    it('点「百度网盘」直接开百度的内嵌登录窗', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      await wrapper.find('[data-test="add-netdisk-baidu_pan"]').trigger('click');
      await flushPromises();

      expect(invokeMock).toHaveBeenCalledWith('remote_baidu_login_start');
    });

    it('点「夸克网盘」直接开夸克的内嵌登录窗', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      await wrapper.find('[data-test="add-netdisk-quark_pan"]').trigger('click');
      await flushPromises();

      expect(invokeMock).toHaveBeenCalledWith('remote_quark_login_start');
    });
  });

  // 用户 2026-11-08：列表也要跟着标签走 —— 网盘连接不该出现在「远程连接」标签下。
  describe('列表按分区分列', () => {
    const netdiskConnection = {
      id: 'nd1',
      name: '我的网盘',
      protocol: 'baidu_pan' as const,
      host: 'pan.baidu.com',
      port: 443,
      username: '',
      initialPath: '/',
      proxy: {
        kind: 'direct' as const,
      },
    };

    const remoteConnection = {
      id: 'rc1',
      name: '机房服务器',
      protocol: 'sftp' as const,
      host: 'sftp.example.com',
      port: 22,
      username: 'me',
      initialPath: '/',
      proxy: {
        kind: 'direct' as const,
      },
    };

    it('默认落在有连接的分区上', async () => {
      storageContents.set('connections', [netdiskConnection, remoteConnection]);

      const wrapper = mountedCategory();
      await flushPromises();

      // 有网盘连接就落网盘分区 —— 用户不该看见一个「什么都没有」的列表。
      expect(wrapper.find('[data-test="entry-nd1"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="entry-rc1"]').exists()).toBe(false);
    });

    it('切到「远程连接」标签，只剩远程连接', async () => {
      storageContents.set('connections', [netdiskConnection, remoteConnection]);

      const wrapper = mountedCategory();
      await flushPromises();
      await switchSection(wrapper, 'remote');

      expect(wrapper.find('[data-test="entry-rc1"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="entry-nd1"]').exists()).toBe(false);
    });

    it('只有远程连接时，默认就落在远程连接分区', async () => {
      storageContents.set('connections', [remoteConnection]);

      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="entry-rc1"]').exists()).toBe(true);
    });

    it('分区请求能直接落到指定标签上', async () => {
      storageContents.set('connections', [netdiskConnection, remoteConnection]);

      const store = useRemoteConnectionsStore();
      // 主页「位置」页的两个入口点进来时的请求。
      store.requestRemoteSection('remote');

      const wrapper = mountedCategory();
      await flushPromises();

      expect(wrapper.find('[data-test="entry-rc1"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="entry-nd1"]').exists()).toBe(false);
    });

    it('分区请求被消费掉，再进这一页不会被拽回某个标签', async () => {
      storageContents.set('connections', [netdiskConnection, remoteConnection]);

      const store = useRemoteConnectionsStore();
      store.requestRemoteSection('remote');

      const first = mountedCategory();
      await flushPromises();
      first.unmount();

      const second = mountedCategory();
      await flushPromises();

      // 回到默认分区（有网盘连接的网盘分区），而不是上一次的远程连接。
      expect(second.find('[data-test="entry-nd1"]').exists()).toBe(true);
      expect(second.find('[data-test="entry-rc1"]').exists()).toBe(false);
    });

    it('空态文案跟着分区走', async () => {
      const wrapper = mountedCategory();
      await flushPromises();

      // i18n 打回键名，所以空态就是键本身。
      expect(wrapper.find('[data-test="empty"]').text()).toBe('settings.remote.nav.emptyNetdisk');

      await switchSection(wrapper, 'remote');

      expect(wrapper.find('[data-test="empty"]').text()).toBe('settings.remote.nav.emptyRemote');
    });
  });
});
