// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: tree view. Keep during upstream sync.
// Tracking issue: aleksey-hoffman/sigma-file-manager#499
//
// v6 rewrite: expansion state is no longer owned by this composable. The
// caller (typically a Pinia store such as `useFolderTreeStore`) provides a
// reactive `expandedPaths: Ref<Set<string>>`, and `rows` is computed by
// intersecting the nodes tree with that set. Newly expanded paths trigger
// async `ensureLoaded` via a side-effect watch; failures are surfaced via
// `onLoadError` and the UI keeps rendering (the row stays visible, just with
// no children). This breaks the imperative "parent calls child.expandToPath"
// chain that was unfixable in v0..v5 — see handoff-2026-09-26-tree-sync-retro.

import {
  computed,
  ref,
  toValue,
  watch,
  type ComputedRef,
  type MaybeRefOrGetter,
  type Ref,
} from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import { resolveDirectoryContents } from '@/utils/virtual-locations';
import type { FileTreeNode, FileTreeFlatRow } from '../types/file-tree';

export interface UseFileTreeDeps {
  readDir: (path: string) => Promise<DirEntry[]>;
}

export interface UseFileTreeOptions {
  /**
   * Root directory paths. Can be reactive (Ref or getter) so the caller can
   * add or remove roots as navigation state changes. The composable rebuilds
   * its internal nodes tree when this changes; existing loaded children are
   * preserved by path key so a path that's still a root keeps its loaded
   * subtree.
   */
  rootPaths: MaybeRefOrGetter<string[]>;
  /**
   * Reactive set of currently expanded directory paths.
   * The caller owns this state — typically `storeToRefs(folderTreeStore).expandedPaths`.
   */
  expandedPaths: MaybeRefOrGetter<Set<string>>;
  /** Called when a directory's children start loading. */
  onLoadStart?: (path: string) => void;
  /** Called when a directory's children finish loading successfully. */
  onLoadEnd?: (path: string) => void;
  /** Called when a directory's children fail to load (e.g. permission denied). */
  onLoadError?: (path: string, err: unknown) => void;
  deps?: UseFileTreeDeps;
}

export interface UseFileTreeApi {
  /** Flat row list (for virtual scroll rendering). */
  rows: ComputedRef<FileTreeFlatRow[]>;
  /** Read-only view of which directory paths currently have their children loaded. */
  loadedPaths: ComputedRef<Set<string>>;
  /**
   * Ensure a directory's children are loaded. Idempotent — no-op if already
   * loaded. Errors are caught and reported via `onLoadError`. If `path` is
   * not in the current root set, it is added as a new root (ancestor
   * expansion case — store auto-expands an ancestor that's not yet a root).
   */
  ensureLoaded: (path: string) => Promise<void>;
  /** Sequentially `ensureLoaded` each ancestor path. Failures are isolated per-path. */
  ensureAncestorsLoaded: (ancestorPaths: string[]) => Promise<void>;
  /** Add a path as a root if it isn't already one. Returns true if added. */
  addRoot: (path: string) => boolean;
}

