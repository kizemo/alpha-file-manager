// SPDX-License-Identifier: GPL-3.0-or-later
// GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-25：本文件为 Alpha File Manager 分支新增（上游不存在本文件），非上游原始代码。
// 详见 docs/ALPHA_FORK_MODIFICATION.md 与 docs/ALPHA_FORK_NOTICES/ 下的逐文件说明。

/**
 * 批次 56 · Escape 优先级链守卫（对应批次 55 的 W-19 待拍板项）
 *
 * 背景：批次 56 用 release 包 + WebView2 CDP（端口 45777）在私有桌面沙箱做键盘路径走查，
 * 按下 Escape 后全局搜索面板并未关闭（07-after-escape.png 与 03-results-ready.png
 * 逐元素比对一致，面板 input 仍在 DOM 中）。回源码核实，navigator.vue 的
 * handleEscapeKey 是**有优先级链**的：只要选中行非空，Escape 先清除选中并 return true，
 * 面板的 close 分支根本轮不到。也就是说「Escape 没关面板」不是缺陷，而是设计行为。
 *
 * 但这个行为极其脆弱：批次 55 曾经为了让面板自己处理 Escape 而把面板的 Escape 处理
 * 「原样搬进」navigator.vue 的链里（用户已裁决回退）。若有人再次调换链的顺序、
 * 摘掉某条分支、或把面板改成自己监听 Escape，回归只会以「按了没反应」的形式
 * 在真机暴露，而界面级取证成本高（需要重装 + 沙箱 + 注入 CDP 旗标）。
 *
 * 因此这里用源码级断言把链的**顺序、注册点、按键定义、面板不本地监听 Escape**钉住。
 * 断言写法沿用 navigator-tree-wiring.test.ts 的先例（读源文件 + 正则），
 * 键名与选择器必须与源码同步，改源码必须同步改本测试。
 *
 * 注意：本文件是「源码文本守卫」，不是行为测试。它不能证明运行时的合成事件路径 ——
 * 那需要 OS 级键盘注入，本项目的私有桌面沙箱做不到（AGENTS.md §2.8 第 667 行）。
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { describe, expect, it } from 'vitest';

const navigatorSource = readFileSync(
  resolve(process.cwd(), 'src/modules/navigator/pages/navigator.vue'),
  'utf8',
);
const globalSearchViewSource = readFileSync(
  resolve(process.cwd(), 'src/modules/global-search/global-search-view.vue'),
  'utf8',
);
const shortcutsSource = readFileSync(
  resolve(process.cwd(), 'src/stores/runtime/shortcuts.ts'),
  'utf8',
);

/** 从 fromIndex 起截到下一个 `\n  {` 数组元素边界，取出一段源码片段。 */
function sliceFunction(source: string, header: string): string {
  const start = source.indexOf(header);
  if (start < 0) return '';
  let end = source.indexOf('\n}', start);
  if (end < 0) end = source.length;
  return source.slice(start, end);
}

