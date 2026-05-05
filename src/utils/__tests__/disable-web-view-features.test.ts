// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { beforeEach, describe, expect, it } from 'vitest';
import { disableWebViewFeatures } from '@/utils/disable-web-view-features';

describe('disableWebViewFeatures', () => {
  beforeEach(() => {
    document.body.innerHTML = '';
  });

  it('prevents native Alt+arrow history navigation', () => {
    disableWebViewFeatures();

    const backEvent = new KeyboardEvent('keydown', {
      key: 'ArrowLeft',
      altKey: true,
      bubbles: true,
      cancelable: true,
    });
    const forwardEvent = new KeyboardEvent('keydown', {
      key: 'ArrowRight',
      altKey: true,
      bubbles: true,
      cancelable: true,
    });
    const upEvent = new KeyboardEvent('keydown', {
      key: 'ArrowUp',
      altKey: true,
      bubbles: true,
      cancelable: true,
    });

    document.dispatchEvent(backEvent);
    document.dispatchEvent(forwardEvent);
    document.dispatchEvent(upEvent);

    expect(backEvent.defaultPrevented).toBe(true);
    expect(forwardEvent.defaultPrevented).toBe(true);
    expect(upEvent.defaultPrevented).toBe(true);
  });

  it('does not prevent native Alt+arrow behavior in editable fields', () => {
    disableWebViewFeatures();

    document.body.innerHTML = '<input id="editable-input" />';
    const editableInput = document.getElementById('editable-input');

    if (!(editableInput instanceof HTMLInputElement)) {
      throw new Error('Expected editable input to be rendered');
    }

    const event = new KeyboardEvent('keydown', {
      key: 'ArrowLeft',
      altKey: true,
      bubbles: true,
      cancelable: true,
    });

    editableInput.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
  });

  it('does not prevent native Alt+arrow behavior while dialogs are open', () => {
    disableWebViewFeatures();

    document.body.innerHTML = '<div role="dialog"></div>';

    const event = new KeyboardEvent('keydown', {
      key: 'ArrowRight',
      altKey: true,
      bubbles: true,
      cancelable: true,
    });

    document.dispatchEvent(event);

    expect(event.defaultPrevented).toBe(false);
  });

  it('prevents native mouse button history navigation', () => {
    disableWebViewFeatures();

    const mouseDownEvent = new MouseEvent('mousedown', {
      button: 3,
      bubbles: true,
      cancelable: true,
    });
    const mouseUpEvent = new MouseEvent('mouseup', {
      button: 3,
      bubbles: true,
      cancelable: true,
    });
    const auxClickEvent = new MouseEvent('auxclick', {
      button: 4,
      bubbles: true,
      cancelable: true,
    });

    document.dispatchEvent(mouseDownEvent);
    document.dispatchEvent(mouseUpEvent);
    document.dispatchEvent(auxClickEvent);

    expect(mouseDownEvent.defaultPrevented).toBe(true);
    expect(mouseUpEvent.defaultPrevented).toBe(true);
    expect(auxClickEvent.defaultPrevented).toBe(true);
  });
});
