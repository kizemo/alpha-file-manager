<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<!-- FORK-MODIFICATION 2026-10-23：移植自 upstream `3eca4c17`（仪表盘重设计）的「给卡片加信息」部分。
     三处改动，都不改变卡片原有的点击行为与插槽结构：
       1. `sizeLabel` —— 文件名右侧显示条目大小；
       2. 缩略图 —— 图片/视频进入视口后显示真实缩略图，没有缩略图就落回原来的图标；
       3. `#meta` 插槽 —— 路径下面多一行元信息（类型 + 相对时间）。
     upstream 那一版把整张卡片换成了「div + 绝对定位 hitbox」并引入了 list/grid 两种布局，
     那是骨架级替换，不在本次范围内。 -->

<!-- FORK-MODIFICATION: 批次 58（2026-11-25）—— 卡片上直接改收藏的**标题**与**路径**。

     两个要点：

     1. 路径显示方式由 `pathDisplay` 决定：`parent`（默认，只显示上级目录）与
        `full`（显示完整路径）。快速访问页的收藏卡用 `full`。
     2. `editable` 打开后，标题与路径可就地编辑（点一下变成输入框）。
        编辑框**不能**塞在 `<button>` 里 —— 按钮内的表单控件在 WebView2 里
        点不动也聚不了焦。所以卡片主体从 `<button>` 换成 `div[role=button]` +
        `tabindex`：语义不变、键盘照样能用，输入框也成了合法的子节点。
        全仓没有任何地方依赖它是一个真正的 <button>。 -->

<script setup lang="ts">
import { computed, nextTick, ref, useSlots } from 'vue';
import { useI18n } from 'vue-i18n';
import { FolderIcon, FileIcon, HardDriveIcon } from '@lucide/vue';
import { resolveFavoriteDisplayName } from '@/utils/favorite-display-name';
import { isVirtualLocationPath } from '@/utils/virtual-locations';
import { useDashboardEntryPreview } from '@/modules/dashboard/composables/use-dashboard-entry-preview';

const props = withDefaults(defineProps<{
  path: string;
  isFile?: boolean;
  sizeLabel?: string;
  /** FORK-MODIFICATION: 批次 37 —— 收藏显示名（别名）。只影响这里显示的名字。 */
  alias?: string;
  /** FORK-MODIFICATION: 批次 58 —— 路径显示方式。`full` = 完整路径，`parent` = 只显示上级目录。 */
  pathDisplay?: 'parent' | 'full';
  /** FORK-MODIFICATION: 批次 58 —— 标题 / 路径可就地编辑。 */
  editable?: boolean;
}>(), {
  sizeLabel: '',
  alias: '',
  pathDisplay: 'parent',
  editable: false,
});

const emit = defineEmits<{
  'click': [];
  /** FORK-MODIFICATION: 批次 58 —— 标题（别名）改好了。空串 = 清掉别名。 */
  'update:title': [value: string];
  /** FORK-MODIFICATION: 批次 58 —— 路径改好了。 */
  'update:path': [value: string];
}>();

const { t } = useI18n();
const slots = useSlots();
const rootRef = ref<HTMLElement | null>(null);

const hasFooter = computed(() => !!slots.footer);

const itemName = computed(() => {
  // FORK-MODIFICATION: 批次 37 —— 有别名就用别名，没有就回退到原始名称。
  return resolveFavoriteDisplayName(props.path, props.alias, t);
});

const itemDirectory = computed(() => {
  if (!props.path || isVirtualLocationPath(props.path)) return '';
  const lastSlashIndex = props.path.lastIndexOf('/');
  return lastSlashIndex > 0 ? props.path.substring(0, lastSlashIndex) : props.path;
});

// FORK-MODIFICATION: 批次 58 —— 完整路径 vs 上级目录。
// 需求是「收藏的标题下方显示完整路径」，所以收藏卡走 `full`；
// 标记 / 常用 / 历史仍是原来的 `parent`。
const pathText = computed(() => (
  props.pathDisplay === 'full' ? props.path : itemDirectory.value
));

