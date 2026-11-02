// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION 2026-11-02：「新功能」条目从「设置 → 常规」迁到「设置 → 关于」。
 *
 * 这个测试盯的是**归属**与**可达性**，三件事一件都不能少：
 * 1. **注册表里它挂在 about 分类下**：分类归属只写在 `stores/runtime/settings.ts` 的
 *    `allSections` 数组里，写错分类不会报错，只会让条目静悄悄跑到别的页签去。
 * 2. **入口真的渲染得出来、按钮真的能打开更新日志对话框**：条目被搬来搬去时
 *    最容易丢的就是那个按钮，而丢了也不会报错，只是点了没反应。
 * 3. **设置页搜索没断**：搜索按 titleKey / tags **跨语言**匹配、**不看分类**，
 *    所以条目换分类后搜索照样能命中 —— 这条用真实的 17 个语言包验，别只验中文。
 *
 * 为什么不直接 import `@/stores/runtime/settings` 来断言注册表：
 * 那个模块会把 router / workspaces 一整条依赖图拖进测试环境，在没有完整应用上下文时
 * 直接 `routes is not iterable` 崩掉（与 `remote-category.test.ts` 同一原因）。
 * 注册表是纯数据，读源码验更稳，且验的仍然是真实的那份文件。
 */

import { describe, expect, it, vi } from 'vitest';
import { existsSync, readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { h, type VNode } from 'vue';
import { mount } from '@vue/test-utils';
import WhatsNew from '@/modules/settings/ui/categories/about/whats-new.vue';
import { filterSettingsSections } from '@/modules/settings/utils/settings-search';

const { openChangelogMock, setUserSettingMock } = vi.hoisted(() => ({
  openChangelogMock: vi.fn(),
  setUserSettingMock: vi.fn(async () => {}),
}));

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

vi.mock('@/modules/changelog', () => ({
  useChangelog: () => ({
    open: openChangelogMock,
  }),
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => ({
    userSettings: {
      changelog: {
        showOnUpdate: true,
      },
    },
    set: setUserSettingMock,
  }),
}));

// `@/modules/settings` 的 barrel 会把 pages/settings.vue 与 nav.vue 都导出进来，
// 一路拖到 router，而 router 在测试环境里 `routes is not iterable` 直接崩
// （与 remote-category.test.ts / thumbnail-cache.test.ts 同一处理）。
// 注意：替身必须把 `description` 插槽也渲染出来 —— 「查看更新日志」按钮就在
// description 插槽里，漏渲染会让断言变成「按钮不存在」，而不是真的验证按钮。
vi.mock('@/modules/settings', () => ({
  SettingsItem: {
    props: ['title', 'description', 'icon'],
    setup(props: { title?: string }, { slots }: {
      slots: {
        default?: () => VNode[];
        description?: () => VNode[];
      };
    }) {
      return () => h('section', { class: 'settings-view-item' }, [
        h('h3', { class: 'settings-item-title' }, props.title ?? ''),
        h('div', { class: 'settings-item-description' }, slots.description?.() ?? []),
        ...(slots.default?.() ?? []),
      ]);
    },
  },
}));

vi.mock('@/components/ui/button', () => ({
  Button: {
    props: ['variant', 'size', 'class'],
    setup(_props: Record<string, unknown>, { slots, emit }: {
      slots: { default?: () => VNode[] };
      emit: (event: string, ...args: unknown[]) => void;
    }) {
      return () => h('button', {
        class: 'button-stub',
        onClick: () => emit('click'),
      }, slots.default?.());
    },
  },
}));

vi.mock('@/components/ui/switch', () => ({
  Switch: {
    props: ['modelValue', 'id'],
    setup(props: { modelValue?: boolean }, { emit }: {
      emit: (event: string, ...args: unknown[]) => void;
    }) {
      return () => h('input', {
        class: 'switch-stub',
        type: 'checkbox',
        checked: props.modelValue === true,
        onChange: () => emit('update:modelValue', !props.modelValue),
      });
    },
  },
}));

interface SectionMeta {
  key: string;
  titleKey: string;
  tags: string;
  category: string;
}

function readSource(relativePath: string): string {
  return readFileSync(resolve(process.cwd(), relativePath), 'utf8');
}

/**
 * 把 `allSections` 注册表从源码里抠出来。
 *
 * 注册表是纯数据，每条都是同构的 `{ key, titleKey, tags, component, category }`，
 * 所以按这个形状解析即可。解析结果为空或明显偏少时，测试会直接红 ——
 * 免得「解析器没匹配上」被当成「注册表里没有这一条」。
 */
function parseSectionRegistry(source: string): SectionMeta[] {
  const block = source.match(/allSections\.value = \[([\s\S]*?)\n {4}\];/);

  if (!block) {
    throw new Error('未能在 stores/runtime/settings.ts 中定位 allSections 注册表');
  }

  const entryPattern = /key:\s*'([^']+)',\s*titleKey:\s*'([^']+)',\s*tags:\s*'([^']+)',\s*component:\s*[^,]+,\s*category:\s*'([^']+)'/g;
  const sections: SectionMeta[] = [];

  for (const match of block[1].matchAll(entryPattern)) {
    sections.push({
      key: match[1],
      titleKey: match[2],
      tags: match[3],
      category: match[4],
    });
  }

  return sections;
}

const registrySource = readSource('src/stores/runtime/settings.ts');
const registry = parseSectionRegistry(registrySource);

describe('「新功能」条目从「常规」迁到「关于」', () => {
  it('注册表里它只出现一次，且挂在 about 分类下', () => {
    // 解析器失灵时（正则没匹配上）会得到空数组，后面两条断言会假绿 —— 先钉住下限。
    expect(registry.length, '注册表解析结果明显偏少，正则多半没匹配上').toBeGreaterThan(30);
    expect(registry.map(section => section.key)).toContain('aboutInfo');

    const whatsNewEntries = registry.filter(section => section.key === 'whatsNew');

    expect(whatsNewEntries, 'whatsNew 条目应当有且只有一条').toHaveLength(1);
    expect(whatsNewEntries[0].category).toBe('about');
    expect(registry.filter(section => section.category === 'general').map(section => section.key))
      .not.toContain('whatsNew');
  });

  it('组件从 about 目录加载，general 目录下的旧文件已经不在了', () => {
    expect(registrySource).toContain('import(\'@/modules/settings/ui/categories/about/whats-new.vue\')');
    expect(registrySource).not.toContain('categories/general/whats-new.vue');
    expect(existsSync(resolve(process.cwd(), 'src/modules/settings/ui/categories/about/whats-new.vue'))).toBe(true);
    expect(existsSync(resolve(process.cwd(), 'src/modules/settings/ui/categories/general/whats-new.vue'))).toBe(false);
  });

  it('「关于」页里的入口渲染得出来，按钮能打开更新日志对话框', async () => {
    const wrapper = mount(WhatsNew);

    expect(wrapper.find('.settings-item-title').text()).toBe('changelog.settingsTitle');

    const button = wrapper.find('.button-stub');

    expect(button.exists()).toBe(true);
    expect(button.text()).toContain('changelog.viewChangelog');

    await button.trigger('click');

    expect(openChangelogMock, '「查看更新日志」应当打开更新日志对话框').toHaveBeenCalled();
  });

  it('「应用更新时显示更新日志」的开关跟着条目一起搬过来了', () => {
    const wrapper = mount(WhatsNew);

    expect(wrapper.text()).toContain('changelog.showOnUpdate');

    const toggle = wrapper.find('.switch-stub');

    expect(toggle.exists()).toBe(true);
    expect((toggle.element as HTMLInputElement).checked).toBe(true);
  });

  it('「常规」页不再含有这一条，「关于」页含有', () => {
    expect(filterSettingsSections(registry, '', 'general').map(section => section.key))
      .not.toContain('whatsNew');
    expect(filterSettingsSections(registry, '', 'about').map(section => section.key))
      .toContain('whatsNew');
  });

  it('设置页搜索仍然命中这一条（中英文都命中）', () => {
    // 走真实的 17 个语言包（settings-search.ts 内部读 @/localization/data），
    // 确认迁移没有把搜索索引打断。
    //
    // 搜索是拿**译文值**做跨语言包含匹配，所以关键词必须用语言包里真实出现的写法：
    // 中文 tags =「新功能, 更新日志, 版本说明, …」；英文 tags =「what's new, changelog,
    // release notes, …」（注意撇号，以及是 release notes 而不是 version notes）。
    const searchTerms = [
      '新功能',
      '更新日志',
      '版本说明',
      'changelog',
      'release notes',
      'what\'s new',
    ];

    for (const term of searchTerms) {
      expect(
        filterSettingsSections(registry, term, 'about').map(section => section.key),
        `搜索「${term}」应当命中 whatsNew 条目`,
      ).toContain('whatsNew');
    }
  });
});
