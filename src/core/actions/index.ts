// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

export type {
  ActionAvailability,
  ActionContext,
  ActionDefinition,
  ActionSource,
  ActionWhen,
  ResolvedAction,
} from '@/core/actions/types';
export type { ActionRegistry } from '@/core/actions/registry';
export type { ShortcutActionDescriptor } from '@/core/actions/shortcut-actions';
export type {
  ShortcutBindingFormat,
  ShortcutBindingLookup,
  ShortcutBindingMigration,
  ShortcutBindingValue,
} from '@/core/actions/shortcut-bindings';
export type {
  PaletteActionBuildOptions,
  PaletteActionCommand,
  PaletteActionEntry,
  PaletteActionInput,
} from '@/core/actions/palette-adapter';

export { createActionRegistry } from '@/core/actions/registry';
export {
  alwaysAvailable,
  combineWhen,
  getActionContext,
  whenNoDialogOpen,
  whenNotEditingText,
  whenRouteIs,
} from '@/core/actions/predicates';
export {
  buildPaletteActionRegistry,
  getPaletteActionEntries,
} from '@/core/actions/palette-adapter';
export type {
  ContextMenuActionBuildOptions,
  ContextMenuActionEntry,
  ContextMenuActionInput,
} from '@/core/actions/context-menu-adapter';
export {
  buildContextMenuActionRegistry,
  CONTEXT_MENU_ACTION_SOURCE_ID,
  getContextMenuActionEntries,
} from '@/core/actions/context-menu-adapter';
export {
  buildShortcutActionId,
  buildShortcutActionRegistry,
  isShortcutActionId,
  parseShortcutActionId,
  SHORTCUT_ACTION_ID_PREFIX,
  SHORTCUT_ACTION_SOURCE_ID,
} from '@/core/actions/shortcut-actions';
export {
  isShortcutBindingValue,
  isShortcutKeys,
  lookupShortcutBinding,
  migrateShortcutBindings,
  pickShortcutBindingSlot,
} from '@/core/actions/shortcut-bindings';
