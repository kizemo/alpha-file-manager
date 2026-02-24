// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import { register, unregister, unregisterAll } from '@tauri-apps/plugin-global-shortcut';
import { WebviewWindow } from '@tauri-apps/api/webviewWindow';
import { invoke } from '@tauri-apps/api/core';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import type { GlobalShortcutId, UserGlobalShortcuts, ShortcutKeys } from '@/types/user-settings';

export type { GlobalShortcutId, UserGlobalShortcuts };

export type GlobalShortcutDefinition = {
  id: GlobalShortcutId;
  labelKey: string;
  defaultShortcut: string;
};

const DEFAULT_GLOBAL_SHORTCUTS: GlobalShortcutDefinition[] = [
  {
    id: 'launchApp',
    labelKey: 'shortcuts.focusAppWindow',
    defaultShortcut: 'Super+Shift+E',
  },
];

export function shortcutKeysToTauriFormat(keys: ShortcutKeys): string {
  const parts: string[] = [];
  if (keys.ctrl) parts.push('Control');
  if (keys.alt) parts.push('Alt');
  if (keys.meta) parts.push('Super');
  if (keys.shift) parts.push('Shift');

  let keyName = keys.key;
  if (keyName === ' ') keyName = 'Space';
  else if (keyName.length === 1) keyName = keyName.toUpperCase();

  parts.push(keyName);
  return parts.join('+');
}

export function tauriFormatToShortcutKeys(shortcut: string): ShortcutKeys {
  const parts = shortcut.split('+');
  const keys: ShortcutKeys = { key: '' };

  for (const part of parts) {
    if (part === 'Control' || part === 'CommandOrControl') keys.ctrl = true;
    else if (part === 'Alt') keys.alt = true;
    else if (part === 'Shift') keys.shift = true;
    else if (part === 'Super') keys.meta = true;
    else keys.key = part === 'Space' ? ' ' : part;
  }

  return keys;
}

export function formatTauriShortcut(shortcut: string): string {
  return shortcut
    .replace('Super', 'Win')
    .replace('Windows', 'Win')
    .replace('CommandOrControl', 'Ctrl')
    .replace('Control', 'Ctrl');
}

function normalizeShortcutString(shortcut: string): string {
  const keys = tauriFormatToShortcutKeys(shortcut);
  return shortcutKeysToTauriFormat(keys);
}

