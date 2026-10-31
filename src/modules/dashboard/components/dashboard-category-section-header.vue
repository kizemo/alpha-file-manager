<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<!-- FORK-MODIFICATION: 需求反馈 4（2026-10-15）——收藏分类的分区标题。
     「未分类」这一组不显示编辑按钮：它不是一个真实分类，没有 id 可改名/删除，
     给它挂一套无效操作比不显示更糟。

     FORK-MODIFICATION: 2026-10-31 —— 分区标题加上排序入口（拖拽手柄 + 上移 / 下移）。
     「未分类」同样不显示排序控件：它是虚拟分组，永远排在最后，没有可换位的对象。 -->

<script setup lang="ts">
import { computed } from 'vue';
import { useI18n } from 'vue-i18n';
import { FolderTreeIcon, PencilIcon, Trash2Icon } from '@lucide/vue';
import { Button } from '@/components/ui/button';
import DashboardOrderControls from '@/modules/dashboard/components/dashboard-order-controls.vue';
import type { FavoriteCategory } from '@/types/user-stats';

const props = defineProps<{
  name: string;
  count: number;
  category: FavoriteCategory | null;
  canMoveUp?: boolean;
  canMoveDown?: boolean;
}>();

const emit = defineEmits<{
  'rename-category': [category: FavoriteCategory];
  'delete-category': [category: FavoriteCategory];
  'move-up': [category: FavoriteCategory];
  'move-down': [category: FavoriteCategory];
  'drag-start': [category: FavoriteCategory];
  'drag-end': [category: FavoriteCategory];
}>();

const { t } = useI18n();

const moveUpLabel = computed(() => t('dashboard.actions.moveUpWithName', { name: props.name }));
const moveDownLabel = computed(() => t('dashboard.actions.moveDownWithName', { name: props.name }));
const dragLabel = computed(() => t('dashboard.actions.dragToReorder'));
</script>

<template>
  <div class="dashboard-category-section-header">
    <!-- FORK-MODIFICATION: 反馈 1（2026-10-15）——分类标题原本也是 FolderIcon，
         与下方目录卡片的兜底图标同一个组件，分类和目录分不出来。
         换成 FolderTreeIcon 后仪表盘收藏区同样是三种可区分图标：
         分类 = FolderTreeIcon、目录 = FolderIcon、文件 = 按扩展名的文件图标。 -->
    <FolderTreeIcon :size="14" />
    <span class="dashboard-category-section-header__name">{{ name }}</span>
    <span class="dashboard-category-section-header__count">{{ count }}</span>

    <div
      v-if="category"
      class="dashboard-category-section-header__actions"
    >
      <!-- FORK-MODIFICATION: 2026-10-31 —— 分类排序（拖拽 + 上移 / 下移）。 -->
      <DashboardOrderControls
        :can-move-up="canMoveUp"
        :can-move-down="canMoveDown"
        :drag-key="category.id"
        :drag-label="dragLabel"
        :move-up-label="moveUpLabel"
        :move-down-label="moveDownLabel"
        drag-image-selector=".dashboard-category-section-header"
        @move-up="emit('move-up', category)"
        @move-down="emit('move-down', category)"
        @drag-start="emit('drag-start', category)"
        @drag-end="emit('drag-end', category)"
      />
      <Button
        variant="ghost"
        size="icon"
        class="dashboard-category-section-header__button"
        :title="t('dashboard.actions.renameCategory')"
        @click="emit('rename-category', category)"
      >
        <PencilIcon :size="14" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        class="dashboard-category-section-header__button"
        :title="t('dashboard.actions.deleteCategory')"
        @click="emit('delete-category', category)"
      >
        <Trash2Icon :size="14" />
      </Button>
    </div>
  </div>
</template>

<style>
.dashboard-category-section-header {
  display: flex;
  width: 100%;
  align-items: center;
  padding: 4px 2px;
  color: hsl(var(--muted-foreground));
  gap: 6px;
}

.dashboard-category-section-header__name {
  color: hsl(var(--foreground));
  font-size: 0.875rem;
  font-weight: 500;
}

.dashboard-category-section-header__count {
  font-size: 0.75rem;
}

.dashboard-category-section-header__actions {
  display: flex;
  align-items: center;
  margin-left: auto;
  gap: 2px;
}

.dashboard-category-section-header__button {
  width: 26px;
  height: 26px;
}
</style>