const isDirectory = computed(() => {
  if (isVirtualLocationPath(props.path)) {
    return true;
  }

  if (props.isFile !== undefined) {
    return !props.isFile;
  }

  return props.path.endsWith('/') || !props.path.includes('.');
});

const showLocationsIcon = computed(() => isVirtualLocationPath(props.path));

const { thumbnailSrc } = useDashboardEntryPreview({
  path: () => props.path,
  isFile: () => !isDirectory.value,
  rootRef,
});

function handleClick() {
  emit('click');
}

// FORK-MODIFICATION: 批次 58 —— 就地编辑。
type EditableField = 'title' | 'path';

const editingField = ref<EditableField | null>(null);
const draftValue = ref('');
const titleInputRef = ref<HTMLInputElement | null>(null);
const pathInputRef = ref<HTMLInputElement | null>(null);

/** 虚拟位置（my.folder 之类）没有真实路径，路径不可编辑；标题始终可改。 */
const canEditTitle = computed(() => props.editable);
const canEditPath = computed(() => props.editable && !isVirtualLocationPath(props.path));

function startEditing(field: EditableField) {
  if (field === 'title' && !canEditTitle.value) {
    return;
  }

  if (field === 'path' && !canEditPath.value) {
    return;
  }

  editingField.value = field;
  // 预填**当前显示出来的那串字**：有别名就预填别名，没有就预填原始名称。
  // 存盘时由上层判断「这串字是不是原始名称」来决定是写别名还是清别名。
  draftValue.value = field === 'title' ? itemName.value : props.path;

  void nextTick(() => {
    const element = field === 'title' ? titleInputRef.value : pathInputRef.value;
    element?.focus();
    element?.select();
  });
}

function cancelEditing() {
  editingField.value = null;
  draftValue.value = '';
}

function commitEditing() {
  const field = editingField.value;

  if (!field) {
    return;
  }

  const value = draftValue.value.trim();

  editingField.value = null;
  draftValue.value = '';

  if (field === 'title') {
    emit('update:title', value);
    return;
  }

  emit('update:path', value);
}
</script>

<template>
  <div
    ref="rootRef"
    class="entry-card"
    :class="{ 'entry-card--with-footer': hasFooter }"
  >
    <!-- FORK-MODIFICATION: 批次 58 —— 主体由 <button> 改成 div[role=button]，
         这样标题 / 路径的输入框才是合法的子节点（按钮里的表单控件点不动）。 -->
    <div
      class="entry-card__main"
      role="button"
      tabindex="0"
      @click="handleClick"
      @keydown.enter.prevent="handleClick"
      @keydown.space.prevent="handleClick"
    >
      <div class="entry-card__icon">
        <img
          v-if="thumbnailSrc"
          class="entry-card__preview"
          :src="thumbnailSrc"
          alt=""
          draggable="false"
        >
        <HardDriveIcon
          v-else-if="showLocationsIcon"
          :size="20"
        />
        <FolderIcon
          v-else-if="isDirectory"
          :size="20"
        />
        <FileIcon
          v-else
          :size="20"
        />
      </div>
      <div class="entry-card__content">
        <div class="entry-card__heading">
          <input
            v-if="editingField === 'title'"
            ref="titleInputRef"
            v-model="draftValue"
            class="entry-card__input entry-card__input--title"
            type="text"
            @click.stop
            @keydown.stop
            @keydown.enter.prevent="commitEditing"
            @keydown.esc.prevent="cancelEditing"
            @blur="commitEditing"
          >
          <span
            v-else
            class="entry-card__name"
            :class="{ 'entry-card__name--editable': canEditTitle }"
            @click.stop="startEditing('title')"
          >{{ itemName }}</span>
          <span
            v-if="sizeLabel"
            class="entry-card__size"
          >{{ sizeLabel }}</span>
        </div>

        <input
          v-if="editingField === 'path'"
          ref="pathInputRef"
          v-model="draftValue"
          class="entry-card__input entry-card__input--path"
          type="text"
          @click.stop
          @keydown.stop
          @keydown.enter.prevent="commitEditing"
          @keydown.esc.prevent="cancelEditing"
          @blur="commitEditing"
        >
        <span
          v-else
          class="entry-card__path"
          :class="{ 'entry-card__path--editable': canEditPath }"
          @click.stop="startEditing('path')"
        >{{ pathText }}</span>

        <div
          v-if="slots.meta"
          class="entry-card__meta"
        >
          <slot name="meta" />
        </div>
      </div>
      <slot />
    </div>
    <div
      v-if="hasFooter"
      class="entry-card__footer"
    >
      <slot name="footer" />
    </div>
  </div>