export const useGlobalShortcutsStore = defineStore('globalShortcuts', () => {
  const userSettingsStore = useUserSettingsStore();

  const definitions = ref<GlobalShortcutDefinition[]>(DEFAULT_GLOBAL_SHORTCUTS);
  const registeredShortcuts = ref<Map<GlobalShortcutId, string>>(new Map());
  const isInitialized = ref(false);

  const userGlobalShortcuts = computed({
    get: () => userSettingsStore.userSettings.globalShortcuts ?? {},
    set: async (value: UserGlobalShortcuts) => {
      userSettingsStore.userSettings.globalShortcuts = value;
      await userSettingsStore.setUserSettingsStorage('globalShortcuts', value);
    },
  });

  function getShortcutString(globalShortcutId: GlobalShortcutId): string {
    const userShortcut = userGlobalShortcuts.value[globalShortcutId];
    const raw = userShortcut ?? definitions.value.find(d => d.id === globalShortcutId)?.defaultShortcut ?? '';
    return raw ? normalizeShortcutString(raw) : '';
  }

  function getShortcutLabel(globalShortcutId: GlobalShortcutId): string {
    return formatTauriShortcut(getShortcutString(globalShortcutId));
  }

  function getShortcutKeys(globalShortcutId: GlobalShortcutId): ShortcutKeys {
    return tauriFormatToShortcutKeys(getShortcutString(globalShortcutId));
  }

  function isCustomized(globalShortcutId: GlobalShortcutId): boolean {
    return userGlobalShortcuts.value[globalShortcutId] !== undefined;
  }

  function getSource(globalShortcutId: GlobalShortcutId): 'system' | 'user' {
    return isCustomized(globalShortcutId) ? 'user' : 'system';
  }

  async function focusAppWindow(): Promise<void> {
    const appWindow = await WebviewWindow.getByLabel('main');
    if (!appWindow) return;
    await appWindow.show();
    await appWindow.unminimize();
    await appWindow.setFocus();
  }

  function getHandler(globalShortcutId: GlobalShortcutId): (() => Promise<void>) | null {
    const handlers: Record<GlobalShortcutId, () => Promise<void>> = {
      launchApp: focusAppWindow,
    };
    return handlers[globalShortcutId] ?? null;
  }

  async function registerShortcut(globalShortcutId: GlobalShortcutId): Promise<void> {
    const shortcutString = getShortcutString(globalShortcutId);
    if (!shortcutString) return;

    const handler = getHandler(globalShortcutId);
    if (!handler) return;

    try {
      await unregister(shortcutString);
    }
    catch {
    }

    try {
      await register(shortcutString, async (event) => {
        if (event.state === 'Pressed') {
          await handler();
        }
      });

      registeredShortcuts.value.set(globalShortcutId, shortcutString);
    }
    catch (error) {
      console.error(`Failed to register global shortcut "${shortcutString}" for "${globalShortcutId}":`, error);
    }
  }

  async function unregisterShortcut(globalShortcutId: GlobalShortcutId): Promise<void> {
    const existingShortcut = registeredShortcuts.value.get(globalShortcutId);
    if (!existingShortcut) return;

    try {
      await unregister(existingShortcut);
      registeredShortcuts.value.delete(globalShortcutId);
    }
    catch (error) {
      console.error(`Failed to unregister global shortcut "${existingShortcut}":`, error);
    }
  }

  async function syncTrayShortcutHint(): Promise<void> {
    try {
      const shortcutLabel = getShortcutLabel('launchApp');
      await invoke('update_tray_shortcut', { shortcut: shortcutLabel });
    }
    catch (error) {
      console.error('Failed to sync tray shortcut hint:', error);
    }
  }

  async function setShortcut(globalShortcutId: GlobalShortcutId, keys: ShortcutKeys): Promise<void> {
    await unregisterShortcut(globalShortcutId);
    const tauriShortcut = shortcutKeysToTauriFormat(keys);
    const newShortcuts = {
      ...userGlobalShortcuts.value,
      [globalShortcutId]: tauriShortcut,
    };
    userGlobalShortcuts.value = newShortcuts;
    await registerShortcut(globalShortcutId);
    await syncTrayShortcutHint();
  }

  async function resetShortcut(globalShortcutId: GlobalShortcutId): Promise<void> {
    await unregisterShortcut(globalShortcutId);
    const newShortcuts = { ...userGlobalShortcuts.value };
    delete newShortcuts[globalShortcutId];
    userGlobalShortcuts.value = newShortcuts;
    await registerShortcut(globalShortcutId);
    await syncTrayShortcutHint();
  }

  async function registerAllShortcuts(): Promise<void> {
    for (const definition of definitions.value) {
      await registerShortcut(definition.id);
    }
  }

  async function unregisterAllShortcuts(): Promise<void> {
    try {
      await unregisterAll();
      registeredShortcuts.value.clear();
    }
    catch (error) {
      console.error('Failed to unregister all global shortcuts:', error);
    }
  }

  async function init(): Promise<void> {
    if (isInitialized.value) return;
    isInitialized.value = true;
    await registerAllShortcuts();
    await syncTrayShortcutHint();
  }

  async function cleanup(): Promise<void> {
    await unregisterAllShortcuts();
    isInitialized.value = false;
  }

  return {
    definitions,
    isInitialized,
    getShortcutString,
    getShortcutLabel,
    getShortcutKeys,
    isCustomized,
    getSource,
    setShortcut,
    resetShortcut,
    registerAllShortcuts,
    unregisterAllShortcuts,
    init,
    cleanup,
  };
});
