// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION 2026-10-27：永久删除确认框的「回车 = 删除」判定。
 *
 * 背景：reka-ui 的 DialogContent 打开时把焦点落在对话框容器本身，
 * 两个按钮（取消 / 删除）都没被激活 ⇒ 裸回车什么都不发生。
 * 用户要求「回车默认映射的是删除」。
 *
 * 本模块只做纯判定，不碰 DOM 事件绑定，方便单测。
 */

/** 焦点在这些元素上时，回车交给浏览器原生行为处理。 */
export const PERMANENT_DELETE_CONFIRM_INTERACTIVE_SELECTOR
  = 'button, a[href], input, textarea, select, [role="button"], [role="menuitem"]';

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
