<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import type { DirEntry } from '@/types/dir-entry';
import { useUserSettingsStore } from '@/stores/storage/user-settings';
import { formatBytes } from '@/modules/navigator/components/file-browser/utils';
import FileBrowserEntryIcon from '@/modules/navigator/components/file-browser/file-browser-entry-icon.vue';
import {
  getListColumnDefinition,
  getListColumnFlexGridTrack,
  getListColumnGridTrack,
} from '@/modules/navigator/components/file-browser/utils/file-browser-list-columns';
import { resolveThumbnailCssVariables } from '@/modules/navigator/components/file-browser/utils/file-browser-thumbnail-sizes';
import {
  formatAbsoluteDateDisplay,
  formatRelativeDateDisplay,
} from '@/utils/relative-date-display';
import {
  getGlobalSearchResultRowKey,
  type GlobalSearchNavigateDirection,
} from './global-search-result-rows';

const props = defineProps<{
  entries: DirEntry[];
  selectedPaths: Set<string>;
  focusedPath: string | null;
}>();

const emit = defineEmits<{
  select: [entry: DirEntry, additive: boolean];
  selectRange: [entry: DirEntry];
  openEntry: [entry: DirEntry];
  navigate: [direction: GlobalSearchNavigateDirection, extend: boolean];
  activate: [];
}>();

const { t, locale } = useI18n();
const userSettingsStore = useUserSettingsStore();

const listElementRef = ref<HTMLElement | null>(null);

/**
 * 相对时间的参照时刻。
 *
 * 与旧搜索面板的 `track-relative-time=false` 一致：不起 1 秒定时器，
 * 参照时刻就是组件创建的那一刻。
 */
const referenceNowMs = Date.now();

/**
 * 列宽模板：复用文件浏览器列表的列定义与轨道计算，
 * 只取「名称 / 大小 / 修改时间」三列，其余列一律不渲染。
 */
const columnGridTemplate = computed(() => {
  const settings = userSettingsStore.userSettings.navigator;
  const definitions = [
    getListColumnDefinition('name'),
    getListColumnDefinition('size'),
    getListColumnDefinition('modified'),
  ];

  return definitions
    .map((definition) => {
      if (definition.isFlex) {
        return getListColumnFlexGridTrack(definition, settings.listColumnFlexWeights[definition.id]);
      }

      return getListColumnGridTrack(definition, settings.listColumnWidths[definition.id]);
    })
    .join(' ');
});

const containerStyle = computed(() => {
  return {
    ...resolveThumbnailCssVariables(userSettingsStore.userSettings.navigator.thumbnailSize),
    '--global-search-result-list-columns': columnGridTemplate.value,
  };
});

function isEntrySelected(entry: DirEntry): boolean {
  return props.selectedPaths.has(entry.path);
}

function isEntryFocused(entry: DirEntry): boolean {
  return props.focusedPath === entry.path;
}

function getEntrySizeDisplay(entry: DirEntry): string {
  if (!entry.is_file) {
    return '—';
  }

  return formatBytes(entry.size);
}

function getEntryModifiedDisplay(entry: DirEntry): string {
  return formatRelativeDateDisplay({
    timestamp: entry.modified_time,
    referenceNowMs,
    dateTimeOptions: userSettingsStore.userSettings.dateTime,
    appLocale: locale.value,
    translate: t,
  });
}

function getEntryModifiedTitle(entry: DirEntry): string {
  return formatAbsoluteDateDisplay(
    entry.modified_time,
    userSettingsStore.userSettings.dateTime,
    locale.value,
  );
}

function handleEntryClick(entry: DirEntry, event: MouseEvent) {
  if (event.shiftKey) {
    emit('selectRange', entry);
    return;
  }

  emit('select', entry, event.ctrlKey || event.metaKey);
}

function handleEntryDoubleClick(entry: DirEntry) {
  emit('openEntry', entry);
}

function handleListKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter') {
    event.preventDefault();
    emit('activate');
    return;
  }

  let direction: GlobalSearchNavigateDirection | null = null;

  if (event.key === 'ArrowDown') {
    direction = 'next';
  }
  else if (event.key === 'ArrowUp') {
    direction = 'previous';
  }
  else if (event.key === 'Home') {
    direction = 'first';
  }
  else if (event.key === 'End') {
    direction = 'last';
  }

  if (!direction) {
    return;
  }

  event.preventDefault();
  emit('navigate', direction, event.shiftKey);
}

