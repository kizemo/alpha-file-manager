// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION 2026-10-27：永久删除确认框的「回车 = 删除」判定。
 *
 * 背景（2026-10-27 真机实测修正）：reka-ui 的 FocusScope 在对话框挂载时
 * 会把焦点落到**内容里第一个可聚焦控件**上。本对话框里那个控件是「取消」按钮
 * （X 关闭按钮渲染在插槽之后，排在「取消 / 删除」后面），于是「刚打开就回车」
 * 走的是浏览器原生的按钮激活 —— 结果是**取消**，而不是用户要的删除。
 *
 * 因此对话框打开时把焦点挪到对话框容器上（容器不是可交互控件），
 * 裸回车就会落到下面的兜底分支「焦点不在控件上 ⇒ 确认删除」；
 * 用户 Tab 之后焦点在按钮上时，仍然交给浏览器原生行为。
 *
 * 本模块只做纯判定与一处 DOM 聚焦，不碰事件绑定，方便单测。
 */

/** 焦点在这些元素上时，回车交给浏览器原生行为处理。 */
export const PERMANENT_DELETE_CONFIRM_INTERACTIVE_SELECTOR
  = 'button, a[href], input, textarea, select, [role="button"], [role="menuitem"]';

/** 永久删除确认框的对话框容器（焦点落点）。 */
export const PERMANENT_DELETE_CONFIRM_CONTENT_SELECTOR = '.permanent-delete-confirm-dialog';

export interface PermanentDeleteConfirmEnterContext {
  key: string;
  defaultPrevented: boolean;
  isComposing: boolean;
  /** 焦点当前是否落在可交互控件（按钮 / 链接 / 输入框等）上。 */
  isFocusOnInteractiveControl: boolean;
}

/** 目标元素是否属于「可交互控件」。非浏览器环境下一律当作「不是控件」。 */
export function isPermanentDeleteConfirmInteractiveTarget(target: EventTarget | null): boolean {
  if (!target || typeof Element === 'undefined' || !(target instanceof Element)) {
    return false;
  }

  return target.closest(PERMANENT_DELETE_CONFIRM_INTERACTIVE_SELECTOR) !== null;
}

/** 这次回车是否应该触发「确认删除」。 */
export function shouldConfirmPermanentDeleteOnEnter(
  context: PermanentDeleteConfirmEnterContext,
): boolean {
  if (context.key !== 'Enter') {
    return false;
  }

  if (context.defaultPrevented || context.isComposing) {
    return false;
  }

  return !context.isFocusOnInteractiveControl;
}

/**
 * 把焦点落到永久删除确认框的对话框容器上。
 *
 * 容器不是可交互控件，焦点停在它上面时，裸回车走
 * 「焦点不在控件上 ⇒ 确认删除」的兜底分支，而不是触发「取消」。
 * 返回焦点是否真的落在了容器上（false 表示没找到容器或聚焦失败）。
 */
export function focusPermanentDeleteConfirmContent(
  root: ParentNode | null = typeof document !== 'undefined' ? document : null,
): boolean {
  if (!root || typeof HTMLElement === 'undefined') {
    return false;
  }

  const content = root.querySelector<HTMLElement>(PERMANENT_DELETE_CONFIRM_CONTENT_SELECTOR);

  if (!content) {
    return false;
  }

  // 容器默认不可聚焦，focus() 是空操作 —— 补一个 tabindex="-1"。
  if (!content.hasAttribute('tabindex')) {
    content.setAttribute('tabindex', '-1');
  }

  content.focus();

  return typeof document !== 'undefined' && document.activeElement === content;
}
