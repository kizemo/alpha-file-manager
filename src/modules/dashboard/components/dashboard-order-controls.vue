<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<!-- FORK-MODIFICATION: 2026-10-31 —— 排序控件（拖拽手柄 + 上移 / 下移）。

     收藏分类与收藏条目两处都要能排序，所以抽成一个共用组件：
     两条交互路径（拖拽、上下按钮）落在**同一个组件**上，
     不会出现「分类能拖、条目只能按钮」这种半截实现。

     手柄用 HTML5 drag & drop：仪表盘收藏是多列 grid，指针事件那套
     （components/sortable-list）只比 Y 轴行中点，横向跨列会算错；
     HTML5 每张卡片各自是 drop target，不需要任何几何推算。 -->

<script setup lang="ts">
import { ArrowDownIcon, ArrowUpIcon, GripVerticalIcon } from '@lucide/vue';

const props = withDefaults(defineProps<{
  canMoveUp?: boolean;
  canMoveDown?: boolean;
  /** 写进 dataTransfer 的标识（收藏路径或分类 id）。 */
  dragKey: string;
  /** 拖拽手柄的无障碍名称。 */
  dragLabel: string;
  moveUpLabel: string;
  moveDownLabel: string;
  /** 拖拽时用作拖影的元素选择器。 */
  dragImageSelector?: string;
}>(), {
  canMoveUp: false,
  canMoveDown: false,
  dragImageSelector: '',
});

const emit = defineEmits<{
  'move-up': [];
  'move-down': [];
  'drag-start': [];
  'drag-end': [];
}>();

function handleGripClick(event: MouseEvent) {
  // 手柄不是操作入口：点它不应该把收藏条目打开（卡片整体是个 button）。
  event.preventDefault();
}

function handleDragStart(event: DragEvent) {
  if (!event.dataTransfer) {
    return;
  }

  event.dataTransfer.effectAllowed = 'move';
  event.dataTransfer.setData('text/plain', props.dragKey);

  const handle = event.currentTarget instanceof HTMLElement ? event.currentTarget : null;
  const dragImage = handle && props.dragImageSelector
    ? handle.closest(props.dragImageSelector)
    : null;

  if (dragImage instanceof HTMLElement) {
    event.dataTransfer.setDragImage(dragImage, 12, 12);
  }

  emit('drag-start');
}

function handleDragEnd() {
  emit('drag-end');
}
</script>

<template>
  <div class="dashboard-order-controls">
    <button
      type="button"
      class="dashboard-order-controls__grip"
      draggable="true"
      :data-drag-key="dragKey"
      :aria-label="dragLabel"
      :title="dragLabel"
      @click.stop="handleGripClick"
      @dragstart="handleDragStart"
      @dragend="handleDragEnd"
    >
      <GripVerticalIcon
        :size="14"
        :draggable="false"
      />
    </button>
    <button
      type="button"
      class="dashboard-order-controls__button"
      :disabled="!canMoveUp"
      :aria-label="moveUpLabel"
      :title="moveUpLabel"
      @click.stop="emit('move-up')"
    >
      <ArrowUpIcon :size="14" />
    </button>
    <button
      type="button"
      class="dashboard-order-controls__button"
      :disabled="!canMoveDown"
      :aria-label="moveDownLabel"
      :title="moveDownLabel"
      @click.stop="emit('move-down')"
    >
      <ArrowDownIcon :size="14" />
    </button>
  </div>
</template>

<style>
.dashboard-order-controls {
  display: flex;
  flex-shrink: 0;
  align-items: center;
  gap: 2px;
}

.dashboard-order-controls__grip,
.dashboard-order-controls__button {
  display: flex;
  width: 22px;
  height: 22px;
  align-items: center;
  justify-content: center;
  padding: 0;
  border: none;
  border-radius: 4px;
  background: transparent;
  color: hsl(var(--muted-foreground));
  cursor: pointer;
  opacity: 0.6;
  transition: opacity 0.15s, color 0.15s, background-color 0.15s;
}

.dashboard-order-controls__grip {
  cursor: grab;
}

.dashboard-order-controls__grip:hover,
.dashboard-order-controls__button:hover:not(:disabled) {
  background-color: hsl(var(--primary) / 10%);
  color: hsl(var(--primary));
  opacity: 1;
}

.dashboard-order-controls__grip:focus-visible,
.dashboard-order-controls__button:focus-visible {
  outline: 2px solid hsl(var(--ring) / 50%);
  outline-offset: var(--ring-outline-offset);
}

.dashboard-order-controls__button:disabled {
  color: hsl(var(--muted-foreground));
  cursor: not-allowed;
  opacity: 0.3;
}
</style>
