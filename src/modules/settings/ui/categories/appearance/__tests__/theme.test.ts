// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 这组测试盯的是「外观 → 主题」选择器（AFM 3.6.5 新增六套 macOS Finder 风格变体）：
//
// 1. **必须真的渲染出 9 个选项**：注册表里有 id 不等于界面上看得见。组合框的选项来自
//    三段过滤（经典 / 内置变体 / 扩展主题），任何一段的过滤条件写错，主题就会从界面里
//    消失，而 npm run ts 与单测都不会报错。
// 2. **分组标题必须在**：变体主题要单独归到「macOS 风格主题」这一组，否则用户要从
//    一长串里自己找。
// 3. **六个变体的名字键不能拼错**：拼错不会报错，只会渲染出原始 key 字符串。
//
// 替身组件一律用 h() 渲染函数，不写 HTML 模板字符串：模板字符串里的标签会让
// 解析器把这个 .ts 文件当成 Vue 模板来读，直接报解析错误。

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import type * as VueI18nModule from 'vue-i18n';
import { h, type VNode } from 'vue';
import { mount } from '@vue/test-utils';
import Theme from '@/modules/settings/ui/categories/appearance/theme.vue';

const { userSettingsStoreMock, extensionsStorageStoreMock } = vi.hoisted(() => ({
  userSettingsStoreMock: {
    userSettings: {
      theme: 'light',
    },
    set: vi.fn(async () => {}),
    setThemeTransitionOrigin: vi.fn(),
  },
  extensionsStorageStoreMock: {
    extensionsData: {
      installedExtensions: {},
    },
  },
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => userSettingsStoreMock,
}));

vi.mock('@/stores/storage/extensions', () => ({
  useExtensionsStorageStore: () => extensionsStorageStoreMock,
}));

// 替身组件的 setup 参数必须显式标类型：vi.mock 的工厂函数没有上下文类型，
// 不标的话 tsconfig.vitest.json（strict）下就是 implicit any，npm run ts 直接红。
vi.mock('@/modules/settings', () => ({
  SettingsItem: {
    props: ['title', 'description', 'icon'],
    setup(_props: Record<string, unknown>, { slots }: { slots: { default?: () => VNode[] } }) {
      return () => h('section', { class: 'settings-view-item' }, slots.default?.());
    },
  },
}));

// 组合框替身只负责把 options / optionGroups 摊平成可断言的文本节点。
vi.mock('@/components/base', () => ({
  BaseCombobox: {
    props: ['modelValue', 'options', 'optionGroups', 'by'],
    setup(props: {
      optionGroups?: {
        heading?: string;
        options?: {
          value: string;
          name: string;
        }[];
      }[];
    }) {
      return () => h('div', { class: 'base-combobox' }, (props.optionGroups ?? []).map(
        (group, index) => h('div', {
          class: 'combobox-group',
          key: index,
        }, [
          group.heading
            ? h('div', { class: 'combobox-group__heading' }, group.heading)
            : null,
          ...(group.options ?? []).map(option =>
            h('div', {
              class: 'combobox-option',
              key: option.value,
            }, option.name)),
        ]),
      ));
    },
  },
}));

vi.mock('vue-i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof VueI18nModule>();

  return {
    ...actual,
    useI18n: () => ({
      t: (key: string) => key,
    }),
  };
});

const MAC_GROUP_HEADING = 'settings.homeBannerEffects.theme.macVariantThemes';
const MAC_VARIANT_NAME_KEYS = [
  'settings.homeBannerEffects.theme.macVariants.classicLight',
  'settings.homeBannerEffects.theme.macVariants.classicDark',
  'settings.homeBannerEffects.theme.macVariants.modernLight',
  'settings.homeBannerEffects.theme.macVariants.modernDark',
  'settings.homeBannerEffects.theme.macVariants.glassLight',
  'settings.homeBannerEffects.theme.macVariants.glassDark',
];
const CLASSIC_NAME_KEYS = [
  'settings.homeBannerEffects.theme.themeTypeRadio.dark',
  'settings.homeBannerEffects.theme.themeTypeRadio.light',
  'system',
];

describe('theme settings', () => {
  beforeEach(() => {
    userSettingsStoreMock.userSettings.theme = 'light';
    userSettingsStoreMock.set.mockClear();
  });

  it('lists the three classic themes plus the six macOS variants', () => {
    const wrapper = mount(Theme);
    const options = wrapper.findAll('.combobox-option');

    expect(options, '主题选择器应当有 9 个选项').toHaveLength(9);
  });

  it('groups the six macOS variants under their own heading', () => {
    const wrapper = mount(Theme);
    const headings = wrapper.findAll('.combobox-group__heading');

    expect(headings, '经典主题没有标题，变体主题有一个分组标题').toHaveLength(1);
    expect(headings[0].text()).toBe(MAC_GROUP_HEADING);
  });

  it('names every classic theme and every macOS variant', () => {
    const wrapper = mount(Theme);
    const names = wrapper.findAll('.combobox-option').map(option => option.text());

    for (const key of CLASSIC_NAME_KEYS) {
      expect(names, `经典主题应当包含 ${key}`).toContain(key);
    }

    for (const key of MAC_VARIANT_NAME_KEYS) {
      expect(names, `变体主题应当包含 ${key}`).toContain(key);
    }
  });

  it('keeps the macOS variants in their own group', () => {
    const wrapper = mount(Theme);
    const groups = wrapper.findAll('.combobox-group');

    expect(groups).toHaveLength(2);
    expect(groups[0].findAll('.combobox-option')).toHaveLength(3);
    expect(groups[1].findAll('.combobox-option')).toHaveLength(6);
  });
});
