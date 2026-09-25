<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

FORK-MODIFICATION: new file (tree view). Keep during upstream sync.
Tracking issue: aleksey-hoffman/sigma-file-manager#499
-->
<script setup lang="ts">
import { computed } from 'vue';
import { ChevronRightIcon, ChevronDownIcon, FolderIcon, FileIcon } from '@lucide/vue';
import { useFileTree } from '@/modules/navigator/composables/use-file-tree';

const props = defineProps<{
  rootPaths: string[];
}>();

const emit = defineEmits<{
  activate: [path: string];
}>();

const { rows, toggle } = useFileTree({ rootPaths: props.rootPaths });

function onClick(row: { path: string; isDirectory: boolean; isExpanded: boolean }) {
  if (row.isDirectory) {
    void toggle(row.path);
  } else {
    emit('activate', row.path);
  }
}

const expandedSet = computed(() => new Set(rows.value.filter(r => r.isExpanded).map(r => r.path)));
</script>

<template>
  <div class="file-tree-view" data-e2e-root="file-tree-view">
    <div
      v-for="row in rows"
      :key="row.path"
      class="file-tree-row"
      :style="{ paddingLeft: `${row.depth * 16 + 8}px` }"
      :aria-expanded="row.isDirectory ? row.isExpanded : undefined"
      :data-tree-path="row.path"
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
      <span class="file-tree-row__name">{{ row.name }}</span>
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

.file-tree-row__spacer {
  display: inline-block;
  width: 14px;
}

.file-tree-row__name {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>