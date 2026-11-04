// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type {
  ActionAvailability,
  ActionContext,
  ActionDefinition,
  ResolvedAction,
} from '@/core/actions/types';

/**
 * A registry is a plain data holder: the owning surface rebuilds it from its
 * reactive snapshot on every change instead of mutating a global singleton.
 */
export type ActionRegistry = {
  register: (definition: ActionDefinition) => boolean;
  get: (id: string) => ActionDefinition | undefined;
  getAll: () => ActionDefinition[];
  has: (id: string) => boolean;
  size: () => number;
  clear: () => void;
  resolve: (context: ActionContext) => ResolvedAction[];
};

function resolveAvailability(
  definition: ActionDefinition,
  context: ActionContext,
): ActionAvailability {
  if (!definition.when) {
    return 'enabled';
  }

  let isAvailable = false;

  try {
    isAvailable = definition.when(context) === true;
  }
  catch {
    isAvailable = false;
  }

  if (isAvailable) {
    return 'enabled';
  }

  return definition.hideWhenUnavailable ? 'hidden' : 'disabled';
}

export function createActionRegistry(): ActionRegistry {
  const definitions = new Map<string, ActionDefinition>();

  return {
    register: (definition) => {
      if (!definition.id) {
        throw new Error('Action definition requires a non-empty id');
      }

      if (definitions.has(definition.id)) {
        return false;
      }

      definitions.set(definition.id, definition);

      return true;
    },
    get: id => definitions.get(id),
    getAll: () => Array.from(definitions.values()),
    has: id => definitions.has(id),
    size: () => definitions.size,
    clear: () => {
      definitions.clear();
    },
    resolve: context => Array.from(definitions.values()).map(definition => ({
      definition,
      availability: resolveAvailability(definition, context),
    })),
  };
}
