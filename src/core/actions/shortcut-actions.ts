// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { ActionDefinition } from '@/core/actions/types';
import type { ActionRegistry } from '@/core/actions/registry';
import { createActionRegistry } from '@/core/actions/registry';

/**
 * 快捷键在动作注册表里的 id 命名空间。
 *
 * 快捷键（copy / cut / paste …）历史上只以 `ShortcutId` 字符串存在，和命令
 * （扩展命令、内置命令）不在同一个命名空间里。批次 2 起快捷键也注册成动作，
 * id 统一为 `sigma.shortcut.<shortcutId>`，于是「一个动作」与「一个快捷键」
 * 可以用同一个 id 寻址。
 */
export const SHORTCUT_ACTION_ID_PREFIX = 'sigma.shortcut.';

/** 注册表里快捷键动作所属的来源标识。 */
export const SHORTCUT_ACTION_SOURCE_ID = 'sigma.shortcuts';

export function buildShortcutActionId(shortcutId: string): string {
  return `${SHORTCUT_ACTION_ID_PREFIX}${shortcutId}`;
}

export function isShortcutActionId(value: string): boolean {
  return value.startsWith(SHORTCUT_ACTION_ID_PREFIX);
}

/**
 * 把动作 id 还原成快捷键 id；不是快捷键动作时返回 null。
 */
export function parseShortcutActionId(actionId: string): string | null {
  if (!isShortcutActionId(actionId)) {
    return null;
  }

  const shortcutId = actionId.slice(SHORTCUT_ACTION_ID_PREFIX.length);

  return shortcutId.length > 0 ? shortcutId : null;
}

/**
 * 核心层只需要快捷键 id 与它的文案 key，不依赖 store 与 i18n。
 */
export type ShortcutActionDescriptor = {
  id: string;
  labelKey: string;
};

/**
 * 把快捷键声明注册进动作注册表。
 *
 * `title` 用快捷键 id 占位，真正的显示名由 `titleKey`（i18n 路径）决定，
 * 避免在还没接入界面的批次里把机器标识当成标题显示出去。
 * 同一个快捷键 id 重复声明时保留先注册的那条，与注册表「先注册者赢」的语义一致。
 */
export function buildShortcutActionRegistry(
  descriptors: ShortcutActionDescriptor[],
): ActionRegistry {
  const registry = createActionRegistry();

  for (const descriptor of descriptors) {
    const definition: ActionDefinition = {
      id: buildShortcutActionId(descriptor.id),
      extensionId: SHORTCUT_ACTION_SOURCE_ID,
      title: descriptor.id,
      source: 'builtin',
    };

    if (descriptor.labelKey) {
      definition.titleKey = descriptor.labelKey;
    }

    registry.register(definition);
  }

  return registry;
}
