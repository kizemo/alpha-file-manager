// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type {
  ActionContext,
  ActionDefinition,
  ActionSource,
  ActionWhen,
} from '@/core/actions/types';
import type { ActionRegistry } from '@/core/actions/registry';
import { createActionRegistry } from '@/core/actions/registry';

/**
 * Narrow input shape. The adapter deliberately does not depend on the
 * extension runtime types so it can be unit tested without Tauri mocks.
 */
export type PaletteActionInput = {
  extensionId: string;
  id: string;
  title: string;
  showInPalette?: boolean;
  when?: ActionWhen;
  hideWhenUnavailable?: boolean;
};

export type PaletteActionBuildOptions = {
  extensionActions: PaletteActionInput[];
  builtinActions: PaletteActionInput[];
  titleKeys?: Record<string, string>;
};

export type PaletteActionCommand = {
  id: string;
  title: string;
};

export type PaletteActionEntry = {
  extensionId: string;
  command: PaletteActionCommand;
  titleKey?: string;
};

function toDefinition(
  action: PaletteActionInput,
  source: ActionSource,
  titleKeys: Record<string, string>,
): ActionDefinition {
  const definition: ActionDefinition = {
    id: action.id,
    extensionId: action.extensionId,
    title: action.title,
    source,
  };
  const titleKey = titleKeys[action.id];

  if (titleKey) {
    definition.titleKey = titleKey;
  }

  if (action.when) {
    definition.when = action.when;
  }

  if (action.hideWhenUnavailable) {
    definition.hideWhenUnavailable = action.hideWhenUnavailable;
  }

  return definition;
}

/**
 * Registers runtime/extension commands first, then the builtin declarations
 * that opt into the palette. The registry keeps the first registration, which
 * reproduces the previous `existingIds` de-duplication.
 */
export function buildPaletteActionRegistry(
  options: PaletteActionBuildOptions,
): ActionRegistry {
  const registry = createActionRegistry();
  const titleKeys = options.titleKeys ?? {};

  for (const action of options.extensionActions) {
    registry.register(toDefinition(action, 'extension', titleKeys));
  }

  for (const action of options.builtinActions) {
    if (action.showInPalette !== true) continue;

    registry.register(toDefinition(action, 'builtin', titleKeys));
  }

  return registry;
}

export function getPaletteActionEntries(
  registry: ActionRegistry,
  context: ActionContext,
): PaletteActionEntry[] {
  const entries: PaletteActionEntry[] = [];

  for (const resolved of registry.resolve(context)) {
    if (resolved.availability !== 'enabled') continue;

    const entry: PaletteActionEntry = {
      extensionId: resolved.definition.extensionId,
      command: {
        id: resolved.definition.id,
        title: resolved.definition.title,
      },
    };

    if (resolved.definition.titleKey) {
      entry.titleKey = resolved.definition.titleKey;
    }

    entries.push(entry);
  }

  return entries;
}
