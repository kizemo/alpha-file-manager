<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { PencilIcon } from '@lucide/vue';
import QuickAccessItemIcon from './quick-access-item-icon.vue';
import { DirEntryInteractive } from '@/components/dir-entry-interactive';
import { resolveFavoriteDisplayName } from '@/utils/favorite-display-name';

const props = defineProps<{
  path: string;
  isFile: boolean;
  isCurrentDirectoryContext: boolean;
  /** FORK-MODIFICATION: 批次 37 —— 收藏显示名（别名）。只影响这里显示的名字。 */
  alias?: string;
  /** FORK-MODIFICATION: 批次 58 —— 是否显示小铅笔编辑按钮（只给收藏条目开）。 */
  editable?: boolean;
  /** FORK-MODIFICATION: 批次 58 —— 小铅笔按钮的无障碍标题。 */
  editLabel?: string;
}>();

const emit = defineEmits<{
  open: [];
  /** FORK-MODIFICATION: 批次 58 —— 点小铅笔 → 打开收藏编辑弹窗。 */
  edit: [];
}>();

const { t } = useI18n();

const itemName = computed(() => {
  // FORK-MODIFICATION: 批次 37 —— 与仪表盘收藏卡用同一个函数，
  // 保证两处显示的名字完全一致（有别名用别名，没有回退原始名称）。
  return resolveFavoriteDisplayName(props.path, props.alias, t);
});
</script>

<template>
  <DirEntryInteractive
    :path="props.path"
    :is-file="props.isFile"
    :is-current-directory-context="props.isCurrentDirectoryContext"
  >
    <div
      class="quick-access-panel__item"
      role="button"
      tabindex="0"
      @click="emit('open')"
      @keydown.enter.prevent="emit('open')"
      @keydown.space.prevent="emit('open')"
    >
      <QuickAccessItemIcon
        :path="props.path"
        :is-file="props.isFile"
        :size="14"
      />
      <span class="quick-access-panel__item-name">{{ itemName }}</span>

      <!-- FORK-MODIFICATION: 批次 58 —— 悬停时右侧的小铅笔。
           @click.stop 是必须的：外层是 div[role=button]，不拦住会连带触发「打开」。 -->
      <button
        v-if="props.editable"
        type="button"
        class="quick-access-panel__edit-button"
        :title="props.editLabel"
        :aria-label="props.editLabel"
        @click.stop="emit('edit')"
        @keydown.enter.stop.prevent="emit('edit')"
        @keydown.space.stop.prevent="emit('edit')"
      >
        <PencilIcon :size="12" />
      </button>
    </div>
  </DirEntryInteractive>
</template>

<style scoped>
.quick-access-panel__item {
  display: flex;
  width: 100%;
  align-items: center;
  padding: 6px 12px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  cursor: pointer;
  gap: 8px;
  text-align: start;
  transition: background-color 0.15s ease;
}

.quick-access-panel__item:hover {
  background-color: hsl(var(--foreground) / 5%);
}

.quick-access-panel__item-name {
  overflow: hidden;
  min-width: 0;
  flex: 1;
  color: hsl(var(--foreground));
  font-size: 0.8125rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* FORK-MODIFICATION: 批次 58 —— 小铅笔：平时隐着，悬停那条才出现。 */
.quick-access-panel__edit-button {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  padding: 2px;
  border: none;
  border-radius: var(--radius-sm);
  background: none;
  color: hsl(var(--muted-foreground));
  cursor: pointer;
  opacity: 0;
  transition: opacity 0.15s ease, color 0.15s ease;
}

.quick-access-panel__item:hover .quick-access-panel__edit-button,
.quick-access-panel__edit-button:focus-visible {
  opacity: 1;
}

.quick-access-panel__edit-button:hover {
  color: hsl(var(--foreground));
}
</style>

<style>
.dir-entry-interactive[data-drag-over] > .quick-access-panel__item {
  background-color: var(--drop-target-background);
  outline: var(--drop-target-outline);
  outline-offset: var(--drop-target-outline-offset);
}
</style>
