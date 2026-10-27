<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import {
  computed, nextTick, onBeforeUnmount, onMounted, watch,
} from 'vue';
import { useI18n } from 'vue-i18n';
import type { DirEntry } from '@/types/dir-entry';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogFooter,
} from '@/components/ui/dialog';
import { Button } from '@/components/ui/button';
import { getPathDisplayValue } from '@/utils/normalize-path';
import {
  focusPermanentDeleteConfirmContent,
  isPermanentDeleteConfirmInteractiveTarget,
  shouldConfirmPermanentDeleteOnEnter,
} from './utils/permanent-delete-confirm-keyboard';

const props = defineProps<{
  open: boolean;
  entries: DirEntry[];
}>();

const emit = defineEmits<{
  'update:open': [boolean];
  'confirm': [];
}>();

const { t } = useI18n();

const singlePathDisplay = computed(() => {
  if (props.entries.length !== 1) {
    return '';
  }

  return getPathDisplayValue(props.entries[0].path);
});

const bodyPrimaryMultiple = computed(() => {
  if (props.entries.length <= 1) {
    return '';
  }

  return t('dialogs.deleteIrreversibly.description', {
    count: props.entries.length,
  });
});

function onRequestClose() {
  emit('update:open', false);
}

function onConfirm() {
  emit('confirm');
}

// FORK-MODIFICATION 2026-10-27：裸回车 = 确认删除。
// reka-ui 的 FocusScope 在对话框挂载时把焦点落到内容里第一个可聚焦控件上，
// 而本对话框里那个控件是「取消」按钮 ⇒ 「刚打开就回车」= 取消，不是删除。
// 因此对话框打开后把焦点挪到对话框容器（容器不是可交互控件）上，
// 裸回车落到「焦点不在控件上 ⇒ 确认删除」的兜底分支；
// 用户 Tab 之后焦点在按钮上时，仍然交给浏览器原生行为。
watch(
  () => props.open,
  async (open) => {
    if (!open) {
      return;
    }

    await nextTick();

    // 等一拍再挪：reka-ui 的 FocusScope 在挂载之后还会再抢一次焦点。
    setTimeout(() => {
      if (props.open) {
        focusPermanentDeleteConfirmContent();
      }
    }, 0);
  },
);

function handleDocumentKeydown(event: KeyboardEvent) {
  if (!props.open) {
    return;
  }

  const activeElement = typeof document !== 'undefined' ? document.activeElement : null;
  const shouldConfirm = shouldConfirmPermanentDeleteOnEnter({
    key: event.key,
    defaultPrevented: event.defaultPrevented,
    isComposing: event.isComposing,
    isFocusOnInteractiveControl: isPermanentDeleteConfirmInteractiveTarget(activeElement),
  });

  if (!shouldConfirm) {
    return;
  }

  // capture 阶段拦截：别让应用级的回车快捷键同时生效。
  event.preventDefault();
  event.stopPropagation();
  onConfirm();
}

onMounted(() => {
  document.addEventListener('keydown', handleDocumentKeydown, true);
});

onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleDocumentKeydown, true);
});
</script>

<template>
  <Dialog
    :open="props.open"
    @update:open="emit('update:open', $event)"
  >
    <DialogContent class="permanent-delete-confirm-dialog">
      <DialogHeader>
        <DialogTitle>{{ t('dialogs.deleteIrreversibly.title') }}</DialogTitle>
        <DialogDescription class="permanent-delete-confirm-dialog__body">
          <span
            v-if="props.entries.length === 1"
            class="permanent-delete-confirm-dialog__primary"
          >
            <i18n-t
              keypath="dialogs.deleteIrreversibly.descriptionSingle"
              tag="span"
            >
              <template #path>
                <code class="permanent-delete-confirm-dialog__path">{{ singlePathDisplay }}</code>
              </template>
            </i18n-t>
          </span>
          <span
            v-else
            class="permanent-delete-confirm-dialog__primary"
          >{{ bodyPrimaryMultiple }}</span>
          <span class="permanent-delete-confirm-dialog__secondary">{{ t('dialogs.deleteIrreversibly.confirm') }}</span>
        </DialogDescription>
      </DialogHeader>
      <DialogFooter class="permanent-delete-confirm-dialog__footer">
        <Button
          variant="outline"
          @click="onRequestClose"
        >
          {{ t('cancel') }}
        </Button>
        <Button
          variant="destructive"
          @click="onConfirm"
        >
          {{ t('dialogs.deleteIrreversibly.delete') }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>

<style scoped>
.permanent-delete-confirm-dialog__body {
  display: flex;
  flex-direction: column;
  gap: 0.75rem;
}

.permanent-delete-confirm-dialog__primary {
  color: hsl(var(--foreground));
}

.permanent-delete-confirm-dialog__path {
  padding: 0.125rem 0.375rem;
  border-radius: var(--radius-sm);
  background-color: hsl(var(--muted));
  font-family: var(--font-mono);
  font-size: 0.875em;
  overflow-wrap: anywhere;
}

.permanent-delete-confirm-dialog__secondary {
  color: hsl(var(--muted-foreground));
  font-size: 0.875rem;
}

.permanent-delete-confirm-dialog :deep(.permanent-delete-confirm-dialog__footer) {
  width: 100%;
  align-items: flex-end;
  gap: 0.5rem;
}
</style>
