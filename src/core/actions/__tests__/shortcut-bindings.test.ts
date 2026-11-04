// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  isShortcutBindingValue,
  lookupShortcutBinding,
  migrateShortcutBindings,
  pickShortcutBindingSlot,
} from '@/core/actions/shortcut-bindings';

const KNOWN_SHORTCUT_IDS = new Set(['copy', 'paste']);

function isKnownShortcutId(shortcutId: string): boolean {
  return KNOWN_SHORTCUT_IDS.has(shortcutId);
}

describe('shortcut binding values', () => {
  it('accepts a single key combination and a slot array', () => {
    expect(isShortcutBindingValue({
      ctrl: true,
      key: 'c',
    })).toBe(true);
    expect(isShortcutBindingValue([{
      ctrl: true,
      key: 'c',
    }, null])).toBe(true);
    expect(isShortcutBindingValue([null, null])).toBe(true);
  });

  it('rejects malformed bindings', () => {
    expect(isShortcutBindingValue({
      ctrl: 'yes',
      key: 'c',
    })).toBe(false);
    expect(isShortcutBindingValue({ ctrl: true })).toBe(false);
    expect(isShortcutBindingValue('ctrl+c')).toBe(false);
    expect(isShortcutBindingValue(null)).toBe(false);
    expect(isShortcutBindingValue([{ ctrl: true }, null])).toBe(false);
  });
});

describe('shortcut binding lookup', () => {
  it('prefers the action registry id over the legacy shortcut id', () => {
    const lookup = lookupShortcutBinding(
      {
        'sigma.shortcut.copy': {
          ctrl: true,
          key: 'v',
        },
        'copy': {
          ctrl: true,
          key: 'c',
        },
      },
      'sigma.shortcut.copy',
      'copy',
    );

    expect(lookup.format).toBe('action-id');
    expect(lookup.entry).toEqual({
      ctrl: true,
      key: 'v',
    });
  });

  it('falls back to the legacy shortcut id', () => {
    const lookup = lookupShortcutBinding(
      {
        copy: {
          ctrl: true,
          key: 'c',
        },
      },
      'sigma.shortcut.copy',
      'copy',
    );

    expect(lookup.format).toBe('legacy');
    expect(lookup.entry).toEqual({
      ctrl: true,
      key: 'c',
    });
  });

  it('reports no binding when neither key is present', () => {
    const lookup = lookupShortcutBinding({}, 'sigma.shortcut.copy', 'copy');

    expect(lookup.entry).toBeUndefined();
    expect(lookup.format).toBeNull();
  });

  it('ignores malformed values instead of binding them', () => {
    const lookup = lookupShortcutBinding({ copy: 'ctrl+c' }, 'sigma.shortcut.copy', 'copy');

    expect(lookup.entry).toBeUndefined();
    expect(lookup.format).toBeNull();
  });
});

describe('shortcut binding slots', () => {
  it('reads slot 0 from a single key combination', () => {
    expect(pickShortcutBindingSlot({
      ctrl: true,
      key: 'c',
    }, 0)).toEqual({
      ctrl: true,
      key: 'c',
    });
    expect(pickShortcutBindingSlot({
      ctrl: true,
      key: 'c',
    }, 1)).toBeNull();
  });

  it('reads each slot of a slot array', () => {
    const entry = [{
      ctrl: true,
      key: 'c',
    }, null, {
      alt: true,
      key: 'p',
    }];

    expect(pickShortcutBindingSlot(entry, 0)).toEqual({
      ctrl: true,
      key: 'c',
    });
    expect(pickShortcutBindingSlot(entry, 1)).toBeNull();
    expect(pickShortcutBindingSlot(entry, 2)).toEqual({
      alt: true,
      key: 'p',
    });
    expect(pickShortcutBindingSlot(entry, 3)).toBeNull();
  });
});

describe('shortcut binding migration', () => {
  it('maps legacy shortcut ids onto action registry ids', () => {
    const migration = migrateShortcutBindings(
      {
        copy: {
          ctrl: true,
          key: 'c',
        },
      },
      isKnownShortcutId,
    );

    expect(migration.bindings).toEqual({
      'sigma.shortcut.copy': {
        ctrl: true,
        key: 'c',
      },
    });
    expect(migration.formats['sigma.shortcut.copy']).toBe('legacy');
    expect(migration.unmappedLegacyKeys).toEqual([]);
    expect(migration.invalidKeys).toEqual([]);
  });

  it('keeps action registry ids untouched', () => {
    const migration = migrateShortcutBindings(
      {
        'sigma.shortcut.copy': {
          ctrl: true,
          key: 'v',
        },
      },
      isKnownShortcutId,
    );

    expect(migration.bindings).toEqual({
      'sigma.shortcut.copy': {
        ctrl: true,
        key: 'v',
      },
    });
    expect(migration.formats['sigma.shortcut.copy']).toBe('action-id');
  });

  it('lets the action registry id win over the legacy shortcut id', () => {
    const migration = migrateShortcutBindings(
      {
        'sigma.shortcut.copy': {
          ctrl: true,
          key: 'v',
        },
        'copy': {
          ctrl: true,
          key: 'c',
        },
      },
      isKnownShortcutId,
    );

    expect(migration.bindings['sigma.shortcut.copy']).toEqual({
      ctrl: true,
      key: 'v',
    });
    expect(migration.formats['sigma.shortcut.copy']).toBe('action-id');
  });

  it('keeps unknown legacy keys aside instead of dropping user data', () => {
    const migration = migrateShortcutBindings(
      {
        retiredShortcut: {
          ctrl: true,
          key: 'c',
        },
      },
      isKnownShortcutId,
    );

    expect(migration.bindings).toEqual({});
    expect(migration.unmappedLegacyKeys).toEqual(['retiredShortcut']);
  });

  it('reports malformed values as invalid bindings', () => {
    const migration = migrateShortcutBindings(
      {
        'copy': 'ctrl+c',
        'sigma.shortcut.paste': 42,
      },
      isKnownShortcutId,
    );

    expect(migration.bindings).toEqual({});
    expect([...migration.invalidKeys].sort()).toEqual(['copy', 'sigma.shortcut.paste']);
  });

  it('handles an empty binding table', () => {
    const migration = migrateShortcutBindings(undefined, isKnownShortcutId);

    expect(migration.bindings).toEqual({});
    expect(migration.unmappedLegacyKeys).toEqual([]);
  });
});
