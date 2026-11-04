// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type {
  ExtensionContextMenuItem,
  ExtensionSidebarItem,
  ExtensionToolbarDropdown,
  ExtensionCommand,
  ContextMenuContext,
  Disposable,
} from '@/types/extension';
import { clearExtensionContextListeners } from '@/modules/extensions/context';
import { clearExtensionClipboardChangeListeners } from '@/modules/extensions/api/clipboard-change';
import {
  clearKeybindingRegistrationsForExtension,
  clearAllKeybindingRegistrations,
} from '@/modules/extensions/api/keybindings';
import {
  deleteExtensionConfiguration,
  clearExtensionConfigurations,
  clearAllSettingsChangeListeners,
  clearExtensionSettingsChangeListeners,
} from '@/modules/extensions/api/configuration';

export type ContextMenuRegistration = {
  extensionId: string;
  item: ExtensionContextMenuItem;
  handler: (context: ContextMenuContext) => Promise<void> | void;
};

export type SidebarRegistration = {
  extensionId: string;
  page: ExtensionSidebarItem;
};

export type ToolbarRegistration = {
  extensionId: string;
  dropdown: ExtensionToolbarDropdown;
  handlers: Record<string, () => Promise<void> | void>;
};

export type CommandRegistration = {
  extensionId: string;
  command: ExtensionCommand;
  handler: (...args: unknown[]) => Promise<unknown> | unknown;
};

/**
 * 批次 3：扩展命令的动作投影。
 *
 * 扩展经 `createCommands().registerCommand()` 注册的命令会同步登记成一条动作，
 * 命令面板因此可以直接从动作注册表投影出条目 ——
 * 不再依赖「扩展激活时把命令灌进 store」那条旁路。
 */
export type ExtensionCommandAction = {
  extensionId: string;
  /** 完整命令 id（已带扩展前缀，与 `command.id` 一致）。 */
  id: string;
  title: string;
};

const contextMenuRegistrations: ContextMenuRegistration[] = [];
const sidebarRegistrations: SidebarRegistration[] = [];
const toolbarRegistrations: ToolbarRegistration[] = [];
const commandRegistrations: CommandRegistration[] = [];

/**
 * 已注册扩展命令的动作投影。
 *
 * ⚠️ 它是 `commandRegistrations` 的**投影**，不是另一份事实源 ——
 * 每次命令注册表变动都整体重算一遍，所以「先注册者赢」与 dispose 后的回收
 * 与命令注册表始终一致，不会出现「命令没了动作还在」。
 */
const extensionCommandActions = new Map<string, ExtensionCommandAction>();

function syncExtensionCommandActions(): void {
  extensionCommandActions.clear();

  for (const registration of commandRegistrations) {
    const id = registration.command.id;

    if (extensionCommandActions.has(id)) {
      continue;
    }

    extensionCommandActions.set(id, {
      extensionId: registration.extensionId,
      id,
      title: registration.command.title,
    });
  }
}

export function addContextMenuRegistration(registration: ContextMenuRegistration): Disposable {
  contextMenuRegistrations.push(registration);
  return {
    dispose: () => {
      const index = contextMenuRegistrations.indexOf(registration);

      if (index !== -1) {
        contextMenuRegistrations.splice(index, 1);
      }
    },
  };
}

export function addSidebarRegistration(registration: SidebarRegistration): Disposable {
  sidebarRegistrations.push(registration);
  return {
    dispose: () => {
      const index = sidebarRegistrations.indexOf(registration);

      if (index !== -1) {
        sidebarRegistrations.splice(index, 1);
      }
    },
  };
}

export function addToolbarRegistration(registration: ToolbarRegistration): Disposable {
  toolbarRegistrations.push(registration);
  return {
    dispose: () => {
      const index = toolbarRegistrations.indexOf(registration);

      if (index !== -1) {
        toolbarRegistrations.splice(index, 1);
      }
    },
  };
}

export function addCommandRegistration(registration: CommandRegistration): Disposable {
  commandRegistrations.push(registration);
  // 批次 3：登记动作投影，命令面板随即就能看到这条命令。
  syncExtensionCommandActions();

  return {
    dispose: () => {
      const index = commandRegistrations.indexOf(registration);

      if (index !== -1) {
        commandRegistrations.splice(index, 1);
      }

      // 命令被释放 ⇒ 动作投影同步释放（否则面板里会留着点不动的条目）。
      syncExtensionCommandActions();
    },
  };
}

export function getContextMenuRegistrations(): ContextMenuRegistration[] {
  return [...contextMenuRegistrations];
}

export function getSidebarRegistrations(): SidebarRegistration[] {
  return [...sidebarRegistrations];
}

export function getToolbarRegistrations(): ToolbarRegistration[] {
  return [...toolbarRegistrations];
}

export function getCommandRegistrations(): CommandRegistration[] {
  return [...commandRegistrations];
}

export function findCommandRegistration(commandId: string): CommandRegistration | undefined {
  return commandRegistrations.find(reg => reg.command.id === commandId);
}

/**
 * 批次 3：已注册扩展命令的动作投影（供命令面板接到动作注册表上）。
 *
 * 返回的是**快照副本**，调用方拿它去建动作注册表即可。
 */
export function getExtensionCommandActions(): ExtensionCommandAction[] {
  return Array.from(extensionCommandActions.values());
}

export function clearAllRegistrations(): void {
  contextMenuRegistrations.length = 0;
  sidebarRegistrations.length = 0;
  toolbarRegistrations.length = 0;
  commandRegistrations.length = 0;
  syncExtensionCommandActions();
  clearAllKeybindingRegistrations();
  clearAllSettingsChangeListeners();
  clearExtensionConfigurations();
}

export function clearExtensionActivationRegistrations(extensionId: string): void {
  clearExtensionContextListeners(extensionId);
  clearExtensionClipboardChangeListeners(extensionId);
  clearExtensionSettingsChangeListeners(extensionId);

  for (let registrationIndex = contextMenuRegistrations.length - 1; registrationIndex >= 0; registrationIndex--) {
    if (contextMenuRegistrations[registrationIndex].extensionId === extensionId) {
      contextMenuRegistrations.splice(registrationIndex, 1);
    }
  }

  for (let registrationIndex = sidebarRegistrations.length - 1; registrationIndex >= 0; registrationIndex--) {
    if (sidebarRegistrations[registrationIndex].extensionId === extensionId) {
      sidebarRegistrations.splice(registrationIndex, 1);
    }
  }

  for (let registrationIndex = toolbarRegistrations.length - 1; registrationIndex >= 0; registrationIndex--) {
    if (toolbarRegistrations[registrationIndex].extensionId === extensionId) {
      toolbarRegistrations.splice(registrationIndex, 1);
    }
  }

  for (let registrationIndex = commandRegistrations.length - 1; registrationIndex >= 0; registrationIndex--) {
    if (commandRegistrations[registrationIndex].extensionId === extensionId) {
      commandRegistrations.splice(registrationIndex, 1);
    }
  }

  syncExtensionCommandActions();
}

export function clearExtensionRegistrations(extensionId: string): void {
  clearExtensionActivationRegistrations(extensionId);
  clearKeybindingRegistrationsForExtension(extensionId);
  deleteExtensionConfiguration(extensionId);
}
