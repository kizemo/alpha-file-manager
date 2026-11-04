// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  buildShortcutActionId,
  buildShortcutActionRegistry,
  isShortcutActionId,
  parseShortcutActionId,
  SHORTCUT_ACTION_ID_PREFIX,
  SHORTCUT_ACTION_SOURCE_ID,
} from '@/core/actions/shortcut-actions';

describe('shortcut action ids', () => {
  it('namespaces shortcut ids inside the action registry', () => {
    expect(SHORTCUT_ACTION_ID_PREFIX).toBe('sigma.shortcut.');
    expect(buildShortcutActionId('copy')).toBe('sigma.shortcut.copy');
  });

  it('parses a shortcut action id back into its shortcut id', () => {
    expect(parseShortcutActionId('sigma.shortcut.copy')).toBe('copy');
  });

  it('refuses to parse ids that are not shortcut actions', () => {
    expect(parseShortcutActionId('sigma.app.openSettings')).toBeNull();
    expect(parseShortcutActionId('sigma.shortcut.')).toBeNull();
    expect(parseShortcutActionId('copy')).toBeNull();
  });

  it('recognizes shortcut action ids', () => {
    expect(isShortcutActionId('sigma.shortcut.copy')).toBe(true);
    expect(isShortcutActionId('sigma.app.openSettings')).toBe(false);
  });
});

describe('shortcut action registry', () => {
  it('registers every shortcut as a builtin action', () => {
    const registry = buildShortcutActionRegistry([
      {
        id: 'copy',
        labelKey: 'shortcuts.copy',
      },
      {
        id: 'paste',
        labelKey: 'shortcuts.paste',
      },
    ]);

    expect(registry.size()).toBe(2);
    expect(registry.has('sigma.shortcut.copy')).toBe(true);

    const definition = registry.get('sigma.shortcut.copy');

    expect(definition?.source).toBe('builtin');
    expect(definition?.extensionId).toBe(SHORTCUT_ACTION_SOURCE_ID);
    expect(definition?.titleKey).toBe('shortcuts.copy');
  });

  it('keeps the first registration when the same shortcut id is declared twice', () => {
    const registry = buildShortcutActionRegistry([
      {
        id: 'copy',
        labelKey: 'shortcuts.copy',
      },
      {
        id: 'copy',
        labelKey: 'shortcuts.copySomethingElse',
      },
    ]);

    expect(registry.size()).toBe(1);
    expect(registry.get('sigma.shortcut.copy')?.titleKey).toBe('shortcuts.copy');
  });
});