export function useFileTree(options: UseFileTreeOptions): UseFileTreeApi {
  const deps = options.deps ?? defaultDeps();
  const nodes = ref<FileTreeNode[]>([]);
  const loadedSet = ref<Set<string>>(new Set());
  const rootSet = ref<Set<string>>(new Set());

  // Initialize nodes from current rootPaths value.
  initializeNodes(toValue(options.rootPaths));

  // Rebuild nodes if the caller's rootPaths changes (e.g. user navigates to a
  // new drive). Loaded children that are still reachable are preserved.
  watch(
    () => toValue(options.rootPaths),
    (next) => rebuildRoots(next),
    { flush: 'post' },
  );

  // Side-effect: any newly expanded path triggers an async load. Immediate
  // so that callers passing an initial expanded set get a sync first pass.
  watch(
    () => toValue(options.expandedPaths),
    (next) => {
      for (const p of next) {
        void ensureLoaded(p);
      }
    },
    { deep: true, flush: 'post', immediate: true },
  );

  function addRoot(path: string): boolean {
    if (rootSet.value.has(path)) return false;
    const newRoot: FileTreeNode = {
      path,
      name: path.split(/[\\/]/).pop() ?? path,
      isDirectory: true,
      isExpanded: false,
      isLoaded: false,
      depth: 0,
      children: null,
    };
    nodes.value = [...nodes.value, newRoot];
    rootSet.value = new Set(rootSet.value).add(path);
    return true;
  }

  function findNodeIndex(list: FileTreeNode[], path: string): number {
    return list.findIndex(n => n.path === path);
  }

  function findNode(list: FileTreeNode[], path: string): FileTreeNode | null {
    function walk(items: FileTreeNode[]): FileTreeNode | null {
      for (const n of items) {
        if (n.path === path) return n;
        if (n.children) {
          const found = walk(n.children);
          if (found) return found;
        }
      }
      return null;
    }
    return walk(list);
  }

  async function loadChildren(node: FileTreeNode): Promise<void> {
    if (!node.isDirectory || node.isLoaded) return;
    const entries = await deps.readDir(node.path);
    node.children = entries.map((entry) => ({
      path: entry.path,
      name: entry.name,
      isDirectory: entry.is_dir,
      isExpanded: false,
      isLoaded: !entry.is_dir,
      depth: node.depth + 1,
      children: null,
    }));
    node.isLoaded = true;
  }

  /** Pure helper: return parent directory of `path`, or null if root. */
  function parentDirectoryOfPath(path: string): string | null {
    if (!path) return null;
    if (path === '/') return null;
    if (/^[A-Z]:\/?$/i.test(path)) return null;
    const lastSlash = path.lastIndexOf('/');
    if (lastSlash === -1) return null;
    if (lastSlash === 0) return '/';
    let parent = path.substring(0, lastSlash);
    if (/^[A-Z]:$/i.test(parent)) parent = `${parent}/`;
    return parent;
  }

  /**
   * v6.3: ensure `path` is in the tree AND its children are loaded.
   *
   * The previous version called `addRoot` for any path that wasn't already
   * a root — that produced duplicate depth-0 nodes when ancestors got
   * auto-expanded (e.g. `selectedPath = "E:/办公文件/002内衣项目"`
   * caused `办公文件` and `002内衣项目` to be added as standalone roots
   * alongside `E:/`, breaking the depth display — see
   * handoff-2026-09-26-tree-sync-v6-3.md).
   *
   * New behavior: if `path` isn't in the tree, walk up to find the closest
   * ancestor that IS in the tree, then load that ancestor's children so
   * `path` gets discovered as a descendant. If no ancestor is in the
   * tree (genuinely new root, e.g. a path outside any drive), fall back
   * to `addRoot`.
   */
  async function ensureLoaded(path: string): Promise<void> {
    if (loadedSet.value.has(path)) return;

    let node = findNode(nodes.value, path);
    if (!node) {
      // Walk up to the nearest ancestor already in the tree and load it
      // so `path` shows up as a child after.
      let cur = path;
      while (true) {
        const parent = parentDirectoryOfPath(cur);
        if (parent === null || parent === cur) break;
        const parentNode = findNode(nodes.value, parent);
        if (parentNode) {
          // Reload parent — this populates parent.children which should
          // include `path` as one entry.
          await loadChildrenOfNode(parentNode);
          // Re-find `path` now that the parent has been loaded.
          node = findNode(nodes.value, path);
          break;
        }
        cur = parent;
      }

      // If still not in tree, fall back to addRoot (genuinely new root).
      if (!node) {
        addRoot(path);
        node = findNode(nodes.value, path);
      }
    }

    if (!node?.isDirectory) return;

    let didLoad = false;
    options.onLoadStart?.(path);
    try {
      await loadChildren(node);
      didLoad = true;
      options.onLoadEnd?.(path);
    }
    catch (err) {
      options.onLoadError?.(path, err);
    }
    finally {
      // Always mark as "attempted" — successful load or surfaced error.
      loadedSet.value = new Set(loadedSet.value).add(path);
      // Ensure rows re-render even if load failed (so the user sees the row
      // without children rather than a permanent loading indicator).
      if (!didLoad) {
        nodes.value = [...nodes.value];
      }
    }
  }

  /** Internal: load children of an already-existing node, bypassing the
   *  ancestor walk. Used when ensureLoaded needs to populate a parent's
   *  children to discover the actual target path as a descendant. */
  async function loadChildrenOfNode(node: FileTreeNode): Promise<void> {
    if (node.isLoaded || !node.isDirectory) return;
    options.onLoadStart?.(node.path);
    try {
      await loadChildren(node);
      options.onLoadEnd?.(node.path);
    }
    catch (err) {
      options.onLoadError?.(node.path, err);
    }
    finally {
      loadedSet.value = new Set(loadedSet.value).add(node.path);
      // Trigger re-render so newly-discovered children appear in rows().
      nodes.value = [...nodes.value];
    }
  }

  async function ensureAncestorsLoaded(ancestorPaths: string[]): Promise<void> {
    for (const a of ancestorPaths) {
      await ensureLoaded(a);
    }
  }

  const loadedPaths = computed<Set<string>>(() => loadedSet.value);

  const rows = computed<FileTreeFlatRow[]>(() => {
    const expanded = toValue(options.expandedPaths);
    const out: FileTreeFlatRow[] = [];
    function walk(list: FileTreeNode[]) {
      for (const n of list) {
        const isExpanded = expanded.has(n.path);
        out.push({
          path: n.path,
          name: n.name,
          depth: n.depth,
          isDirectory: n.isDirectory,
          hasChildren: n.isDirectory,
          isExpanded,
        });
        if (isExpanded && n.children) walk(n.children);
      }
    }
    walk(nodes.value);
    return out;
  });

  function initializeNodes(paths: string[]) {
    rootSet.value = new Set(paths);
    nodes.value = buildRoots(paths);
  }

  function rebuildRoots(paths: string[]) {
    const nextRoots = new Set(paths);
    const next: FileTreeNode[] = [];
    const previousRoots = rootSet.value;

    for (const p of paths) {
      if (previousRoots.has(p)) {
        // Preserve existing loaded root subtree.
        const idx = findNodeIndex(nodes.value, p);
        if (idx >= 0) {
          next.push(nodes.value[idx]);
          continue;
        }
      }
      next.push(...buildRoots([p]));
    }

    rootSet.value = nextRoots;
    nodes.value = next;
  }

  return {
    rows,
    loadedPaths,
    ensureLoaded,
    ensureAncestorsLoaded,
    addRoot,
  };
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
