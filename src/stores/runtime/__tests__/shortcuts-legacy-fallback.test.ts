// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { reactive } from 'vue';

const {
  executeCommandMock,
  extensionKeybindings,
  routerCurrentRouteMock,
  setAppKeybindingConflictCheckerMock,
  userSettingsStoreMock,
} = vi.hoisted(() => ({
  extensionKeybindings: [] as unknown[],
  executeCommandMock: vi.fn(),
  setAppKeybindingConflictCheckerMock: vi.fn(),
  userSettingsStoreMock: {
    userSettings: {
      shortcuts: {},
      shortcutUserAlternateChordSlots: {},
    },
    setUserSettingsStorage: vi.fn(),
  },
  routerCurrentRouteMock: {
    value: { name: 'navigator' as string | symbol | null | undefined },
  },
}));

/**
 * 迁移失败时，store 必须回落旧逻辑：既有绑定不能失效。
 */
vi.mock('@/core/actions/shortcut-bindings', async (importOriginal) => {
  const actual = await importOriginal<typeof import('@/core/actions/shortcut-bindings')>();

  return {
    ...actual,
    migrateShortcutBindings: () => {
      throw new Error('migration failed');
    },
  };
});

vi.mock('@/router', () => ({
  default: {
    currentRoute: routerCurrentRouteMock,
  },
}));

vi.mock('@/stores/storage/user-settings', () => ({
  useUserSettingsStore: () => userSettingsStoreMock,
}));

vi.mock('@/stores/runtime/extensions', () => ({
  useExtensionsStore: () => ({
    enabledExtensions: [],
    keybindings: extensionKeybindings,
    executeCommand: executeCommandMock,
  }),
}));

vi.mock('@/modules/extensions/api', () => ({
  setAppKeybindingConflictChecker: setAppKeybindingConflictCheckerMock,
}));

vi.mock('@/modules/extensions/context', () => ({
  getSelectedEntries: () => [],
  getCurrentPath: () => '',
}));

import { useShortcutsStore } from '@/stores/runtime/shortcuts';

describe('shortcut bindings when the action id migration fails', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    routerCurrentRouteMock.value = { name: 'navigator' };
    extensionKeybindings.splice(0, extensionKeybindings.length);
    userSettingsStoreMock.userSettings = reactive({
      shortcuts: {},
      shortcutUserAlternateChordSlots: {},
    });
    userSettingsStoreMock.setUserSettingsStorage.mockReset();
    setAppKeybindingConflictCheckerMock.mockReset();
    executeCommandMock.mockReset();
  });

  it('falls back to the legacy shortcut id', () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      copy: {
        ctrl: true,
        key: 'c',
      },
    };

    const shortcutsStore = useShortcutsStore();

    expect(shortcutsStore.getShortcutLabel('copy')).toBe('Ctrl+C');
  });

  it('still resolves a binding stored under the action registry id', () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      'sigma.shortcut.copy': {
        ctrl: true,
        key: 'v',
      },
    };

    const shortcutsStore = useShortcutsStore();

    expect(shortcutsStore.getShortcutLabel('copy')).toBe('Ctrl+V');
  });

  it('falls back to the default key when no binding is stored at all', () => {
    const shortcutsStore = useShortcutsStore();
    const definition = shortcutsStore.definitions.find(item => item.id === 'copy');

    expect(definition).toBeDefined();
    expect(shortcutsStore.getShortcutLabel('copy')).not.toBe('');
  });
});
