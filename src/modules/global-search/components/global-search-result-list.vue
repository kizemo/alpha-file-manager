<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import { useRouter } from 'vue-router';
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from '@/components/ui/context-menu';
import { toast } from '@/components/ui/toaster';
import FileBrowserActionsMenu from '@/modules/navigator/components/file-browser/file-browser-actions-menu.vue';
import type { ContextMenuAction } from '@/modules/navigator/components/file-browser/types';
import { useDirEntryActions } from '@/composables/use-dir-entry-actions';
import { openNavigatorRevealEntry } from '@/utils/open-navigator-directory';
import { revealEntriesContainingDirectory } from '@/utils/reveal-containing-directory';
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
  /** FORK-MODIFICATION (2026-11-05)：右键菜单里的「重命名」交给面板的改名对话框。 */
  rename: [entry: DirEntry];
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
    ...resolveThumbnailCssVariables(
      userSettingsStore.userSettings.navigator.thumbnailSize,
      // FORK-MODIFICATION: 字体大小三档（需求 3，2026-10-31）——绝对 px 偏移。
      userSettingsStore.fontSizeOffset,
    ),
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

/**
 * FORK-MODIFICATION (2026-11-05)：全局搜索结果行的右键菜单。
 *
 * 背景：`perf: 收敛渲染与索引的常驻开销（资源优化批次 C）`（bb003f97）把搜索结果
 * 从「导航器 FileBrowser（external 模式）」换成了这里的轻量列表，
 * 右键菜单与「打开所属目录」因此一起消失（2026-11-05 用户反馈：搜索结果里右键
 * 没有菜单弹出、「打开所在目录」也没了）。
 *
 * 这里把菜单补回来，**不**把 FileBrowser 塞回搜索面板 ——
 * 轻量列表是那次性能优化的成果，要保住。
 */
const router = useRouter();
const { handleContextMenuAction } = useDirEntryActions();

const contextMenuOpen = ref(false);
const contextMenuEntry = ref<DirEntry | null>(null);

/** 右键落在哪一行 —— 行处理器写入，菜单打开时消费。 */
let pendingContextEntry: DirEntry | null = null;

/**
 * 这几项动作在搜索结果面板里**没有执行通道**：
 * 「打开方式」「编辑标签」要各自的对话框，「新建」要落点目录。
 * 与其显示出来点了没反应，不如不显示（其余动作照常可用）。
 */
const EXCLUDED_ACTIONS: ContextMenuAction[] = [
  'open-with',
  'edit-tags',
  'create-file',
  'create-directory',
];

function handleRowContextMenu(entry: DirEntry) {
  pendingContextEntry = entry;
}

function handleContextMenuOpenChange(isOpen: boolean) {
  if (isOpen) {
    if (!pendingContextEntry) {
      // 右键落在行与行之间的空白处 —— 不弹菜单。
      contextMenuOpen.value = false;
      pendingContextEntry = null;

      return;
    }

    contextMenuEntry.value = pendingContextEntry;
  }
  else {
    contextMenuEntry.value = null;
  }

  pendingContextEntry = null;
  contextMenuOpen.value = isOpen;
}

function closeContextMenu() {
  contextMenuOpen.value = false;
  contextMenuEntry.value = null;
}

function handleAction(action: ContextMenuAction) {
  const entry = contextMenuEntry.value;

  if (!entry) {
    return;
  }

  closeContextMenu();

  if (action === 'rename') {
    emit('rename', entry);

    return;
  }

  if (action === 'open-containing-directory') {
    void revealEntriesContainingDirectory([entry], {
      router,
      filterQuery: '',
      onError: (error: unknown) => {
        toast.error(t('fileBrowser.actions.openContainingDirectoryFailed'), {
          description: error instanceof Error ? error.message : String(error),
        });
      },
    });

    return;
  }

  if (action === 'open-containing-directory-in-app') {
    // 搜索结果面板这一路：父目录显示在旁边的导航器 pane，面板本身保持打开
    // （`openNavigatorRevealEntry` 靠 `globalSearchStore.isOpen` 区分两条通道）。
    openNavigatorRevealEntry(router, entry.path, { filterQuery: '' });

    return;
  }

  handleContextMenuAction(action, [entry]);
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
  <ContextMenu
    :open="contextMenuOpen"
    @update:open="handleContextMenuOpenChange"
  >
    <ContextMenuTrigger as-child>
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
          @contextmenu="handleRowContextMenu(entry)"
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
    </ContextMenuTrigger>
    <ContextMenuContent
      v-if="contextMenuEntry"
      class="global-search-result-context-menu"
    >
      <FileBrowserActionsMenu
        :selected-entries="[contextMenuEntry]"
        :menu-item-component="ContextMenuItem"
        :menu-separator-component="ContextMenuSeparator"
        :is-search-results-view="true"
        :excluded-actions="EXCLUDED_ACTIONS"
        @action="handleAction"
      />
    </ContextMenuContent>
  </ContextMenu>
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
