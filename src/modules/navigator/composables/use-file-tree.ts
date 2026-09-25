// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: new file (tree view). Keep during upstream sync.
// Tracking issue: aleksey-hoffman/sigma-file-manager#499

import { ref, computed, type Ref, type ComputedRef } from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import { resolveDirectoryContents } from '@/utils/virtual-locations';
import type { FileTreeNode, FileTreeFlatRow } from '../types/file-tree';

export interface UseFileTreeDeps {
  readDir: (path: string) => Promise<Array<{
    path: string;
    name: string;
    isDirectory: boolean;
    size?: number;
    modifiedAt?: number;
  }>>;
}

export interface UseFileTreeOptions {
  rootPaths: string[];
  deps?: UseFileTreeDeps;
}

export interface UseFileTreeApi {
  rows: ComputedRef<FileTreeFlatRow[]>;
  toggle: (path: string) => Promise<void>;
  refresh: (path: string) => Promise<void>;
  expand: (path: string) => Promise<void>;
}

export function useFileTree(options: UseFileTreeOptions): UseFileTreeApi {
  const deps = options.deps ?? defaultDeps();
  const nodes = ref<FileTreeNode[]>(buildRoots(options.rootPaths));

  async function loadChildren(node: FileTreeNode): Promise<void> {
    if (!node.isDirectory || node.isLoaded) return;
    const entries = await deps.readDir(node.path);
    node.children = entries.map((entry) => ({
      path: entry.path,
      name: entry.name,
      isDirectory: entry.isDirectory,
      isExpanded: false,
      isLoaded: !entry.isDirectory,
      depth: node.depth + 1,
      children: null,
    }));
    node.isLoaded = true;
  }

  async function findAndMutate(
    path: string,
    mutator: (n: FileTreeNode) => Promise<void> | void,
  ): Promise<void> {
    async function walk(list: FileTreeNode[]): Promise<boolean> {
      for (const n of list) {
        if (n.path === path) { await mutator(n); return true; }
        if (n.children && (await walk(n.children))) return true;
      }
      return false;
    }
    await walk(nodes.value);
  }

  async function toggle(path: string): Promise<void> {
    await findAndMutate(path, async (n) => {
      if (!n.isDirectory) return;
      if (n.isExpanded) {
        n.isExpanded = false;
      } else {
        await loadChildren(n);
        n.isExpanded = true;
      }
    });
  }

  async function expand(path: string): Promise<void> {
    await findAndMutate(path, async (n) => {
      if (!n.isDirectory) return;
      await loadChildren(n);
      n.isExpanded = true;
    });
  }

  async function refresh(path: string): Promise<void> {
    await findAndMutate(path, (n) => {
      if (!n.isDirectory) return;
      n.isLoaded = false;
      n.children = null;
      n.isExpanded = false;
    });
  }

  const rows = computed<FileTreeFlatRow[]>(() => {
    const out: FileTreeFlatRow[] = [];
    function walk(list: FileTreeNode[]) {
      for (const n of list) {
        out.push({
          path: n.path,
          name: n.name,
          depth: n.depth,
          isDirectory: n.isDirectory,
          hasChildren: n.isDirectory,
          isExpanded: n.isExpanded,
        });
        if (n.isExpanded && n.children) walk(n.children);
      }
    }
    walk(nodes.value);
    return out;
  });

  return { rows, toggle, refresh, expand };
}

function buildRoots(paths: string[]): FileTreeNode[] {
  return paths.map((p) => ({
    path: p,
    name: p.split(/[\\/]/).pop() ?? p,
    isDirectory: true,
    isExpanded: false,
    isLoaded: false,
    depth: 0,
    children: null,
  }));
}

function defaultDeps(): UseFileTreeDeps {
  return {
    readDir: async (path: string): Promise<DirEntry[]> => {
      const contents = await resolveDirectoryContents(path);
      return contents.entries;
    },
  };
}