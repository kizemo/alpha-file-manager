<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved. -->

<!-- FORK-MODIFICATION: 批次 58（2026-11-25）—— 收藏编辑弹窗（标题 / 路径 / 分类）。

     两处入口共用这一个组件：
       1. 侧边栏「快速访问」悬停面板上的小铅笔按钮；
       2. 快速访问（仪表盘）收藏卡的 ⋮ 菜单里的「编辑收藏」。

     组件只负责**收集与校验**用户填的值，写盘一律交给 store ——
     这样弹窗本身不持有数据，出错时也不会出现「界面改了、盘上没改」的错位。 -->

<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
  SelectGroup,
} from '@/components/ui/select';
import { toast } from '@/components/ui/toaster';
import { useUserStatsStore } from '@/stores/storage/user-stats';
import { checkFavoritePath, normalizeFavoritePathInput } from '@/utils/favorite-edit';
import type { FavoriteItem } from '@/types/user-stats';

const UNCATEGORIZED_VALUE = '__afm_uncategorized__';

const props = defineProps<{
  open: boolean;
  /** 要编辑的收藏条目；null 表示没有目标（弹窗不该打开）。 */
  favorite: FavoriteItem | null;
}>();

const emit = defineEmits<{
  'update:open': [value: boolean];
}>();

const { t } = useI18n();
const userStatsStore = useUserStatsStore();

const titleDraft = ref('');
const pathDraft = ref('');
const categoryDraft = ref<string>(UNCATEGORIZED_VALUE);
const isSaving = ref(false);
const pathError = ref('');
const pathInputRef = ref<InstanceType<typeof Input> | null>(null);

const categories = computed(() => userStatsStore.favoriteCategories);

watch(
  () => props.open,
  (open) => {
    if (!open) {
      return;
    }

    const favorite = props.favorite;

    titleDraft.value = favorite?.alias ?? '';
    pathDraft.value = favorite?.path ?? '';
    categoryDraft.value = favorite?.categoryId ?? UNCATEGORIZED_VALUE;
    pathError.value = '';
    isSaving.value = false;

    nextTick(() => {
      const host = pathInputRef.value?.$el;
      const element = host instanceof HTMLInputElement ? host : host?.querySelector('input');
      element?.focus();
      element?.select();
    });
  },
  { immediate: true },
);

async function handleSubmit() {
  const favorite = props.favorite;

  if (!favorite || isSaving.value) {
    return;
  }

  const path = normalizeFavoritePathInput(pathDraft.value);

  if (!path) {
    pathError.value = t('dashboard.favoriteEdit.pathRequired');
    return;
  }

  isSaving.value = true;
  pathError.value = '';

  try {
    // 路径变了才做存在性 / 重复判定 —— 路径没变时这两条检查都是白跑。
    if (path !== favorite.path) {
      const otherPaths = userStatsStore.favorites
        .filter(item => item.path !== favorite.path)
        .map(item => item.path);

      const result = await checkFavoritePath(path, otherPaths);

      if (result.status === 'missing') {
        pathError.value = t('dashboard.newFavoriteDialog.pathNotFound');
        return;
      }

      if (result.status === 'duplicate') {
        pathError.value = t('dashboard.newFavoriteDialog.pathAlreadyFavorite');
        return;
      }

      await userStatsStore.updateFavoritePath(favorite.path, path, result.isFile);
    }

    const title = titleDraft.value.trim();

    if (title !== (favorite.alias ?? '').trim()) {
      await userStatsStore.setFavoriteAlias(path, title);
    }

    const categoryId = categoryDraft.value === UNCATEGORIZED_VALUE
      ? null
      : categoryDraft.value;

    if (categoryId !== favorite.categoryId) {
      await userStatsStore.moveFavoriteToCategory(path, categoryId);
    }

    toast.success(t('dashboard.favoriteEdit.saved'));
    emit('update:open', false);
  }
  catch (error) {
    console.error('Failed to save favorite edit:', error);
    toast.error(t('dashboard.favoriteEdit.saveFailed'));
  }
  finally {
    isSaving.value = false;
  }
}
</script>

<template>
  <Dialog
    :open="open"
    @update:open="emit('update:open', $event)"
  >
    <DialogContent class="favorite-edit-dialog">
      <DialogHeader>
        <DialogTitle>{{ t('dashboard.actions.editFavorite') }}</DialogTitle>
      </DialogHeader>

      <div class="favorite-edit-dialog__form">
        <label
          class="favorite-edit-dialog__label"
          for="favorite-edit-title"
        >{{ t('dashboard.newFavoriteDialog.displayNameLabel') }}</label>

        <Input
          id="favorite-edit-title"
          v-model="titleDraft"
          :placeholder="t('dashboard.newFavoriteDialog.displayNamePlaceholder')"
          @keydown.enter.prevent="handleSubmit"
        />

        <label
          class="favorite-edit-dialog__label"
          for="favorite-edit-path"
        >{{ t('dashboard.newFavoriteDialog.pathLabel') }}</label>

        <Input
          id="favorite-edit-path"
          ref="pathInputRef"
          v-model="pathDraft"
          :placeholder="t('dashboard.newFavoriteDialog.pathPlaceholder')"
          :aria-invalid="Boolean(pathError)"
          @keydown.enter.prevent="handleSubmit"
        />

        <p
          v-if="pathError"
          class="favorite-edit-dialog__hint"
        >
          {{ pathError }}
        </p>

        <label class="favorite-edit-dialog__label">
          {{ t('dashboard.favoriteCategoryDialog.categoryLabel') }}
        </label>

        <Select v-model="categoryDraft">
          <SelectTrigger class="favorite-edit-dialog__select">
            <SelectValue :placeholder="t('dashboard.actions.uncategorized')" />
          </SelectTrigger>
          <SelectContent>
            <SelectGroup>
              <SelectItem :value="UNCATEGORIZED_VALUE">
                {{ t('dashboard.actions.uncategorized') }}
              </SelectItem>
              <SelectItem
                v-for="category in categories"
                :key="category.id"
                :value="category.id"
              >
                {{ category.name }}
              </SelectItem>
            </SelectGroup>
          </SelectContent>
        </Select>
      </div>

      <DialogFooter>
        <Button
          variant="outline"
          :disabled="isSaving"
          @click="emit('update:open', false)"
        >
          {{ t('cancel') }}
        </Button>
        <Button
          :disabled="isSaving"
          @click="handleSubmit"
        >
          {{ t('save') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<style>
.favorite-edit-dialog {
  width: 520px;
  max-width: calc(100vw - 32px);
  box-sizing: border-box;
  overflow-x: hidden;
}

.favorite-edit-dialog > * {
  min-width: 0;
}

.favorite-edit-dialog__form {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  gap: 8px;
}

.favorite-edit-dialog__label {
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
}

.favorite-edit-dialog__hint {
  margin: 0;
  color: hsl(var(--muted-foreground));
  font-size: 0.75rem;
}

.favorite-edit-dialog__select {
  width: 100%;
}
</style>
