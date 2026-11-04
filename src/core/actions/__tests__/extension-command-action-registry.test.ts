// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import type { ExtensionPermission } from '@/types/extension';
import type { ExtensionContext } from '@/modules/extensions/api/extension-context';

// 扩展运行时的这些模块会拉起 Tauri / store 链，本用例只关心
// 「命令注册 → 动作投影 → 命令面板投影」这一段，故全部隔离掉。
vi.mock('@/modules/extensions/context', () => ({
  clearExtensionContextListeners: vi.fn(),
}));

vi.mock('@/modules/extensions/api/clipboard-change', () => ({
  clearExtensionClipboardChangeListeners: vi.fn(),
}));

vi.mock('@/modules/extensions/api/keybindings', () => ({
  clearKeybindingRegistrationsForExtension: vi.fn(),
  clearAllKeybindingRegistrations: vi.fn(),
}));

vi.mock('@/modules/extensions/api/configuration', () => ({
  deleteExtensionConfiguration: vi.fn(),
  clearExtensionConfigurations: vi.fn(),
  clearAllSettingsChangeListeners: vi.fn(),
  clearExtensionSettingsChangeListeners: vi.fn(),
}));

vi.mock('@/modules/extensions/builtin-commands', () => ({
  getBuiltinCommandDefinitions: vi.fn(() => []),
  getBuiltinCommandHandler: vi.fn(),
  getBuiltinCommandRequiredPermission: vi.fn(),
  isBuiltinCommand: vi.fn(() => false),
}));

import { createCommandsAPI } from '@/modules/extensions/api/create-commands-api';
import {
  clearAllRegistrations,
  getExtensionCommandActions,
} from '@/modules/extensions/api/registrations';
import {
  buildPaletteActionRegistry,
  getActionContext,
  getPaletteActionEntries,
} from '@/core/actions';

function createContext(permissions: ExtensionPermission[] = []): ExtensionContext {
  return {
    extensionId: 'test.extension',
    hasPermission: (permission: ExtensionPermission) => permissions.includes(permission),
    t: (key: string, params?: Record<string, string | number>) => params?.permission
      ? `${key}:${params.permission}`
      : key,
    getExtensionPath: vi.fn(),
    getExtensionStoragePath: vi.fn(),
    isPathWithinDirectory: vi.fn(),
    isInExtensionDir: vi.fn(),
    isInExtensionStorageDir: vi.fn(),
    getSharedBinariesDir: vi.fn(),
    isInSharedBinariesDir: vi.fn(),
    isInAllowedReadDir: vi.fn(),
    isInAllowedWriteDir: vi.fn(),
    normalizeRelativePath: vi.fn(),
    resolvePrivatePath: vi.fn(),
    resolveStoragePath: vi.fn(),
    getExtensionName: vi.fn(),
    getExtensionIconPath: vi.fn(),
    getExtensionToastTitle: vi.fn(),
    grantDialogReadAccess: vi.fn(),
    hasDialogReadAccess: vi.fn(),
    grantDialogWriteAccess: vi.fn(),
    consumeDialogWriteAccess: vi.fn(),
    grantSessionAccessFromNavigation: vi.fn(),
    grantSessionAccessFromCurrentNavigation: vi.fn(),
  };
}

/**
 * 命令面板的投影口径 —— 与 `command-palette.vue` 里 `resolveAllCommands()` 一致。
 */
function getPaletteCommandIds(): string[] {
  const registry = buildPaletteActionRegistry({
    extensionActions: getExtensionCommandActions().map(action => ({
      extensionId: action.extensionId,
      id: action.id,
      title: action.title,
    })),
    builtinActions: [],
  });

  return getPaletteActionEntries(registry, getActionContext(null))
    .map(entry => entry.command.id);
}

describe('扩展命令接到动作注册表上（判据 B）', () => {
  beforeEach(() => {
    clearAllRegistrations();
  });

  it('registerCommand() 注册的命令出现在命令面板投影里', () => {
    const commandsApi = createCommandsAPI(createContext(['commands']));

    commandsApi.registerCommand(
      {
        id: 'demo-command',
        title: 'Demo Command',
      },
      vi.fn(),
    );

    expect(getExtensionCommandActions()).toContainEqual({
      extensionId: 'test.extension',
      id: 'test.extension.demo-command',
      title: 'Demo Command',
    });

    const registry = buildPaletteActionRegistry({
      extensionActions: getExtensionCommandActions().map(action => ({
        extensionId: action.extensionId,
        id: action.id,
        title: action.title,
      })),
      builtinActions: [],
    });
    const paletteEntries = getPaletteActionEntries(registry, getActionContext(null));
    const entry = paletteEntries.find(item => item.command.id === 'test.extension.demo-command');

    expect(entry).toBeDefined();
    expect(entry?.command.title).toBe('Demo Command');
    expect(entry?.extensionId).toBe('test.extension');
  });

  it('负对照：命令被 dispose 后投影里不再有它（证明投影真的来自注册表）', () => {
    const commandsApi = createCommandsAPI(createContext(['commands']));
    const disposable = commandsApi.registerCommand(
      {
        id: 'temp-command',
        title: 'Temp Command',
      },
      vi.fn(),
    );

    expect(getPaletteCommandIds()).toContain('test.extension.temp-command');

    disposable.dispose();

    expect(getPaletteCommandIds()).not.toContain('test.extension.temp-command');
  });

  it('负对照：清空注册表后投影为空（登记这一步坏掉时正向断言就会红）', () => {
    const commandsApi = createCommandsAPI(createContext(['commands']));
    commandsApi.registerCommand(
      {
        id: 'demo-command',
        title: 'Demo Command',
      },
      vi.fn(),
    );

    expect(getPaletteCommandIds()).toContain('test.extension.demo-command');

    clearAllRegistrations();

    expect(getPaletteCommandIds()).not.toContain('test.extension.demo-command');
  });

  it('负对照：换一个命令 id 不会把原来的命令顶掉', () => {
    const commandsApi = createCommandsAPI(createContext(['commands']));

    commandsApi.registerCommand({
      id: 'first-command',
      title: 'First',
    }, vi.fn());
    commandsApi.registerCommand({
      id: 'second-command',
      title: 'Second',
    }, vi.fn());

    const ids = getPaletteCommandIds();

    expect(ids).toContain('test.extension.first-command');
    expect(ids).toContain('test.extension.second-command');
    expect(ids).toHaveLength(2);
  });
});
