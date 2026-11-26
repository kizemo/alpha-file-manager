// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, it, expect, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import FileBrowserNewItemDialog from '../file-browser-new-item-dialog.vue';

vi.mock('vue-i18n', () => ({
  useI18n: () => ({
    t: (key: string) => key,
  }),
}));

/**
 * D1 回归测试（2026-11-09 批次 F.9）。
 *
 * 缺陷现象：远程「新建目录」失败后，对话框的「创建」按钮**永久变灰**，
 * 用户既看不到报错（失败 toast 缺失）也点不动按钮，只能关掉重来。
 *
 * 根因：对话框的 `isSubmitting` 只在 `watch(name)` 里复位；父组件此前
 * 只在**成功**时关对话框，失败时既不复位 `isSubmitting` 也不给父级信号。
 *
 * 修法：`confirm` 事件多带一个 `done` 回执，父组件在失败时回执 `false`，
 * 对话框据此复位 `isSubmitting`。
 */
function mountDialog() {
  return mount(FileBrowserNewItemDialog, {
    props: {
      'open': true,
      'type': 'directory',
      'onUpdate:open': () => {},
    },
    global: {
      stubs: {
        Dialog: { template: '<div class="dialog-stub"><slot /></div>' },
        DialogContent: { template: '<div><slot /></div>' },
        DialogHeader: { template: '<div><slot /></div>' },
        DialogTitle: { template: '<div><slot /></div>' },
        DialogFooter: { template: '<div><slot /></div>' },
        Input: {
          props: ['modelValue'],
          template: '<input :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)">',
        },
        Button: {
          props: ['disabled'],
          template: '<button :disabled="disabled"><slot /></button>',
        },
      },
    },
  });
}

describe('「新建」对话框的提交状态', () => {
  it('提交失败并回执后，「创建」按钮恢复可点', async () => {
    const wrapper = mountDialog();

    await wrapper.find('input').setValue('afm-fail');

    const button = wrapper.find('button');
    expect(button.attributes('disabled')).toBeUndefined();

    await button.trigger('click');

    // 提交中：按钮必须禁用，避免重复提交。
    expect(wrapper.find('button').attributes('disabled')).toBeDefined();

    const emitted = wrapper.emitted('confirm');
    expect(emitted).toBeTruthy();
    expect(emitted![0][0]).toBe('afm-fail');

    // 父组件回执「失败」。
    const done = emitted![0][1] as unknown as (ok: boolean) => void;
    done(false);
    await wrapper.vm.$nextTick();

    // **D1 的核心断言**：失败后按钮恢复可点，能改名重试。
    expect(wrapper.find('button').attributes('disabled')).toBeUndefined();
  });

  it('父组件还没回执时，按钮保持禁用（不重复提交）', async () => {
    const wrapper = mountDialog();

    await wrapper.find('input').setValue('afm-pending');
    await wrapper.find('button').trigger('click');

    expect(wrapper.find('button').attributes('disabled')).toBeDefined();
  });

  it('成功回执时按钮保持禁用，由父组件负责关闭对话框', async () => {
    const wrapper = mountDialog();

    await wrapper.find('input').setValue('afm-ok');
    await wrapper.find('button').trigger('click');

    const emitted = wrapper.emitted('confirm');
    const done = emitted![0][1] as unknown as (ok: boolean) => void;
    done(true);
    await wrapper.vm.$nextTick();

    expect(wrapper.find('button').attributes('disabled')).toBeDefined();
  });
});
