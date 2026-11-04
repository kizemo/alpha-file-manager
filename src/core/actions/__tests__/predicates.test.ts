// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { afterEach, describe, expect, it } from 'vitest';
import type { ActionContext } from '@/core/actions/types';
import {
  alwaysAvailable,
  combineWhen,
  getActionContext,
  whenNoDialogOpen,
  whenNotEditingText,
  whenRouteIs,
} from '@/core/actions/predicates';

function createContext(overrides: Partial<ActionContext> = {}): ActionContext {
  return {
    isInputFieldActive: false,
    isDialogOpened: false,
    activeRouteName: null,
    ...overrides,
  };
}

describe('action context predicates', () => {
  it('treats alwaysAvailable as available even while a dialog is open and text is focused', () => {
    const predicate = alwaysAvailable();

    expect(predicate(createContext({
      isDialogOpened: true,
      isInputFieldActive: true,
    }))).toBe(true);
  });

  it('blocks actions while a dialog is open', () => {
    const predicate = whenNoDialogOpen();

    expect(predicate(createContext({ isDialogOpened: false }))).toBe(true);
    expect(predicate(createContext({ isDialogOpened: true }))).toBe(false);
  });

  it('blocks actions while a text field is focused', () => {
    const predicate = whenNotEditingText();

    expect(predicate(createContext({ isInputFieldActive: false }))).toBe(true);
    expect(predicate(createContext({ isInputFieldActive: true }))).toBe(false);
  });

  it('scopes actions to a single route', () => {
    const predicate = whenRouteIs('navigator');

    expect(predicate(createContext({ activeRouteName: 'navigator' }))).toBe(true);
    expect(predicate(createContext({ activeRouteName: 'settings' }))).toBe(false);
    expect(predicate(createContext({ activeRouteName: null }))).toBe(false);
  });

  it('combines predicates with logical and', () => {
    const predicate = combineWhen(whenNoDialogOpen(), whenNotEditingText());

    expect(predicate(createContext())).toBe(true);
    expect(predicate(createContext({ isDialogOpened: true }))).toBe(false);
    expect(predicate(createContext({ isInputFieldActive: true }))).toBe(false);
  });

  it('treats an empty combination as always available', () => {
    expect(combineWhen()(createContext({ isDialogOpened: true }))).toBe(true);
  });
});

describe('getActionContext', () => {
  afterEach(() => {
    document.body.innerHTML = '';
  });

  it('reports an idle context by default', () => {
    expect(getActionContext()).toEqual({
      isInputFieldActive: false,
      isDialogOpened: false,
      activeRouteName: null,
    });
  });

  it('detects a focused text input', () => {
    const input = document.createElement('input');
    document.body.appendChild(input);
    input.focus();

    expect(getActionContext().isInputFieldActive).toBe(true);
  });

  it('detects a focused textarea', () => {
    const textarea = document.createElement('textarea');
    document.body.appendChild(textarea);
    textarea.focus();

    expect(getActionContext().isInputFieldActive).toBe(true);
  });

  it('ignores a focused element that is not editable', () => {
    const container = document.createElement('div');
    document.body.appendChild(container);
    container.focus();

    expect(getActionContext().isInputFieldActive).toBe(false);
  });

  it('detects an open dialog', () => {
    const dialog = document.createElement('div');
    dialog.setAttribute('role', 'dialog');
    document.body.appendChild(dialog);

    expect(getActionContext().isDialogOpened).toBe(true);
  });

  it('ignores popover content that carries the dialog role', () => {
    const popover = document.createElement('div');
    popover.setAttribute('role', 'dialog');
    popover.classList.add('sigma-ui-popover-content');
    document.body.appendChild(popover);

    expect(getActionContext().isDialogOpened).toBe(false);
  });

  it('passes the active route name through', () => {
    expect(getActionContext('navigator').activeRouteName).toBe('navigator');
  });
});
