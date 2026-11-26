// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * Ambient state an action can be conditioned on.
 *
 * The context is captured once per render pass so every action in a single
 * surface (palette, toolbar, context menu) is evaluated against the same
 * snapshot instead of querying the DOM once per action.
 */
export type ActionContext = {
  isInputFieldActive: boolean;
  isDialogOpened: boolean;
  activeRouteName: string | null;
};

export type ActionWhen = (context: ActionContext) => boolean;

/**
 * `enabled`  — the action can run and is shown.
 * `disabled` — the action is shown but must not run.
 * `hidden`   — the action must not be shown at all.
 */
export type ActionAvailability = 'enabled' | 'disabled' | 'hidden';

export type ActionSource = 'builtin' | 'extension';

export type ActionDefinition = {
  id: string;
  extensionId: string;
  title: string;
  source: ActionSource;
  when?: ActionWhen;
  hideWhenUnavailable?: boolean;
  titleKey?: string;
};

export type ResolvedAction = {
  definition: ActionDefinition;
  availability: ActionAvailability;
};