function scrollFocusedRowIntoView(index: number) {
  const container = listElementRef.value;

  if (!container) {
    return;
  }

  const row = container.querySelector(`[data-index="${index}"]`);

  if (row instanceof HTMLElement && typeof row.scrollIntoView === 'function') {
    row.scrollIntoView({ block: 'nearest' });
  }
}

watch(() => props.focusedPath, async (path) => {
  if (!path) {
    return;
  }

  const index = props.entries.findIndex(entry => entry.path === path);

  // 焦点落在别的盘符分组里时，本组不抢焦点、不滚动。
  if (index < 0) {
    return;
  }

  await nextTick();
  scrollFocusedRowIntoView(index);
  listElementRef.value?.focus({ preventScroll: true });
});
</script>

<template>
  <div
    ref="listElementRef"
    class="global-search-result-list"
    :style="containerStyle"
    role="listbox"
    tabindex="0"
    @keydown="handleListKeydown"
  >
    <div
      v-for="(entry, index) in props.entries"
      :key="getGlobalSearchResultRowKey(index)"
      class="global-search-result-list__row"
      :class="{
        'global-search-result-list__row--selected': isEntrySelected(entry),
        'global-search-result-list__row--focused': isEntryFocused(entry),
      }"
      :data-index="index"
      role="option"
      :aria-selected="isEntrySelected(entry)"
      @click="handleEntryClick(entry, $event)"
      @dblclick="handleEntryDoubleClick(entry)"
    >
      <div class="global-search-result-list__entry-name">
        <FileBrowserEntryIcon
          :entry="entry"
          :size="18"
          class="global-search-result-list__entry-icon"
          :class="{ 'global-search-result-list__entry-icon--folder': entry.is_dir }"
        />
        <div class="global-search-result-list__entry-name-content">
          <span class="global-search-result-list__entry-text">{{ entry.name }}</span>
          <span class="global-search-result-list__entry-description">{{ entry.path }}</span>
        </div>
      </div>
      <span class="global-search-result-list__entry-size">{{ getEntrySizeDisplay(entry) }}</span>
      <span
        class="global-search-result-list__entry-modified"
        :title="getEntryModifiedTitle(entry)"
      >{{ getEntryModifiedDisplay(entry) }}</span>
    </div>
  </div>
</template>

<style scoped>
.global-search-result-list {
  display: flex;
  overflow: hidden;
  min-width: 0;
  flex-direction: column;
  outline: none;
}

.global-search-result-list:focus-visible {
  outline: 2px solid hsl(var(--ring) / 50%);
  outline-offset: -2px;
}

.global-search-result-list__row {
  display: grid;
  overflow: hidden;
  min-width: 0;
  min-height: var(--navigator-list-view-entry-height, 28px);
  align-items: center;
  padding: 4px 8px;
  border-radius: var(--radius-sm);
  column-gap: 12px;
  cursor: default;
  grid-template-columns: var(--global-search-result-list-columns, minmax(200px, 4fr) minmax(60px, 1fr) minmax(120px, 1fr));
  user-select: none;
}

.global-search-result-list__row:hover {
  background-color: hsl(var(--accent) / 40%);
}

.global-search-result-list__row--selected {
  background-color: hsl(var(--primary) / 15%);
}

.global-search-result-list__row--focused {
  outline: 1px solid hsl(var(--primary) / 60%);
  outline-offset: -1px;
}

.global-search-result-list__entry-name {
  display: flex;
  overflow: hidden;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.global-search-result-list__entry-icon {
  flex-shrink: 0;
}

.global-search-result-list__entry-icon--folder {
  color: hsl(var(--primary));
}

.global-search-result-list__entry-name-content {
  display: flex;
  overflow: hidden;
  min-width: 0;
  flex-direction: column;
}

.global-search-result-list__entry-text {
  overflow: hidden;
  color: hsl(var(--foreground));
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.global-search-result-list__entry-description {
  overflow: hidden;
  color: hsl(var(--muted-foreground));
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 11px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.global-search-result-list__entry-size,
.global-search-result-list__entry-modified {
  overflow: hidden;
  color: hsl(var(--muted-foreground));
  font-size: 12px;
  text-align: right;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
