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

import {
  formatShortcutKeys,
  useShortcutsStore,
} from '@/stores/runtime/shortcuts';

function getDefinition(shortcutsStore: ReturnType<typeof useShortcutsStore>, id: string) {
  const definition = shortcutsStore.definitions.find(item => item.id === id);

  if (!definition) {
    throw new Error(`Shortcut definition not found: ${id}`);
  }

  return definition;
}

describe('shortcut bindings bound to action registry ids', () => {
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

  it('reads a legacy binding stored under the plain shortcut id', () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      copy: {
        ctrl: true,
        key: 'c',
      },
    };

    const shortcutsStore = useShortcutsStore();
    const definition = getDefinition(shortcutsStore, 'copy');

    expect(shortcutsStore.getShortcutLabel('copy')).toBe('Ctrl+C');
    expect(shortcutsStore.getShortcutBindingSource(definition)).toBe('user');
    expect(shortcutsStore.getShortcutBindingFormat(definition)).toBe('legacy');
  });

  it('prefers the action registry id over the legacy shortcut id', () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      'sigma.shortcut.copy': {
        ctrl: true,
        key: 'v',
      },
      'copy': {
        ctrl: true,
        key: 'c',
      },
    };

    const shortcutsStore = useShortcutsStore();
    const definition = getDefinition(shortcutsStore, 'copy');

    expect(shortcutsStore.getShortcutLabel('copy')).toBe('Ctrl+V');
    expect(shortcutsStore.getShortcutBindingFormat(definition)).toBe('action-id');
  });

  it('writes a new binding under the action registry id and drops the legacy key', async () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      copy: {
        ctrl: true,
        key: 'c',
      },
    };

    const shortcutsStore = useShortcutsStore();

    await shortcutsStore.setShortcut('copy', {
      ctrl: true,
      key: 'v',
    });

    expect(userSettingsStoreMock.userSettings.shortcuts).toEqual({
      'sigma.shortcut.copy': {
        ctrl: true,
        key: 'v',
      },
    });
    expect(userSettingsStoreMock.setUserSettingsStorage).toHaveBeenCalledWith('shortcuts', {
      'sigma.shortcut.copy': {
        ctrl: true,
        key: 'v',
      },
    });
  });

  it('keeps the multi-slot array shape when binding under the action registry id', async () => {
    const shortcutsStore = useShortcutsStore();

    await shortcutsStore.setShortcut('navigateHistoryBack', {
      ctrl: true,
      key: 'h',
    });

    expect(userSettingsStoreMock.userSettings.shortcuts).toEqual({
      'sigma.shortcut.navigateHistoryBack': [{
        ctrl: true,
        key: 'h',
      }, null],
    });
    expect(shortcutsStore.getShortcutLabel('navigateHistoryBack')).toBe('Ctrl+H');
  });

  it('resets a legacy binding back to the default key and clears both key formats', async () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      copy: {
        ctrl: true,
        key: 'c',
      },
    };

    const shortcutsStore = useShortcutsStore();
    const definition = getDefinition(shortcutsStore, 'copy');

    await shortcutsStore.resetShortcut('copy');

    expect(shortcutsStore.getShortcutLabel('copy')).toBe(formatShortcutKeys(definition.defaultKeys));
    expect(shortcutsStore.getShortcutBindingSource(definition)).toBe('system');
    expect(userSettingsStoreMock.userSettings.shortcuts).toEqual({});
  });

  it('does not resurrect a legacy binding after the action registry binding was reset', async () => {
    userSettingsStoreMock.userSettings.shortcuts = {
      copy: {
        alt: true,
        key: 'j',
      },
    };

    const shortcutsStore = useShortcutsStore();
    const definition = getDefinition(shortcutsStore, 'copy');
    const defaultLabel = formatShortcutKeys(definition.defaultKeys);

    await shortcutsStore.setShortcut('copy', {
      ctrl: true,
      key: 'v',
    });
    expect(shortcutsStore.getShortcutLabel('copy')).toBe('Ctrl+V');

    await shortcutsStore.resetShortcut('copy');

    expect(shortcutsStore.getShortcutLabel('copy')).toBe(defaultLabel);
    expect(shortcutsStore.getShortcutLabel('copy')).not.toBe('Alt+J');
    expect(userSettingsStoreMock.userSettings.shortcuts).toEqual({});
  });

  it('detects a conflict against a binding stored under the action registry id', () => {
    const probe = {
      ctrl: true,
      alt: true,
      key: 'q',
    };

    userSettingsStoreMock.userSettings.shortcuts = {};
    const shortcutsStore = useShortcutsStore();

    expect(shortcutsStore.findConflictingShortcut(probe)).toBeNull();

    userSettingsStoreMock.userSettings.shortcuts = {
      'sigma.shortcut.copy': probe,
    };

    expect(shortcutsStore.findConflictingShortcut(probe)?.id).toBe('copy');
  });

  it('exposes the action registry id of a shortcut', () => {
    const shortcutsStore = useShortcutsStore();

    expect(shortcutsStore.getShortcutActionId('copy')).toBe('sigma.shortcut.copy');
    expect(shortcutsStore.shortcutActionRegistry.has('sigma.shortcut.copy')).toBe(true);
  });
});
