// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { ActionContext, ActionWhen } from '@/core/actions/types';
import { isDialogOpened, isInputFieldActive } from '@/utils/dom-interaction-state';

/**
 * Reads the live ambient state. Callers pass the active route name in so this
 * module stays free of router imports and can run outside a component.
 */
export function getActionContext(activeRouteName: string | null = null): ActionContext {
  return {
    isInputFieldActive: isInputFieldActive(),
    isDialogOpened: isDialogOpened(),
    activeRouteName,
  };
}

export function alwaysAvailable(): ActionWhen {
  return () => true;
}

export function whenNoDialogOpen(): ActionWhen {
  return context => !context.isDialogOpened;
}

export function whenNotEditingText(): ActionWhen {
  return context => !context.isInputFieldActive;
}

export function whenRouteIs(routeName: string): ActionWhen {
  return context => context.activeRouteName === routeName;
}

export function combineWhen(...predicates: ActionWhen[]): ActionWhen {
  return context => predicates.every(predicate => predicate(context));
}
