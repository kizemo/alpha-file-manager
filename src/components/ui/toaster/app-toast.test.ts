// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
//
// 真机复验（2026-11-14）发现的缺陷：应用里所有 toast 都只弹出一张**空卡片**，
// 一个字都看不见。根因在 `app-toast.ts` 的包装器签名与调用方形态对不上：
//
//   包装器：  toast(message: string, data?: ExternalToast)
//   调用方：  toast({ title, description })        ← 全仓 7 处都是这个形态
//
// ⇒ `showStaticToast` 把**整个对象**当成 `title` 传下去，
//   `toast-static.vue` → `toast-header.vue` 的 `props.title.indexOf('|')`
//   对对象调用字符串方法 ⇒ 抛错 ⇒ 整棵 ToastHeader 渲染成注释节点
//   ⇒ 卡片只剩背景与边框，**没有任何文字**。
//
// 这个用例挂的是**真的** Toaster 与**真的** toast()，不走 mock ——
// 所以它能真的抓到「渲染成空壳」这件事。

import { describe, expect, it } from 'vitest';
import { mount } from '@vue/test-utils';
import { createI18n } from 'vue-i18n';
import { nextTick } from 'vue';
import { toast } from './app-toast';
import Toaster from './toaster.vue';

function mountToaster() {
  const i18n = createI18n({
    legacy: false,
    locale: 'en',
    fallbackWarn: false,
    missingWarn: false,
    messages: {},
  });

  return mount(Toaster, {
    global: {
      plugins: [i18n],
      stubs: {
        transition: false,
      },
    },
  });
}

async function flushToasts(): Promise<void> {
  for (let i = 0; i < 5; i += 1) {
    await nextTick();
    await new Promise((resolve) => {
      setTimeout(resolve, 20);
    });
  }

  await nextTick();
}

describe('toast 包装器：真的 Toaster + 真的 toast()', () => {
  it('两参形态 toast(标题, { description }) 能把标题渲染出来', async () => {
    const wrapper = mountToaster();

    toast('标题文字', { description: '描述文字' });
    await flushToasts();

    expect(wrapper.text()).toContain('标题文字');
    expect(wrapper.text()).toContain('描述文字');

    wrapper.unmount();
  });

  it('对象形态 toast({ title, description }) 也必须把标题渲染出来 —— 现状是空卡片', async () => {
    const wrapper = mountToaster();

    toast({
      title: '对象标题',
      description: '对象描述',
    } as never);
    await flushToasts();

    // 真机（2026-11-14）实拍：卡片在、边框在、文字不在。
    expect(wrapper.text()).toContain('对象标题');
    expect(wrapper.text()).toContain('对象描述');

    wrapper.unmount();
  });
});
