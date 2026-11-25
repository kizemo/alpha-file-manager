// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (批次 58)：收藏卡就地编辑的**键盘冒泡**回归测试。
//
// 背景（批次 58 真机 CDP 走查坐实的缺陷）：卡片主体是 `div[role=button]`，
// 上面挂了 `@keydown.enter.prevent="handleClick"` 与 `@keydown.space.prevent="handleClick"`。
// 就地编辑的输入框是它的子节点，而输入框自己只写了 `.prevent`（阻止默认行为），
// **没有** `.stop`（阻止冒泡）⇒ 两个后果：
//
//   1. 在标题框里按 Enter 保存 → 事件继续冒泡到卡片 → 触发 handleClick → 卡片跳走
//      （真机实测：保存成功后页面从 /dashboard 被导航到 /navigator）；
//   2. 在标题框里按空格 → 冒泡到卡片的 `@keydown.space.prevent` → 默认行为被取消
//      ⇒ 空格**根本打不出来**（带空格的收藏标题改不了）。
//
// 这组测试把两条都钉死：编辑输入框里的按键不得触发卡片的点击 / 跳转，
// 空格不得被取消默认行为；同时锁住路径编辑器预填的是**完整路径**。

import { describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';

vi.mock('vue-i18n', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue-i18n')>();

  return {
    ...actual,
    useI18n: () => ({
      t: (key: string) => key,
      locale: { value: 'ch' },
    }),
  };
});

// 缩略图走 Tauri invoke，jsdom 里没有 IPC —— 直接桩掉。
vi.mock('@/modules/dashboard/composables/use-dashboard-entry-preview', async () => {
  const { ref } = await import('vue');

  return {
    useDashboardEntryPreview: () => ({ thumbnailSrc: ref(null) }),
  };
});

const FAVORITE_PATH = 'E:/办公文件/C1人力资源管理/赤兔报表';

async function mountCard() {
  const { default: EntryCard } = await import('../entry-card.vue');

  return mount(EntryCard, {
    props: {
      path: FAVORITE_PATH,
      editable: true,
      pathDisplay: 'full',
    },
  });
}

/** 派发一个会冒泡、可取消的键盘事件，返回事件本身以便断言 defaultPrevented。 */
function pressKey(element: Element, key: string): KeyboardEvent {
  const event = new KeyboardEvent('keydown', {
    key,
    bubbles: true,
    cancelable: true,
  });

  element.dispatchEvent(event);

  return event;
}

describe('entry-card — 就地编辑的按键不得冒泡到卡片（批次 58）', () => {
  it('标题编辑器按 Enter：提交标题，且不触发卡片点击（不得跳转）', async () => {
    const wrapper = await mountCard();

    await wrapper.find('.entry-card__name').trigger('click');

    const input = wrapper.find('.entry-card__input--title');
    expect(input.exists()).toBe(true);

    await input.setValue('新标题');
    await input.trigger('keydown', { key: 'Enter' });

    expect(wrapper.emitted('update:title')).toEqual([['新标题']]);
    expect(wrapper.emitted('click')).toBeUndefined();

    wrapper.unmount();
  });

  it('标题编辑器按空格：默认行为不得被取消（空格要能打出来），且不触发卡片点击', async () => {
    const wrapper = await mountCard();

    await wrapper.find('.entry-card__name').trigger('click');

    const input = wrapper.find('.entry-card__input--title');
    const event = pressKey(input.element, ' ');

    expect(event.defaultPrevented).toBe(false);
    expect(wrapper.emitted('click')).toBeUndefined();

    wrapper.unmount();
  });

  it('路径编辑器预填完整路径；按 Enter 提交路径，且不触发卡片点击', async () => {
    const wrapper = await mountCard();

    expect(wrapper.find('.entry-card__path').text()).toBe(FAVORITE_PATH);

    await wrapper.find('.entry-card__path').trigger('click');

    const input = wrapper.find('.entry-card__input--path');
    expect(input.exists()).toBe(true);
    expect((input.element as HTMLInputElement).value).toBe(FAVORITE_PATH);

    await input.setValue('E:/办公文件/新目录');
    await input.trigger('keydown', { key: 'Enter' });

    expect(wrapper.emitted('update:path')).toEqual([['E:/办公文件/新目录']]);
    expect(wrapper.emitted('click')).toBeUndefined();

    wrapper.unmount();
  });

  it('路径编辑器按 Escape：取消编辑，既不提交路径也不触发卡片点击', async () => {
    const wrapper = await mountCard();

    await wrapper.find('.entry-card__path').trigger('click');

    const input = wrapper.find('.entry-card__input--path');
    await input.setValue('E:/别的目录');
    await input.trigger('keydown', { key: 'Escape' });

    expect(wrapper.emitted('update:path')).toBeUndefined();
    expect(wrapper.emitted('click')).toBeUndefined();
    expect(wrapper.find('.entry-card__path').text()).toBe(FAVORITE_PATH);

    wrapper.unmount();
  });
});
