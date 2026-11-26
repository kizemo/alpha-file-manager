// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import type { ActionContext } from '@/core/actions/types';
import {
  buildPaletteActionRegistry,
  getPaletteActionEntries,
} from '@/core/actions/palette-adapter';
import { whenRouteIs } from '@/core/actions/predicates';

function createContext(overrides: Partial<ActionContext> = {}): ActionContext {
  return {
    isInputFieldActive: false,
    isDialogOpened: false,
    activeRouteName: null,
    ...overrides,
  };
}

describe('buildPaletteActionRegistry', () => {
  it('registers extension actions before builtin actions', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [
        {
          extensionId: 'kizemo.focus-sync',
          id: 'kizemo.focus-sync.start',
          title: 'Start focus session',
        },
      ],
      builtinActions: [
        {
          extensionId: 'sigma',
          id: 'sigma.app.openSettings',
          title: 'Open settings',
          showInPalette: true,
        },
      ],
    });

    expect(registry.getAll().map(action => action.id)).toEqual([
      'kizemo.focus-sync.start',
      'sigma.app.openSettings',
    ]);
  });

  it('skips builtin actions that are not exposed in the palette', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [],
      builtinActions: [
        {
          extensionId: 'sigma',
          id: 'sigma.app.openSettings',
          title: 'Open settings',
          showInPalette: true,
        },
        {
          extensionId: 'sigma',
          id: 'sigma.app.openExtensions',
          title: 'Open extensions',
        },
      ],
    });

    expect(registry.getAll().map(action => action.id)).toEqual(['sigma.app.openSettings']);
  });

  it('keeps the extension action when a builtin action reuses the same id', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [
        {
          extensionId: 'sigma',
          id: 'sigma.app.openSettings',
          title: 'Runtime override',
        },
      ],
      builtinActions: [
        {
          extensionId: 'sigma',
          id: 'sigma.app.openSettings',
          title: 'Builtin declaration',
          showInPalette: true,
        },
      ],
    });

    expect(registry.size()).toBe(1);
    expect(registry.get('sigma.app.openSettings')?.title).toBe('Runtime override');
    expect(registry.get('sigma.app.openSettings')?.source).toBe('extension');
  });

  it('records the builtin source for builtin actions', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [],
      builtinActions: [
        {
          extensionId: 'sigma',
          id: 'sigma.app.openSettings',
          title: 'Open settings',
          showInPalette: true,
        },
      ],
    });

    expect(registry.get('sigma.app.openSettings')?.source).toBe('builtin');
  });
});

describe('getPaletteActionEntries', () => {
  it('maps every registered action to the palette entry shape', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [
        {
          extensionId: 'kizemo.focus-sync',
          id: 'kizemo.focus-sync.start',
          title: 'Start focus session',
        },
      ],
      builtinActions: [],
    });

    expect(getPaletteActionEntries(registry, createContext())).toEqual([
      {
        extensionId: 'kizemo.focus-sync',
        command: {
          id: 'kizemo.focus-sync.start',
          title: 'Start focus session',
        },
      },
    ]);
  });

  it('attaches the translation key declared for a builtin action', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [],
      builtinActions: [
        {
          extensionId: 'sigma',
          id: 'sigma.app.openSettings',
          title: 'Open settings',
          showInPalette: true,
        },
      ],
      titleKeys: {
        'sigma.app.openSettings': 'openSettings',
      },
    });

    expect(getPaletteActionEntries(registry, createContext())[0].titleKey).toBe('openSettings');
  });

  it('omits actions that are unavailable in the current context', () => {
    const registry = buildPaletteActionRegistry({
      extensionActions: [
        {
          extensionId: 'kizemo.focus-sync',
          id: 'kizemo.focus-sync.start',
          title: 'Start focus session',
          when: whenRouteIs('navigator'),
        },
      ],
      builtinActions: [],
    });

    expect(getPaletteActionEntries(registry, createContext({ activeRouteName: 'settings' }))).toEqual([]);
    expect(getPaletteActionEntries(registry, createContext({ activeRouteName: 'navigator' }))).toHaveLength(1);
  });
});
