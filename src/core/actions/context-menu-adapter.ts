// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type {
  ActionAvailability,
  ActionContext,
  ActionDefinition,
  ActionSource,
  ActionWhen,
} from '@/core/actions/types';
import type { ActionRegistry } from '@/core/actions/registry';
import { createActionRegistry } from '@/core/actions/registry';

/**
 * 内置动作的来源标识。
 *
 * 右键菜单里的动作全部由核心提供（扩展命令走另一条注册路径），
 * 所以来源固定是 `sigma`。
 */
export const CONTEXT_MENU_ACTION_SOURCE_ID = 'sigma';

/**
 * 上下文菜单动作的声明。
 *
 * 内核不认识图标与 i18n：`title` 只是占位文案，真实显示名由界面层决定；
 * `titleKey` 也只是被搬运，核心不解析它。
 */
export type ContextMenuActionInput = {
  /** 动作 id。界面层用它把界面元素与注册表里的动作对上。 */
  id: string;
  /** 占位标题。 */
  title: string;
  /** i18n 路径（可选）。 */
  titleKey?: string;
  /** 可用性谓词。返回 false ⇒ 动作不可用。 */
  when?: ActionWhen;
  /**
   * 不可用时是否**隐藏**。
   *
   * 右键菜单的默认语义是「不适用就不出现」，所以默认值为 true。
   * 需要「显示但置灰」时显式传 false。
   */
  hideWhenUnavailable?: boolean;
};

export type ContextMenuActionBuildOptions = {
  actions: ContextMenuActionInput[];
  extensionId?: string;
  source?: ActionSource;
};

/**
 * 右键菜单条目投影。
 *
 * `availability` 的三态与注册表一致，但**含义在菜单里多了一层**：
 *
 * - `enabled` —— 显示，可点击；
 * - `disabled` —— **显示但置灰**（界面层负责 `disabled` / `aria-disabled`）；
 * - `hidden` —— 不显示（这一项**不会**出现在投影结果里）。
 */
export type ContextMenuActionEntry = {
  actionId: string;
  availability: ActionAvailability;
};

/**
 * 把动作声明注册进动作注册表。
 *
 * 同一个 id 重复声明时保留先注册的那条，与注册表「先注册者赢」的语义一致。
 */
export function buildContextMenuActionRegistry(
  options: ContextMenuActionBuildOptions,
): ActionRegistry {
  const registry = createActionRegistry();
  const extensionId = options.extensionId ?? CONTEXT_MENU_ACTION_SOURCE_ID;
  const source = options.source ?? 'builtin';

  for (const action of options.actions) {
    const definition: ActionDefinition = {
      id: action.id,
      extensionId,
      title: action.title,
      source,
      // 右键菜单默认「不适用就不显示」。
      hideWhenUnavailable: action.hideWhenUnavailable ?? true,
    };

    if (action.titleKey) {
      definition.titleKey = action.titleKey;
    }

    if (action.when) {
      definition.when = action.when;
    }

    registry.register(definition);
  }

  return registry;
}

/**
 * 把动作注册表投影成右键菜单条目。
 *
 * `hidden` 的动作**不进**结果（不显示）；`disabled` 的动作**进**结果，
 * 由界面层决定怎么置灰 —— 这是「显示但置灰」与「不显示」的分界线。
 */
export function getContextMenuActionEntries(
  registry: ActionRegistry,
  context: ActionContext,
): ContextMenuActionEntry[] {
  const entries: ContextMenuActionEntry[] = [];

  for (const resolved of registry.resolve(context)) {
    if (resolved.availability === 'hidden') {
      continue;
    }

    entries.push({
      actionId: resolved.definition.id,
      availability: resolved.availability,
    });
  }

  return entries;
}
