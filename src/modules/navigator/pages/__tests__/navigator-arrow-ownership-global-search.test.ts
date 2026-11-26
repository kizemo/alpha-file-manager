// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * W-19 回归守卫：全局搜索结果列表的方向键归属（源码文本断言，非行为测试）。
 *
 * 缺陷本体（批次 59 真机取证坐实，见 handoff §三十四）：裸 ↑/↓ 被 navigator 注册的
 * `navigateUp`/`navigateDown` 快捷键（`shortcuts.ts:583-609`，conditions 只排除
 * 输入框与对话框，不排除搜索结果列表）在 **document capture 阶段**吃掉，
 * `handleKeydown` 随即 `stopPropagation`（`shortcuts.ts:2029-2032`），于是
 * `global-search-result-list.vue` 挂在 listbox 上的 `handleListKeydown` 一次都不会跑；
 * 而 navigator 的 `callActivePaneNavigate` 把按键转发给背景导航格，导航格改树选中触发
 * `handleSelectionChange`，其 `entries.length > 0` 分支调用
 * `globalSearchViewRef.clearSelections()` 把搜索结果焦点清空 —— 操作员看到的判据是
 * 「第一次按 ↓，带 `global-search-result-list__row--focused` 的那一行消失、
 * focusedIndex 变 null、activeElement 落到背景 `file-browser-list-view__entry`」。
 *
 * 修法：方向键归属判断 + 显式转交给 GlobalSearchView（与批次 C1 的 Ctrl+A
 * `handleSelectAllShortcut` 同一姿势）。本文件钉住该形态不被改回。
 *
 * 为什么不是行为测试：私有桌面沙箱拿不到 OS 级键盘注入，方向键只能由 CDP
 * `Input.dispatchKeyEvent` 在真机取证里驱动（`Input.dispatchMouseEvent` 不合成 hover、
 * JS `.focus()` 不触发真实 focusout，见 AGENTS.md §2.8）；因此与
 * `navigator-escape-global-search.test.ts` 保持同一范式：源码文本断言 + 防回归。
 */
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';

// 与 navigator-escape-global-search.test.ts 同姿势：本项目的 vitest 环境里
// import.meta.url 不是 file: scheme，只能用 process.cwd() 解析仓库相对路径。
const navSource = readFileSync(resolve(process.cwd(), 'src/modules/navigator/pages/navigator.vue'), 'utf8');
const searchSource = readFileSync(resolve(process.cwd(), 'src/modules/global-search/global-search-view.vue'), 'utf8');

/** 从 `function <name>` 起到它自己的顶层 `}` 为止（列 0 的右括号 = 函数结束）。 */
function sliceFunction(source: string, name: string): string {
  const start = source.indexOf(`function ${name}`);
  if (start < 0) throw new Error(`missing function ${name}`);
  const end = source.indexOf('\n}', start);
  if (end < 0) throw new Error(`unterminated function ${name}`);
  return source.slice(start, end + 2);
}

/** 取 `defineExpose({ ... })` 的内容体。 */
function sliceExpose(source: string): string {
  const start = source.indexOf('defineExpose({');
  if (start < 0) throw new Error('missing defineExpose');
  const end = source.indexOf('})', start);
  if (end < 0) throw new Error('unterminated defineExpose');
  return source.slice(start, end);
}

