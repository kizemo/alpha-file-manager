// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, it, expect } from 'vitest';
import type { FileTreeNode, FileTreeFlatRow } from '@/modules/navigator/types/file-tree';

describe('FileTreeNode', () => {
  it('represents an expandable directory', () => {
    const node: FileTreeNode = {
      path: 'C:/work',
      name: 'work',
      isDirectory: true,
      isExpanded: false,
      isLoaded: false,
      depth: 0,
      children: null,
    };
    expect(node.isDirectory).toBe(true);
    expect(node.children).toBeNull();
  });

  it('represents a leaf file', () => {
    const node: FileTreeNode = {
      path: 'C:/work/readme.md',
      name: 'readme.md',
      isDirectory: false,
      isExpanded: false,
      isLoaded: true,
      depth: 1,
      children: null,
    };
    expect(node.isDirectory).toBe(false);
  });
});

describe('FileTreeFlatRow', () => {
  it('flattens a node tree into a list view', () => {
    const root: FileTreeNode = {
      path: 'C:/work', name: 'work', isDirectory: true,
      isExpanded: true, isLoaded: true, depth: 0,
      children: [
        { path: 'C:/work/a.md', name: 'a.md', isDirectory: false, isExpanded: false, isLoaded: true, depth: 1, children: null },
      ],
    };
    const rows: FileTreeFlatRow[] = [
      { path: root.path, name: root.name, depth: 0, isDirectory: true, hasChildren: true, isExpanded: true },
      { path: root.children![0].path, name: root.children![0].name, depth: 1, isDirectory: false, hasChildren: false, isExpanded: false },
    ];
    expect(rows).toHaveLength(2);
  });
});