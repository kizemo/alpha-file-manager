<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

FORK-MODIFICATION: tree view. Keep during upstream sync.
Tracking issue: aleksey-hoffman/sigma-file-manager#499

v6: this component is now store-driven (no defineExpose, no template ref
forwarding). Expansion state lives in `useFolderTreeStore`; this component
just renders `rows` computed from `useFileTree({ expandedPaths, ... })`,
where `useFileTree` itself watches `expandedPaths` and lazy-loads children.
When `store.selectedPath` changes, ancestors are added to the dynamic root
list and loaded. Failures surface via `store.markLoadError` and the row
stays visible (just with no children). See handoff-2026-09-26-tree-sync-retro.
-->
<script setup lang="ts">
import { watch } from 'vue';
import { ChevronRightIcon, ChevronDownIcon, FolderIcon, FileIcon } from '@lucide/vue';
import { storeToRefs } from 'pinia';
import {
  computeAncestorPaths,
  useFolderTreeStore,
} from '@/stores/runtime/folder-tree';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

const props = withDefaults(defineProps<{
  /** Root directory paths (typically the drives from `useDrives()`).
   *  Ancestors of the selected path that are *not* in this list are still
   *  loadable — `useFileTree` walks up to a known ancestor in the tree
   *  instead of duplicating the path as a new root. This keeps the depth
   *  display correct (see handoff-2026-09-26-tree-sync-v6-3.md). */
  rootPaths?: string[];
}>(), {
  rootPaths: () => [],
});

const emit = defineEmits<{
  activate: [path: string];
}>();

const folderTreeStore = useFolderTreeStore();
const { expandedPaths, selectedPath } = storeToRefs(folderTreeStore);

const { rows, ensureAncestorsLoaded } = useFileTree({
  rootPaths: () => props.rootPaths,
  expandedPaths,
  onLoadStart: (path) => {
    folderTreeStore.markLoadError(path, false);
    folderTreeStore.markLoading(path, true);
  },
  onLoadEnd: (path) => {
    folderTreeStore.markLoading(path, false);
  },
  onLoadError: (path, err) => {
    folderTreeStore.markLoading(path, false);
    folderTreeStore.markLoadError(path, true);
    console.error('[file-browser-tree-view] failed to load', path, err);
  },
});

// Whenever the selected path changes, make sure every ancestor directory has
// its children loaded so the user sees the selected entry highlighted in its
// expanded ancestor chain.
watch(selectedPath, async (path) => {
  if (!path) return;
  await ensureAncestorsLoaded(computeAncestorPaths(path));
}, { immediate: true });

// v6.3: separate the chevron click (expand/collapse only) from the row
// click (emit activate → parent navigates). Per user feedback: clicking the
// icon should expand/collapse without changing the current path; clicking
// the name (or row body) should open/navigate.
function onChevronClick(row: { path: string }) {
  folderTreeStore.toggleExpanded(row.path);
}

function onRowClick(row: { path: string; isDirectory: boolean }) {
  emit('activate', row.path);
  // Don't manually toggle expand on name click — the store's
  // setSelectedPath already auto-expands ancestors when the parent
  // (navigator.vue) syncs the new selected path into the store.
}

function isRowSelected(path: string): boolean {
  return folderTreeStore.selectedPath === path;
}

function isRowLoading(path: string): boolean {
  return folderTreeStore.isLoading(path);
}

function isRowLoadError(path: string): boolean {
  return folderTreeStore.hasLoadError(path);
}
</script>

<template>
  <div class="file-tree-view" data-e2e-root="file-tree-view">
    <div
      v-for="row in rows"
      :key="row.path"
      class="file-tree-row"
      :class="{
        'file-tree-row--selected': isRowSelected(row.path),
        'file-tree-row--loading': isRowLoading(row.path),
        'file-tree-row--error': isRowLoadError(row.path),
      }"
      :style="{ paddingLeft: `${row.depth * 16 + 8}px` }"
      :aria-expanded="row.isDirectory ? row.isExpanded : undefined"
      :data-tree-path="row.path"
      :data-selected="isRowSelected(row.path) || undefined"
      :title="row.path"
      @click="onRowClick(row)"
    >
      <component
        :is="row.isDirectory && row.isExpanded ? ChevronDownIcon : ChevronRightIcon"
        v-if="row.isDirectory"
        :size="14"
        class="file-tree-row__chevron"
        @click.stop="onChevronClick(row)"
      />
      <span v-else class="file-tree-row__spacer" />
      <component
        :is="row.isDirectory ? FolderIcon : FileIcon"
        :size="14"
      />
      <span class="file-tree-row__name" :title="row.name">{{ row.name }}</span>
      <span
        v-if="isRowLoadError(row.path)"
        class="file-tree-row__error-marker"
        title="Failed to load children"
      >!</span>
    </div>
  </div>
</template>

<style scoped>
.file-tree-view {
  overflow-y: auto;
  height: 100%;
  font-size: 13px;
}

.file-tree-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px 2px 0;
  cursor: pointer;
  user-select: none;
}

.file-tree-row:hover {
  background-color: hsl(var(--muted) / 50%);
}

.file-tree-row--selected {
  background-color: hsl(var(--primary) / 18%);
}

.file-tree-row--loading {
  opacity: 0.6;
}

.file-tree-row__spacer {
  display: inline-block;
  width: 14px;
}

/* v6.3: chevron is a dedicated expand/collapse affordance — show its
 * own pointer cursor + hover background so the user can discover that
 * clicking it expands/collapses without navigating (the row body click
 * still navigates per the v6.2 contract). */
.file-tree-row__chevron {
  display: inline-flex;
  cursor: pointer;
  border-radius: 3px;
  padding: 1px;
  flex-shrink: 0;
}

.file-tree-row__chevron:hover {
  background-color: hsl(var(--muted) / 70%);
}

.file-tree-row__name {
  /* Keep one line per tree depth level so siblings align; long names clip
   * with ellipsis. The full path/name is also exposed via the `title`
   * attribute on the row + name span so hovering shows the whole thing in
   * a native tooltip — see handoff-2026-09-26-tree-sync-v6-2.md. */
  overflow-wrap: anywhere;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  min-width: 0;
  flex: 1;
}

.file-tree-row__error-marker {
  margin-left: auto;
  color: hsl(var(--destructive));
  font-weight: 700;
}
</style>