describe('W-19: 方向键必须归搜索结果列表，而不是背景导航格', () => {
  const body = sliceFunction(navSource, 'callActivePaneNavigate');

  it('navigator 转发 ↑/↓ 前先问搜索结果列表是否拥有方向键', () => {
    expect(body).toMatch(/method === 'navigateUp' \|\| method === 'navigateDown'/);
    expect(body).toMatch(/ownsArrowNavigation\?\.\(\)/);
  });

  it('拥有时显式转交给 navigateResults，并直接返回（不得再落到背景导航格）', () => {
    expect(body).toMatch(/navigateResults\?\.\(/);
    // 归属判断与转交必须发生在取 pane ref 之前，否则导航格已经被动过。
    const ownership = body.search(/ownsArrowNavigation\?\.\(\)/);
    const paneRef = body.indexOf('const pane = getActivePaneRef()');
    expect(ownership).toBeGreaterThan(-1);
    expect(paneRef).toBeGreaterThan(-1);
    expect(ownership).toBeLessThan(paneRef);
  });

  it('归属守卫在 blocking layer 之后（对话框打开时仍优先让位）', () => {
    expect(body).toMatch(/hasBlockingDismissalLayersForNavigatorShortcuts\(\)/);
    expect(body).toMatch(/hasBlockingRekaDismissableLayersForNavigatorShortcuts\(\)/);
    expect(body.indexOf('hasBlockingDismissalLayersForNavigatorShortcuts()')).toBeLessThan(body.search(/ownsArrowNavigation\?\.\(\)/));
  });

  it('只截 navigateUp/navigateDown，不得顺手改掉左右键语义', () => {
    expect(body).not.toMatch(/method === 'navigateLeft' \|\| method === 'navigateRight'/);
  });

  it('结果列表为空时不劫持方向键（保留交还背景导航格的原行为）', () => {
    const owns = sliceFunction(searchSource, 'ownsArrowNavigation');
    expect(owns).toMatch(/allEntries\.value\.length > 0/);
    expect(owns).toMatch(/isPanelFocused\.value/);
    expect(owns).toMatch(/isEditableFocusTarget\(\)/);
  });
});

describe('W-19: GlobalSearchView 侧的归属与移动实现', () => {
  it('ownsArrowNavigation / navigateResults 都已 defineExpose（navigator 靠它们接线）', () => {
    const exposed = sliceExpose(searchSource);
    expect(exposed).toContain('ownsArrowNavigation');
    expect(exposed).toContain('navigateResults');
    // 既有契约不得丢
    expect(exposed).toContain('clearSelections');
    expect(exposed).toContain('isSearchPanelFocused');
    expect(exposed).toContain('selectAllResults');
  });

  it('navigateResults 在无焦点行时落到首行，末位 clamp 不越界', () => {
    const fn = sliceFunction(searchSource, 'navigateResults');
    expect(fn).toMatch(/entries\.length === 0/);
    expect(fn).toMatch(/currentIndex < 0 \? 0 : Math\.min\(currentIndex \+ 1, entries\.length - 1\)/);
    expect(fn).toMatch(/currentIndex < 0 \? 0 : Math\.max\(currentIndex - 1, 0\)/);
    expect(fn).toMatch(/entries\.length - 1/);
  });

  it('Shift 连选与裸移动都走同一个 navigateResults', () => {
    const fn = sliceFunction(searchSource, 'handleResultNavigate');
    expect(fn).toMatch(/navigateResults\(direction, extend\)/);
    const nav = sliceFunction(searchSource, 'navigateResults');
    expect(nav).toMatch(/if \(extend\) \{\s*extendSelectionTo\(nextIndex\);\s*\}\s*else \{\s*focusEntryAt\(nextIndex\);/);
  });

  it('焦点落在输入框时不算列表拥有方向键（↓ 仍走既有 focusFirstResult）', () => {
    const fn = sliceFunction(searchSource, 'isEditableFocusTarget');
    expect(fn).toMatch(/isContentEditable/);
    expect(fn).toMatch(/INPUT/);
    expect(fn).toMatch(/TEXTAREA/);
    expect(searchSource).toMatch(/@keydown\.down\.prevent="focusFirstResult"/);
  });
});

describe('W-19: 不得把方向键改回「无条件交给背景导航格」', () => {
  it('callActivePaneNavigate 里对 navigateUp/navigateDown 不存在无条件透传', () => {
    const body = sliceFunction(navSource, 'callActivePaneNavigate');
    // 防回归：不允许出现「方法名映射直接 return」这种绕过归属判断的写法
    expect(body).not.toMatch(/if \(method === 'navigateUp'\) return pane/);
    expect(body).not.toMatch(/return callActivePaneMethod\(method, event\);\s*\n\s*const pane/);
  });

  it('树选中清空搜索焦点仍只在树真正获得焦点时发生', () => {
    const fn = sliceFunction(navSource, 'handleSelectionChange');
    // 该清理本身是对的（用户确实把焦点交给了导航格），但它不得在方向键归属搜索面板时被触发：
    // 方向键路径已改走 navigateResults，不再调用 pane.navigateUp/navigateDown。
    expect(fn).toMatch(/globalSearchViewRef\.value\?\.clearSelections\?\.\(\)/);
    // 只断言「不存在真实调用」：本函数上方的说明注释里出现了 clearSelections() 字样，
    // 用裸 clearSelections 做 not.toMatch 会被自己的注释打红。
    expect(sliceFunction(navSource, 'callActivePaneNavigate')).not.toMatch(/clearSelections\?\.\(\)/);
  });
});