</template>

<style>
.entry-card {
  position: relative;
  overflow: hidden;
  border-radius: var(--radius);
  background-color: hsl(var(--card));
  transition: background-color var(--hover-transition-duration-out) var(--hover-transition-easing-out);
}

.entry-card:hover {
  background-color: hsl(var(--muted));
  transition: background-color var(--hover-transition-duration-in);
}

.entry-card:hover .entry-card__action {
  opacity: 1;
  transition: opacity var(--hover-transition-duration-in);
}

.entry-card__main {
  display: flex;
  width: 100%;
  box-sizing: border-box;
  align-items: center;
  padding: 12px 16px;
  border: none;
  background: none;
  cursor: pointer;
  font: inherit;
  gap: 12px;
  text-align: left;
}

.entry-card__main:focus-visible {
  outline: 2px solid hsl(var(--primary));
  outline-offset: -2px;
}

.entry-card__icon {
  display: flex;
  overflow: hidden;
  width: 40px;
  height: 40px;
  flex-shrink: 0;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-sm);
  background-color: hsl(var(--primary) / 10%);
  color: hsl(var(--primary));
}

.entry-card__preview {
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.entry-card__content {
  display: flex;
  min-width: 0;
  flex: 1;
  flex-direction: column;
  gap: 4px;
}

.entry-card__heading {
  display: flex;
  min-width: 0;
  align-items: baseline;
  gap: 8px;
}

.entry-card__name {
  overflow: hidden;
  min-width: 0;
  flex: 1;
  color: hsl(var(--foreground));
  font-size: 0.9rem;
  font-weight: 500;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.entry-card__name--editable,
.entry-card__path--editable {
  cursor: text;
}

.entry-card__size {
  flex-shrink: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  font-variant-numeric: tabular-nums;
}

.entry-card__path {
  overflow: hidden;
  color: hsl(var(--muted-foreground));
  font-size: 0.8rem;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* FORK-MODIFICATION: 批次 58 —— 就地编辑的输入框。 */
.entry-card__input {
  box-sizing: border-box;
  padding: 2px 6px;
  border: 1px solid hsl(var(--primary));
  border-radius: var(--radius-sm);
  background-color: hsl(var(--card));
  color: hsl(var(--foreground));
  font: inherit;
}

.entry-card__input--title {
  min-width: 0;
  flex: 1;
  font-size: 0.9rem;
  font-weight: 500;
}

.entry-card__input--path {
  width: 100%;
  font-size: 0.8rem;
}

.entry-card__meta {
  display: flex;
  overflow: hidden;
  min-width: 0;
  align-items: center;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
  gap: 16px;
  white-space: nowrap;
}

.entry-card__footer {
  display: flex;
  width: 100%;
  min-width: 0;
  align-items: center;
  padding: 0 16px 8px;
}

.entry-card__action {
  flex-shrink: 0;
  opacity: 0;
  transition: opacity var(--hover-transition-duration-out) var(--hover-transition-easing-out);
}

.entry-card__action:hover {
  color: hsl(var(--destructive));
}

.entry-card__stats {
  display: flex;
  flex-shrink: 0;
  align-items: center;
}

.entry-card__badge {
  padding: 4px 10px;
  border-radius: var(--radius-sm);
  background-color: hsl(var(--muted));
  color: hsl(var(--muted-foreground));
  font-size: 0.8rem;
  font-weight: 500;
}

.entry-card__time {
  flex-shrink: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.8rem;
}
</style>
