// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import type { ActionContext, ActionDefinition } from '@/core/actions/types';
import { createActionRegistry } from '@/core/actions/registry';
import { whenNoDialogOpen, whenRouteIs } from '@/core/actions/predicates';

function createContext(overrides: Partial<ActionContext> = {}): ActionContext {
  return {
    isInputFieldActive: false,
    isDialogOpened: false,
    activeRouteName: null,
    ...overrides,
  };
}

function createAction(overrides: Partial<ActionDefinition> = {}): ActionDefinition {
  return {
    id: 'sigma.test.action',
    extensionId: 'sigma',
    title: 'Test Action',
    source: 'builtin',
    ...overrides,
  };
}

function throwBrokenPredicate(): boolean {
  throw new Error('broken predicate');
}

describe('action registry', () => {
  it('registers a definition and reports it as present', () => {
    const registry = createActionRegistry();
    const added = registry.register(createAction({ id: 'sigma.a' }));

    expect(added).toBe(true);
    expect(registry.has('sigma.a')).toBe(true);
    expect(registry.size()).toBe(1);
    expect(registry.get('sigma.a')?.title).toBe('Test Action');
  });

  it('keeps the first definition when the same id is registered twice', () => {
    const registry = createActionRegistry();
    registry.register(createAction({
      id: 'sigma.a',
      title: 'First',
      extensionId: 'sigma',
    }));
    const added = registry.register(createAction({
      id: 'sigma.a',
      title: 'Second',
      extensionId: 'other',
    }));

    expect(added).toBe(false);
    expect(registry.size()).toBe(1);
    expect(registry.get('sigma.a')?.title).toBe('First');
    expect(registry.get('sigma.a')?.extensionId).toBe('sigma');
  });

  it('rejects a definition without an id', () => {
    const registry = createActionRegistry();

    expect(() => registry.register(createAction({ id: '' }))).toThrow();
    expect(registry.size()).toBe(0);
  });

  it('preserves registration order', () => {
    const registry = createActionRegistry();
    registry.register(createAction({ id: 'sigma.a' }));
    registry.register(createAction({ id: 'sigma.b' }));
    registry.register(createAction({ id: 'sigma.c' }));

    expect(registry.getAll().map(action => action.id)).toEqual([
      'sigma.a',
      'sigma.b',
      'sigma.c',
    ]);
  });

  it('returns a copy instead of the internal collection', () => {
    const registry = createActionRegistry();
    registry.register(createAction({ id: 'sigma.a' }));
    registry.getAll().pop();

    expect(registry.size()).toBe(1);
  });

  it('marks definitions without a predicate as enabled', () => {
    const registry = createActionRegistry();
    registry.register(createAction({ id: 'sigma.a' }));

    expect(registry.resolve(createContext())[0].availability).toBe('enabled');
  });

  it('marks definitions whose predicate fails as disabled', () => {
    const registry = createActionRegistry();
    registry.register(createAction({
      id: 'sigma.a',
      when: whenNoDialogOpen(),
    }));

    const resolved = registry.resolve(createContext({ isDialogOpened: true }));

    expect(resolved[0].availability).toBe('disabled');
  });

  it('hides unavailable definitions that ask to be hidden', () => {
    const registry = createActionRegistry();
    registry.register(createAction({
      id: 'sigma.a',
      when: whenRouteIs('navigator'),
      hideWhenUnavailable: true,
    }));

    const resolved = registry.resolve(createContext({ activeRouteName: 'settings' }));

    expect(resolved[0].availability).toBe('hidden');
  });

  it('marks definitions with a passing predicate as enabled', () => {
    const registry = createActionRegistry();
    registry.register(createAction({
      id: 'sigma.a',
      when: whenRouteIs('navigator'),
    }));

    const resolved = registry.resolve(createContext({ activeRouteName: 'navigator' }));

    expect(resolved[0].availability).toBe('enabled');
  });

  it('treats a throwing predicate as unavailable instead of breaking resolution', () => {
    const registry = createActionRegistry();
    registry.register(createAction({
      id: 'sigma.a',
      when: throwBrokenPredicate,
    }));

    expect(registry.resolve(createContext())[0].availability).toBe('disabled');
  });

  it('clears every registration', () => {
    const registry = createActionRegistry();
    registry.register(createAction({ id: 'sigma.a' }));
    registry.clear();

    expect(registry.size()).toBe(0);
    expect(registry.has('sigma.a')).toBe(false);
  });
});
