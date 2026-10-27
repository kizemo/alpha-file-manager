// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { afterEach, describe, expect, it } from 'vitest';
import {
  focusPermanentDeleteConfirmContent,
  isPermanentDeleteConfirmInteractiveTarget,
  shouldConfirmPermanentDeleteOnEnter,
} from '../permanent-delete-confirm-keyboard';

describe('shouldConfirmPermanentDeleteOnEnter', () => {
  it('焦点不在可交互控件上时，回车判定为确认删除', () => {
    expect(shouldConfirmPermanentDeleteOnEnter({
      key: 'Enter',
      defaultPrevented: false,
      isComposing: false,
      isFocusOnInteractiveControl: false,
    })).toBe(true);
  });

  it('焦点在按钮上时，回车交给浏览器原生行为处理', () => {
    expect(shouldConfirmPermanentDeleteOnEnter({
      key: 'Enter',
      defaultPrevented: false,
      isComposing: false,
      isFocusOnInteractiveControl: true,
    })).toBe(false);
  });

  it('事件已被处理过（defaultPrevented）时不响应', () => {
    expect(shouldConfirmPermanentDeleteOnEnter({
      key: 'Enter',
      defaultPrevented: true,
      isComposing: false,
      isFocusOnInteractiveControl: false,
    })).toBe(false);
  });

  it('输入法组合过程中（isComposing）不响应', () => {
    expect(shouldConfirmPermanentDeleteOnEnter({
      key: 'Enter',
      defaultPrevented: false,
      isComposing: true,
      isFocusOnInteractiveControl: false,
    })).toBe(false);
  });

  it('其它按键一律不响应', () => {
    for (const key of ['Escape', ' ', 'a', 'Tab']) {
      expect(shouldConfirmPermanentDeleteOnEnter({
        key,
        defaultPrevented: false,
        isComposing: false,
        isFocusOnInteractiveControl: false,
      })).toBe(false);
    }
  });
});

describe('isPermanentDeleteConfirmInteractiveTarget', () => {
  it('按钮元素算作可交互控件', () => {
    const button = document.createElement('button');

    expect(isPermanentDeleteConfirmInteractiveTarget(button)).toBe(true);
  });

  it('按钮的子元素同样算作可交互控件', () => {
    const wrapper = document.createElement('div');
    const button = document.createElement('button');
    const span = document.createElement('span');

    button.appendChild(span);
    wrapper.appendChild(button);

    expect(isPermanentDeleteConfirmInteractiveTarget(span)).toBe(true);
  });

  it('普通容器元素不算作可交互控件', () => {
    const div = document.createElement('div');

    expect(isPermanentDeleteConfirmInteractiveTarget(div)).toBe(false);
  });

  it('空目标不算作可交互控件', () => {
    expect(isPermanentDeleteConfirmInteractiveTarget(null)).toBe(false);
  });
});

describe('focusPermanentDeleteConfirmContent', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  function makeContent(): HTMLElement {
    const content = document.createElement('div');
    content.className = 'permanent-delete-confirm-dialog';
    const cancel = document.createElement('button');
    const confirm = document.createElement('button');
    content.appendChild(cancel);
    content.appendChild(confirm);
    document.body.appendChild(content);

    return content;
  }

  it('把焦点落到对话框容器上', () => {
    const content = makeContent();

    expect(focusPermanentDeleteConfirmContent(document)).toBe(true);
    expect(document.activeElement).toBe(content);
  });

  it('容器不是可交互控件 —— 裸回车会走「确认删除」分支', () => {
    const content = makeContent();
    focusPermanentDeleteConfirmContent(document);

    expect(isPermanentDeleteConfirmInteractiveTarget(content)).toBe(false);
    expect(shouldConfirmPermanentDeleteOnEnter({
      key: 'Enter',
      defaultPrevented: false,
      isComposing: false,
      isFocusOnInteractiveControl: isPermanentDeleteConfirmInteractiveTarget(document.activeElement),
    })).toBe(true);
  });

  it('找不到容器时返回 false，不抛异常', () => {
    expect(focusPermanentDeleteConfirmContent(document)).toBe(false);
  });

  it('没有 root 时返回 false，不抛异常', () => {
    expect(focusPermanentDeleteConfirmContent(null)).toBe(false);
  });
});
