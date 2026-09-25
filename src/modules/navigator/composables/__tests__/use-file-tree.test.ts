// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, it, expect, beforeEach, vi } from 'vitest';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

describe('useFileTree', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
  });

  it('initializes a tree rooted at a single path', () => {
    const tree = useFileTree({ rootPaths: ['C:/work'] });
    expect(tree.rows.value).toHaveLength(1);
    expect(tree.rows.value[0].path).toBe('C:/work');
    expect(tree.rows.value[0].isExpanded).toBe(false);
  });

  it('toggles expansion without loading children when collapsing', async () => {
    const readDir = vi.fn().mockResolvedValue([]);
    const tree = useFileTree({ rootPaths: ['C:/work'], deps: { readDir } });
    await tree.toggle(tree.rows.value[0].path);
    expect(tree.rows.value[0].isExpanded).toBe(true);
    await tree.toggle(tree.rows.value[0].path);
    expect(tree.rows.value[0].isExpanded).toBe(false);
    expect(tree.rows.value[0].hasChildren).toBe(true);
  });

  it('loads children lazily on first expand', async () => {
    const readDir = vi.fn().mockResolvedValue([
      { path: 'C:/work/a.md', name: 'a.md', isDirectory: false, size: 0, modifiedAt: 0 },
      { path: 'C:/work/sub', name: 'sub', isDirectory: true, size: 0, modifiedAt: 0 },
    ]);

    const tree = useFileTree({ rootPaths: ['C:/work'], deps: { readDir } });
    await tree.toggle('C:/work');

    expect(readDir).toHaveBeenCalledOnce();
    expect(tree.rows.value).toHaveLength(3); // root + 2 children
    expect(tree.rows.value[1].depth).toBe(1);
  });
});