describe('批次 56：Escape 优先级链的分支顺序（navigator.vue）', () => {
  const handleEscapeKey = sliceFunction(navigatorSource, 'function handleEscapeKey(): boolean {');

  it('取到了 handleEscapeKey 的函数体', () => {
    expect(handleEscapeKey.length).toBeGreaterThan(0);
  });

  it('链的优先级顺序为：reka → dismissal → 剪贴板 → 选中 → 全局搜索', () => {
    const order = [
      'data-dismissable-layer',
      'dismissalLayerStore.hasLayers',
      'clipboardStore.hasItems',
      'selectedEntries.value.length',
      'globalSearchStore.isOpen',
    ];
    const positions = order.map(token => handleEscapeKey.indexOf(token));
    positions.forEach((position, index) => {
      expect(position, `链中缺少分支：${order[index]}`).toBeGreaterThanOrEqual(0);
    });

    for (let i = 1; i < positions.length; i++) {
      expect(
        positions[i],
        `分支顺序被调换：${order[i]} 必须排在 ${order[i - 1]} 之后`,
      ).toBeGreaterThan(positions[i - 1]);
    }
  });

  it('选中文项的分支仍然清除选中并接管按键（不会被摘掉）', () => {
    expect(handleEscapeKey).toMatch(/if \(selectedEntries\.value\.length > 0\)/);
    expect(handleEscapeKey).toMatch(/clearAllSelections\(\);\s*\n\s*return true/);
  });

  it('全局搜索的分支只在前面所有分支都不成立时才关面板', () => {
    expect(handleEscapeKey).toMatch(/if \(globalSearchStore\.isOpen\) \{\s*\n\s*globalSearchStore\.close\(\);\s*\n\s*return true/);
    // 兜底 return false：链走完无人接管时不吞按键
    expect(handleEscapeKey.trimEnd()).toMatch(/return false;\s*$/);
  });

  it('Escape 处理器注册在 shortcuts 总线上（未挂到 document 或 window）', () => {
    expect(navigatorSource).toMatch(/shortcutsStore\.registerHandler\(\s*'escape',\s*handleEscapeKey\s*,?\s*\)/);
    expect(navigatorSource).not.toMatch(/document\.addEventListener\(\s*['"]keydown/);
    expect(navigatorSource).not.toMatch(/window\.addEventListener\(\s*['"]keydown/);
  });

  it('shortcuts.ts 中 escape 的定义未被改动：按键 Escape、作用域 global、无条件', () => {
    const start = shortcutsSource.indexOf('id: \'escape\'');
    expect(start).toBeGreaterThanOrEqual(0);
    // 定义块从 id: 'escape' 起、到下一个数组元素 'id:' 之前结束（避免定长窗口与缩进假设）
    const nextEntry = shortcutsSource.indexOf('\n    id: \'', start + 1);
    const block = shortcutsSource.slice(start, nextEntry < 0 ? start + 400 : nextEntry);
    expect(block).toMatch(/defaultKeys:\s*\{\s*\n\s*key:\s*'Escape',/);
    expect(block).toMatch(/scope:\s*'global'/);
    expect(block).toMatch(/conditions:\s*\{\s*\}/);
    expect(block).toMatch(/isReadOnly:\s*true/);
  });
});

describe('批次 56：全局搜索面板自己不抢 Escape（global-search-view.vue）', () => {
  it('输入框只挂 down / enter 两个按键，不挂 esc（Escape 交给总线处理）', () => {
    // 面板的输入框是 <Input> 组件（L511），没有 data-test 属性，因此直接按绑定断言。
    expect(globalSearchViewSource).toMatch(/@keydown\.down\.prevent="focusFirstResult"/);
    expect(globalSearchViewSource).toMatch(/@keydown\.enter\.prevent="handleSearchInputEnter"/);
    expect(globalSearchViewSource).not.toMatch(/@keydown\.esc/);
    expect(globalSearchViewSource).not.toMatch(/@keydown\.escape/);
    expect(globalSearchViewSource).not.toMatch(/@keyup\.esc/);
  });

  it('面板不自行监听 keydown（否则会和 navigator.vue 的链抢按键）', () => {
    expect(globalSearchViewSource).not.toMatch(/addEventListener\(\s*['"]keydown['"]/);
    expect(globalSearchViewSource).not.toMatch(/onKey\(?\s*['"]escape/i);
  });

  it('✕ 关闭按钮走 handleClose，由父组件（navigator.vue）决定关不关', () => {
    expect(globalSearchViewSource).toMatch(/function handleClose\(\) \{\s*\n\s*emit\('close'\);/);
    expect(globalSearchViewSource).toMatch(/class="global-search-view__close"\s*@click="handleClose"/);
    // 面板本体不直接调 store.close —— 关闭动作必须回到 navigator.vue 的链上
    expect(globalSearchViewSource).not.toMatch(/globalSearchStore\.close\(\)/);
  });
});
