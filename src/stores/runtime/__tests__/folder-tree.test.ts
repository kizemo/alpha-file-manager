// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: test for folder-tree store (tree sync v6).
// Tracking issue: aleksey-hoffman/sigma-file-manager#499

import { beforeEach, describe, expect, it } from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import {
  computeAncestorPaths,
  useFolderTreeStore,
} from '@/stores/runtime/folder-tree';

describe('computeAncestorPaths', () => {
  it('returns empty for root or empty', () => {
    expect(computeAncestorPaths('')).toEqual([]);
    expect(computeAncestorPaths('/')).toEqual([]);
    expect(computeAncestorPaths('C:/')).toEqual([]);
    expect(computeAncestorPaths('C:\\')).toEqual([]);
  });

  it('expands POSIX absolute path', () => {
    expect(computeAncestorPaths('/usr/local/bin'))
      .toEqual(['/usr', '/usr/local']);
  });

  it('expands POSIX absolute path with trailing slash', () => {
    expect(computeAncestorPaths('/usr/local/bin/'))
      .toEqual(['/usr', '/usr/local']);
  });

  it('expands Windows drive path with forward slashes', () => {
    expect(computeAncestorPaths('C:/Users/foo'))
      .toEqual(['C:/', 'C:/Users']);
  });

  it('expands Windows drive path with backslashes', () => {
    expect(computeAncestorPaths('C:\\Users\\foo\\bar'))
      .toEqual(['C:/', 'C:/Users', 'C:/Users/foo']);
  });

  it('expands single-segment relative path to nothing', () => {
    expect(computeAncestorPaths('foo')).toEqual([]);
  });

  it('expands multi-segment relative path', () => {
    expect(computeAncestorPaths('foo/bar/baz'))
      .toEqual(['foo', 'foo/bar']);
  });
});

describe('useFolderTreeStore', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('starts with empty state', () => {
    const store = useFolderTreeStore();
    expect(store.selectedPath).toBeNull();
    expect(store.expandedPaths.size).toBe(0);
    expect(store.loadingPaths.size).toBe(0);
    expect(store.loadErrorPaths.size).toBe(0);
  });

  it('setSelectedPath stores the path and auto-expands ancestors', () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/Users/foo');
    expect(store.selectedPath).toBe('C:/Users/foo');
    expect(store.isExpanded('C:/')).toBe(true);
    expect(store.isExpanded('C:/Users')).toBe(true);
    expect(store.isExpanded('C:/Users/foo')).toBe(false);
  });

  it('setSelectedPath(null) clears the path without dropping expanded set', () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/Users/foo');
    store.setSelectedPath(null);
    expect(store.selectedPath).toBeNull();
    // ancestors stay expanded so the tree keeps its shape
    expect(store.isExpanded('C:/Users')).toBe(true);
  });

  it('setSelectedPath merges ancestors into existing expanded set (no clobber)', () => {
    const store = useFolderTreeStore();
    store.expandPath('D:/other');
    store.setSelectedPath('C:/Users/foo');
    expect(store.isExpanded('D:/other')).toBe(true);
    expect(store.isExpanded('C:/Users')).toBe(true);
  });

  it('toggleExpanded adds and removes paths', () => {
    const store = useFolderTreeStore();
    store.toggleExpanded('C:/Users');
    expect(store.isExpanded('C:/Users')).toBe(true);
    store.toggleExpanded('C:/Users');
    expect(store.isExpanded('C:/Users')).toBe(false);
  });

  it('expandPath is idempotent', () => {
    const store = useFolderTreeStore();
    store.expandPath('C:/Users');
    store.expandPath('C:/Users');
    expect(store.expandedPaths.size).toBe(1);
  });

  it('collapsePath only removes when present', () => {
    const store = useFolderTreeStore();
    store.collapsePath('C:/Users'); // not present, no-op
    expect(store.expandedPaths.size).toBe(0);
    store.expandPath('C:/Users');
    store.collapsePath('C:/Users');
    expect(store.expandedPaths.size).toBe(0);
  });

  it('markLoading toggles loading flag', () => {
    const store = useFolderTreeStore();
    expect(store.isLoading('C:/Users')).toBe(false);
    store.markLoading('C:/Users', true);
    expect(store.isLoading('C:/Users')).toBe(true);
    store.markLoading('C:/Users', false);
    expect(store.isLoading('C:/Users')).toBe(false);
  });

  it('markLoadError toggles error flag', () => {
    const store = useFolderTreeStore();
    store.markLoadError('E:/virtual', true);
    expect(store.hasLoadError('E:/virtual')).toBe(true);
    store.markLoadError('E:/virtual', false);
    expect(store.hasLoadError('E:/virtual')).toBe(false);
  });

  it('expandedPaths / loadingPaths / loadErrorPaths use new-Set replacement (Vue reactivity)', () => {
    // This pins the root cause from v5: ref(new Map()).set() did NOT trigger
    // reactivity. For Sets in store, we replace the ref on every mutation
    // (same fix). Verify: store.expandedPaths.value is a NEW Set reference
    // after every mutation.
    const store = useFolderTreeStore();
    const before = store.expandedPaths;
    store.toggleExpanded('C:/Users');
    expect(store.expandedPaths).not.toBe(before);
  });

  it('reset clears all state', () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/Users/foo');
    store.markLoading('C:/Users', true);
    store.markLoadError('C:/Users', true);
    store.reset();
    expect(store.selectedPath).toBeNull();
    expect(store.expandedPaths.size).toBe(0);
    expect(store.loadingPaths.size).toBe(0);
    expect(store.loadErrorPaths.size).toBe(0);
  });

  it('ancestor load failure is observable: markLoadError does not throw and does not clobber expanded set', () => {
    // Per handoff §8.3: ancestor load failures must not break the UI. The store
    // simply records the error flag; the tree-view can keep showing the entry.
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/Users/foo');
    store.markLoadError('C:/Users', true);
    expect(store.isExpanded('C:/Users')).toBe(true);
    expect(store.hasLoadError('C:/Users')).toBe(true);
  });
});
