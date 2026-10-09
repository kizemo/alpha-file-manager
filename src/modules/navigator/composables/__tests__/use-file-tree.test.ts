// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: test for use-file-tree (tree view, v6 reactive shared state).
// Tracking issue: aleksey-hoffman/sigma-file-manager#499

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { nextTick, ref } from 'vue';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

function makeTree(opts: Parameters<typeof useFileTree>[0]) {
  return useFileTree(opts);
}

describe('useFileTree', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('initializes a tree rooted at the given paths', () => {
    const expanded = ref<Set<string>>(new Set());
    const tree = makeTree({ rootPaths: ['C:/work'], expandedPaths: expanded });
    expect(tree.rows.value).toHaveLength(1);
    expect(tree.rows.value[0].path).toBe('C:/work');
    expect(tree.rows.value[0].isExpanded).toBe(false);
  });

  it('reflects expansion state from the caller-owned ref', async () => {
    const expanded = ref<Set<string>>(new Set());
    const readDir = vi.fn().mockResolvedValue([]);
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    expanded.value = new Set([...expanded.value, 'C:/work']);
    await nextTick();
    await nextTick();
    await tree.ensureLoaded('C:/work');

    expect(readDir).toHaveBeenCalledOnce();
    expect(tree.rows.value[0].isExpanded).toBe(true);
    expect(tree.rows.value).toHaveLength(1);
  });

  it('loads children lazily on first expand (via side-effect watch)', async () => {
    const expanded = ref<Set<string>>(new Set());
    const readDir = vi.fn().mockResolvedValue([
      { path: 'C:/work/a.md', name: 'a.md', isDirectory: false, size: 0, modifiedAt: 0 },
      { path: 'C:/work/sub', name: 'sub', isDirectory: true, size: 0, modifiedAt: 0 },
    ]);
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    expanded.value = new Set([...expanded.value, 'C:/work']);
    await nextTick();
    await nextTick();

    expect(readDir).toHaveBeenCalledOnce();
    expect(tree.rows.value).toHaveLength(3); // root + 2 children
    expect(tree.rows.value[1].depth).toBe(1);
  });

  it('ensureLoaded is idempotent on already-loaded paths', async () => {
    const expanded = ref<Set<string>>(new Set());
    const readDir = vi.fn().mockResolvedValue([]);
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    await tree.ensureLoaded('C:/work');
    await tree.ensureLoaded('C:/work');
    await tree.ensureLoaded('C:/work');

    expect(readDir).toHaveBeenCalledTimes(1);
  });

  it('ensureAncestorsLoaded adds each ancestor as a root and loads it', async () => {
    const expanded = ref<Set<string>>(new Set());
    const readDir = vi.fn().mockImplementation(async (p: string) => {
      if (p === 'C:/') return [
        { path: 'C:/Users', name: 'Users', is_dir: true, size: 0, modified_time: 0 },
      ];
      if (p === 'C:/Users') return [
        { path: 'C:/Users/foo', name: 'foo', is_dir: true, size: 0, modified_time: 0 },
      ];
      return [];
    });
    const tree = makeTree({
      rootPaths: ['C:/Users/foo'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    await tree.ensureAncestorsLoaded(['C:/', 'C:/Users']);
    expect(readDir).toHaveBeenCalledTimes(2);
    expect(readDir).toHaveBeenNthCalledWith(1, 'C:/');
    expect(readDir).toHaveBeenNthCalledWith(2, 'C:/Users');
  });

  it('readDir failure surfaces via onLoadError and does not throw', async () => {
    const expanded = ref<Set<string>>(new Set());
    const boom = new Error('permission denied');
    const readDir = vi.fn().mockRejectedValue(boom);
    const onLoadError = vi.fn();
    const tree = makeTree({
      rootPaths: ['C:/locked'],
      expandedPaths: expanded,
      deps: { readDir },
      onLoadError,
    });

    await expect(tree.ensureLoaded('C:/locked')).resolves.toBeUndefined();
    expect(onLoadError).toHaveBeenCalledWith('C:/locked', boom);
    // Per handoff §8.3: load failure must not break the UI. The row stays
    // visible, just with no children.
    expect(tree.rows.value).toHaveLength(1);
    expect(tree.rows.value[0].isExpanded).toBe(false);
  });

  it('collapsing then re-expanding does not re-read (children stay loaded)', async () => {
    const expanded = ref<Set<string>>(new Set(['C:/work']));
    const readDir = vi.fn().mockResolvedValue([]);
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    await nextTick();
    await nextTick();
    expect(readDir).toHaveBeenCalledTimes(1);

    // Collapse then re-expand — should NOT re-read.
    expanded.value = new Set();
    await nextTick();
    expanded.value = new Set(['C:/work']);
    await nextTick();
    await nextTick();

    expect(readDir).toHaveBeenCalledTimes(1);
  });

  it('addRoot dynamically grows the tree (ancestor auto-expansion case)', () => {
    const expanded = ref<Set<string>>(new Set());
    const tree = makeTree({
      rootPaths: ['C:/Users/foo'],
      expandedPaths: expanded,
    });

    expect(tree.rows.value).toHaveLength(1);
    expect(tree.addRoot('C:/Users')).toBe(true);
    expect(tree.rows.value).toHaveLength(2);
    expect(tree.addRoot('C:/Users')).toBe(false); // idempotent
    expect(tree.rows.value).toHaveLength(2);
  });

  it('reactive rootPaths rebuilds the tree while preserving loaded subtrees', async () => {
    const rootPaths = ref(['C:/work']);
    const expanded = ref<Set<string>>(new Set(['C:/work']));
    const readDir = vi.fn().mockImplementation(async (p: string) => {
      if (p === 'C:/work') return [
        { path: 'C:/work/sub', name: 'sub', isDirectory: true, size: 0, modifiedAt: 0 },
      ];
      return [];
    });
    const tree = makeTree({ rootPaths, expandedPaths: expanded, deps: { readDir } });

    await nextTick();
    await nextTick();
    expect(tree.rows.value).toHaveLength(2); // root + sub

    // Switch to a different root — C:/work is no longer in the list, so its
    // subtree is dropped. D:/projects is the new sole root.
    rootPaths.value = ['D:/projects'];
    await nextTick();
    await nextTick();

    expect(tree.rows.value).toHaveLength(1);
    expect(tree.rows.value[0].path).toBe('D:/projects');
  });

  it('uses rootLabels for drive root display names (v6.4)', () => {
    // v6.4: drive roots show their volume label, not just "E:".
    const tree = makeTree({
      rootPaths: ['E:/'],
      rootLabels: { 'E:/': '系统盘 (E:)' },
      expandedPaths: ref(new Set()),
    });
    expect(tree.rows.value).toHaveLength(1);
    expect(tree.rows.value[0].name).toBe('系统盘 (E:)');
    expect(tree.rows.value[0].path).toBe('E:/');
  });

  it('falls back to path basename when rootLabels has no entry', () => {
    const tree = makeTree({
      rootPaths: ['C:/work'],
      rootLabels: {}, // empty
      expandedPaths: ref(new Set()),
    });
    expect(tree.rows.value[0].name).toBe('work');
  });

  it('handles trailing backslash / slash without producing an empty name', () => {
    // Regression guard: `E:\\` (Windows-style with trailing backslash) used
    // to render as an empty name because the basename split kept nothing.
    const tree = makeTree({
      rootPaths: ['E:\\'],
      expandedPaths: ref(new Set()),
    });
    expect(tree.rows.value[0].name).toBe('E:');
  });
});

describe('useFileTree — invalidate (v6.5)', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('invalidate lets ensureLoaded re-read a path that was already loaded', async () => {
    // The contract this preserves: `ensureLoaded` stays idempotent on its own
    // ("ensureLoaded is idempotent on already-loaded paths" above asserts
    // three calls produce one read). `invalidate` is the only way to buy a
    // second read.
    const newFolder = {
      path: 'C:/work/new',
      name: 'new',
      is_dir: true,
      size: 0,
      modifiedAt: 0,
    };
    const expanded = ref<Set<string>>(new Set());
    let readCount = 0;
    const readDir = vi.fn().mockImplementation(async () => {
      readCount++;

      if (readCount === 1) {
        return [];
      }

      return [newFolder];
    });
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    await tree.ensureLoaded('C:/work');
    expect(readDir).toHaveBeenCalledTimes(1);
    expect(tree.rows.value).toHaveLength(1); // no children yet

    expanded.value = new Set(['C:/work']);
    await nextTick();

    await tree.invalidate('C:/work');

    expect(readDir).toHaveBeenCalledTimes(2);
    expect(tree.rows.value.map(r => r.path)).toEqual(['C:/work', 'C:/work/new']);
  });

  it('surfaces a new child of an already-expanded dir without collapse/re-expand', async () => {
    // The exact user report: create a folder inside an expanded directory and
    // it never appears. Previously unreachable — no code path cleared the cache.
    const oldFile = {
      path: 'C:/work/a.md',
      name: 'a.md',
      is_dir: false,
      size: 0,
      modifiedAt: 0,
    };
    const freshFolder = {
      path: 'C:/work/fresh',
      name: 'fresh',
      is_dir: true,
      size: 0,
      modifiedAt: 0,
    };
    const expanded = ref<Set<string>>(new Set());
    let readCount = 0;
    const readDir = vi.fn().mockImplementation(async () => {
      readCount++;

      if (readCount === 1) {
        return [oldFile];
      }

      return [oldFile, freshFolder];
    });
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    expanded.value = new Set(['C:/work']);
    await nextTick();
    await nextTick();
    expect(tree.rows.value.map(r => r.path)).toEqual(['C:/work', 'C:/work/a.md']);

    // The filesystem mutation publishes the changed directory.
    await tree.invalidate('C:/work');

    // Expanded the whole time — no manual toggle, and invalidate had to
    // re-read on its own.
    expect(expanded.value.has('C:/work')).toBe(true);
    expect(readDir).toHaveBeenCalledTimes(2);
    expect(tree.rows.value.map(r => r.path)).toEqual([
      'C:/work',
      'C:/work/a.md',
      'C:/work/fresh',
    ]);
  });

  it('invalidate drops the loaded-subtree cache so expanded descendants re-read', async () => {
    // Guards the desync: reloading a parent replaces its child objects with
    // fresh, unloaded ones while `loadedSet` still claims they are loaded.
    // Without clearing the subtree, an expanded grandchild renders empty.
    const subEntry = {
      path: 'C:/work/sub',
      name: 'sub',
      is_dir: true,
      size: 0,
      modifiedAt: 0,
    };
    const leafEntry = {
      path: 'C:/work/sub/leaf.txt',
      name: 'leaf.txt',
      is_dir: false,
      size: 0,
      modifiedAt: 0,
    };
    const expanded = ref<Set<string>>(new Set());
    const readDir = vi.fn().mockImplementation(async (p: string) => {
      if (p === 'C:/work') {
        return [subEntry];
      }

      if (p === 'C:/work/sub') {
        return [leafEntry];
      }

      return [];
    });
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    // Build the starting state through awaited, sequential loads rather than
    // the expansion watcher: the watcher fires its loads concurrently, which
    // would make the read-order assertion below meaningless.
    await tree.ensureLoaded('C:/work');
    expanded.value = new Set(['C:/work']);
    await nextTick();
    await tree.ensureLoaded('C:/work/sub');
    expanded.value = new Set(['C:/work', 'C:/work/sub']);
    await nextTick();

    expect(readDir.mock.calls.map(c => c[0])).toEqual(['C:/work', 'C:/work/sub']);
    expect(tree.rows.value.map(r => r.path)).toContain('C:/work/sub/leaf.txt');

    await tree.invalidate('C:/work');

    // Parent re-read, then the still-expanded child — in that order, or the
    // child would load a node object the parent is about to replace.
    expect(readDir.mock.calls.map(c => c[0])).toEqual([
      'C:/work',
      'C:/work/sub',
      'C:/work',
      'C:/work/sub',
    ]);
    expect(tree.rows.value.map(r => r.path)).toContain('C:/work/sub/leaf.txt');
  });

  it('invalidate on an unknown path is a no-op that does not throw', async () => {
    const expanded = ref<Set<string>>(new Set());
    const readDir = vi.fn().mockResolvedValue([]);
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    await expect(tree.invalidate('C:/never/existed')).resolves.toBeUndefined();
    expect(tree.rows.value).toHaveLength(1);
  });

  it('invalidate keeps stale children visible until the re-read lands (no flicker)', async () => {
    const oldChild = {
      path: 'C:/work/old',
      name: 'old',
      is_dir: false,
      size: 0,
      modifiedAt: 0,
    };
    const readGate: { resolve?: (value: unknown[]) => void } = {};
    const expanded = ref<Set<string>>(new Set());
    let readCount = 0;
    const readDir = vi.fn().mockImplementation(async () => {
      readCount++;

      if (readCount === 1) {
        return [oldChild];
      }

      return new Promise<unknown[]>((r) => {
        readGate.resolve = r;
      });
    });
    const tree = makeTree({
      rootPaths: ['C:/work'],
      expandedPaths: expanded,
      deps: { readDir },
    });

    expanded.value = new Set(['C:/work']);
    await nextTick();
    await nextTick();
    expect(tree.rows.value.map(r => r.path)).toEqual(['C:/work', 'C:/work/old']);

    const pending = tree.invalidate('C:/work');
    // Mid-flight: the old child is still rendered rather than a blank gap.
    expect(tree.rows.value.map(r => r.path)).toEqual(['C:/work', 'C:/work/old']);

    readGate.resolve?.([]);
    await pending;
    expect(tree.rows.value.map(r => r.path)).toEqual(['C:/work']);
  });
});
