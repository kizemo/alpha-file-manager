// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: new store (tree sync v6). Keep during upstream sync.
// Tracking issue: aleksey-hoffman/sigma-file-manager#499
//
// Replaces the imperative "parent holds child ref + calls expandToPath" chain
// (broken in v0..v5, see handoff-2026-09-26-tree-sync-retro.md) with reactive
// shared state. Parent writes `selectedPath`; child (tree-view) reads it and
// reactively expands ancestors. No template ref, no async silent-fail.

import { defineStore } from 'pinia';
import { ref } from 'vue';

/**
 * Pure helper: return all ancestor directories of `path`, root-most first,
 * excluding the path itself.
 *
 * Examples (separator-agnostic: both `\` and `/` accepted; trailing slash
 * is normalized away):
 *   'C:/Users/foo'        → ['C:/', 'C:/Users']
 *   'C:\\Users\\foo'      → ['C:/', 'C:/Users']
 *   '/usr/local/bin'      → ['/usr', '/usr/local']
 *   'foo/bar/baz'         → ['foo', 'foo/bar']
 *   '/'                   → []
 *   ''                    → []
 */
export function computeAncestorPaths(path: string): string[] {
  if (!path) return [];
  const trimmed = path.replace(/\\/g, '/').replace(/\/+$/, '');
  if (!trimmed) return [];

  const ancestors: string[] = [];
  let current = trimmed;
  while (true) {
    const parent = parentDirectoryOf(current);
    if (parent === null || parent === current) break;
    // POSIX root '/' is not an "ancestor" to expand (no useful UI state).
    if (parent === '/') break;
    ancestors.unshift(parent);
    // Windows drive root 'C:/' is the last ancestor to add; stop after it.
    if (/^[A-Z]:\/$/i.test(parent)) break;
    current = parent;
  }
  return ancestors;
}

/** Return parent directory of `path`, or null if path is a root.
 *  Pure / path-string only — no FS access. */
function parentDirectoryOf(path: string): string | null {
  if (!path) return null;
  if (path === '/') return null;
  // Windows drive root: 'C:/', 'C:'
  if (/^[A-Z]:\/?$/i.test(path)) return null;

  const lastSlash = path.lastIndexOf('/');
  if (lastSlash === -1) return null;
  if (lastSlash === 0) return '/';

  let parent = path.substring(0, lastSlash);
  // Normalize Windows drive bare 'C:' to 'C:/' so equality checks line up
  if (/^[A-Z]:$/i.test(parent)) parent = `${parent}/`;
  return parent;
}

export const useFolderTreeStore = defineStore('folderTree', () => {
  /** Currently selected path (drives tree-view highlight + ancestor expansion). */
  const selectedPath = ref<string | null>(null);
  /** Set of directory paths the user has expanded in the tree. */
  const expandedPaths = ref<Set<string>>(new Set());
  /** Set of paths whose children are currently being loaded. */
  const loadingPaths = ref<Set<string>>(new Set());
  /** Set of paths whose children failed to load (permission denied, etc.). */
  const loadErrorPaths = ref<Set<string>>(new Set());

  function isExpanded(path: string): boolean {
    return expandedPaths.value.has(path);
  }

  function isLoading(path: string): boolean {
    return loadingPaths.value.has(path);
  }

  function hasLoadError(path: string): boolean {
    return loadErrorPaths.value.has(path);
  }

  /**
   * Set the selected path. Side effect: replace `expandedPaths` with the
   * chain from the drive/root down to (and including) the selected path so
   * the tree reveals the selection while everything else collapses back to
   * drive level. This is the v6.2 "single-active-path" policy — see
   * handoff-2026-09-26-tree-sync-v6-2.md. Manual expansions of unrelated
   * branches are cleared on every navigation.
   */
  function setSelectedPath(path: string | null): void {
    selectedPath.value = path;
    if (!path) {
      expandedPaths.value = new Set();
      return;
    }
    const ancestors = computeAncestorPaths(path);
    const next = new Set<string>(ancestors);
    next.add(path);
    expandedPaths.value = next;
  }

  function toggleExpanded(path: string): void {
    const next = new Set(expandedPaths.value);
    if (next.has(path)) {
      next.delete(path);
    }
    else {
      next.add(path);
    }
    expandedPaths.value = next;
  }

  function expandPath(path: string): void {
    if (expandedPaths.value.has(path)) return;
    const next = new Set(expandedPaths.value);
    next.add(path);
    expandedPaths.value = next;
  }

  function collapsePath(path: string): void {
    if (!expandedPaths.value.has(path)) return;
    const next = new Set(expandedPaths.value);
    next.delete(path);
    expandedPaths.value = next;
  }

  function markLoading(path: string, on: boolean): void {
    const next = new Set(loadingPaths.value);
    if (on) next.add(path);
    else next.delete(path);
    loadingPaths.value = next;
  }

  function markLoadError(path: string, on: boolean): void {
    const next = new Set(loadErrorPaths.value);
    if (on) next.add(path);
    else next.delete(path);
    loadErrorPaths.value = next;
  }

  function reset(): void {
    selectedPath.value = null;
    expandedPaths.value = new Set();
    loadingPaths.value = new Set();
    loadErrorPaths.value = new Set();
  }

  return {
    // state
    selectedPath,
    expandedPaths,
    loadingPaths,
    loadErrorPaths,
    // queries
    isExpanded,
    isLoading,
    hasLoadError,
    // mutations
    setSelectedPath,
    toggleExpanded,
    expandPath,
    collapsePath,
    markLoading,
    markLoadError,
    reset,
  };
});
