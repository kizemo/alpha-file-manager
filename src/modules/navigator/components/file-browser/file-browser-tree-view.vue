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
import { ref, watch } from 'vue';
import { ChevronRightIcon, ChevronDownIcon, FolderIcon, FileIcon } from '@lucide/vue';
import { storeToRefs } from 'pinia';
import {
  computeAncestorPaths,
  useFolderTreeStore,
} from '@/stores/runtime/folder-tree';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

const props = withDefaults(defineProps<{
  /** Initial root paths. Subsequent changes are picked up via `expandedPaths`. */
  rootPaths?: string[];
}>(), {
  rootPaths: () => [],
});

const emit = defineEmits<{
  activate: [path: string];
}>();

const folderTreeStore = useFolderTreeStore();
const { expandedPaths, selectedPath } = storeToRefs(folderTreeStore);

// Dynamic root list: starts with caller-provided roots, grows as ancestors of
// the selected path need to be loaded (e.g. user navigates to a deep folder
// that wasn't in the initial root set).
const dynamicRoots = ref<string[]>([...props.rootPaths]);

watch(() => props.rootPaths, (next) => {
  const merged = [...next];
  for (const r of dynamicRoots.value) {
    if (!merged.includes(r)) merged.push(r);
  }
  dynamicRoots.value = merged;
});

watch(selectedPath, (path) => {
  if (!path) return;
  const ancestors = computeAncestorPaths(path);
  if (ancestors.length === 0) return;
  const next = [...dynamicRoots.value];
  let changed = false;
  for (const a of ancestors) {
    if (!next.includes(a)) {
      next.push(a);
      changed = true;
    }
  }
  if (changed) dynamicRoots.value = next;
}, { immediate: true });

const { rows, ensureAncestorsLoaded } = useFileTree({
  rootPaths: dynamicRoots,
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
// expanded ancestor chain. This is the v6 replacement for the v0..v5
// imperative `treeViewRef.value?.expandToPath(path)` dance.
watch(selectedPath, async (path) => {
  if (!path) return;
  await ensureAncestorsLoaded(computeAncestorPaths(path));
}, { immediate: true });

// v6.2: a click on a tree row both navigates and (for directories) toggles
// expansion. This matches the user's "single click = expand + jump" flow
// (see handoff-2026-09-26-tree-sync-v6-2.md). Files navigate to their
// parent directory; the parent decides what to do with the activation
// (typically pane navigation).
function onClick(row: { path: string; isDirectory: boolean }) {
  emit('activate', row.path);
  if (row.isDirectory) {
    folderTreeStore.toggleExpanded(row.path);
  }
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
      @click="onClick(row)"
    >
      <component
        :is="row.isDirectory && row.isExpanded ? ChevronDownIcon : ChevronRightIcon"
        v-if="row.isDirectory"
        :size="14"
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
