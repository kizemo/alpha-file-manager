<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { ref, computed, watch, nextTick } from 'vue';
import { useI18n } from 'vue-i18n';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { Button } from '@/components/ui/button';

const props = defineProps<{
  type: 'directory' | 'file';
}>();

const emit = defineEmits<{
  /**
   * `done` 是提交结果的回执：父组件**必须**在异步操作结束后调用它。
   *
   * 2026-11-09 批次 F.9 修 D1：父组件此前只在**成功**时关对话框，
   * 失败时既不复位 `isSubmitting`（⇒「创建」按钮永久变灰、无法重试），
   * 也不给父级任何信号。回执机制让失败路径也能解锁按钮。
   */
  confirm: [name: string, done: (ok: boolean) => void];
  cancel: [];
}>();

const isOpen = defineModel<boolean>('open', { required: true });

const { t } = useI18n();

const inputRef = ref<InstanceType<typeof Input> | null>(null);
const name = ref('');
const isSubmitting = ref(false);

const dialogTitle = computed(() => {
  return props.type === 'directory'
    ? t('navigator.newDirectory')
    : t('navigator.newFile');
});

const trimmedName = computed(() => name.value.trim());

const isValid = computed(() => {
  if (!trimmedName.value) return false;

  const invalidChars = /[<>:"/\\|?*\x00-\x1f]/;

  if (invalidChars.test(trimmedName.value)) return false;

  if (trimmedName.value === '.' || trimmedName.value === '..') return false;

  return true;
});

watch(name, () => {
  if (isSubmitting.value) {
    isSubmitting.value = false;
  }
});

watch(isOpen, (open) => {
  if (open) {
    name.value = '';
    isSubmitting.value = false;

    nextTick(() => {
      const componentEl = inputRef.value?.$el;
      let inputElement: HTMLInputElement | null = null;

      if (componentEl) {
        if (componentEl instanceof HTMLInputElement) {
          inputElement = componentEl;
        }
        else {
          inputElement = componentEl.querySelector('input');
        }
      }

      if (inputElement) {
        inputElement.focus();
      }
    });
  }
  else {
    handleCancel();
  }
});

async function handleSubmit() {
  if (!isValid.value || isSubmitting.value) return;

  isSubmitting.value = true;

  let settled = false;

  function done(ok: boolean) {
    if (settled) return;
    settled = true;

    // 成功时父组件会关掉对话框，对话框关闭时统一复位；
    // **失败时必须在这里复位**，否则「创建」按钮会永久保持禁用状态。
    if (!ok) {
      isSubmitting.value = false;
    }
  }

  emit('confirm', trimmedName.value, done);
}

function handleCancel() {
  emit('cancel');
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Enter' && isValid.value) {
    event.preventDefault();
    handleSubmit();
  }
}
</script>

<template>
  <Dialog v-model:open="isOpen">
    <DialogContent class="file-browser-new-item-dialog">
      <DialogHeader>
        <DialogTitle>{{ dialogTitle }}</DialogTitle>
      </DialogHeader>

      <div class="file-browser-new-item-dialog__form">
        <div class="file-browser-new-item-dialog__field-wrapper">
          <label
            for="new-item-input"
            class="file-browser-new-item-dialog__label"
          >
            {{ t('name') }}
          </label>
          <div class="file-browser-new-item-dialog__field">
            <Input
              id="new-item-input"
              ref="inputRef"
              v-model="name"
              :class="{ 'file-browser-new-item-dialog__input--error': name && !isValid }"
              @keydown="handleKeydown"
            />
            <Button
              :disabled="!isValid || isSubmitting"
              @click="handleSubmit"
            >
              {{ t('create') }}
            </Button>
          </div>
        </div>
      </div>

      <DialogFooter />
    </DialogContent>
  </Dialog>
</template>

<style>
.file-browser-new-item-dialog {
  width: 420px;
  max-width: calc(100vw - 32px);
  box-sizing: border-box;
  overflow-x: hidden;
}

.file-browser-new-item-dialog > * {
  min-width: 0;
}

.file-browser-new-item-dialog__form {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  gap: 16px;
}

.file-browser-new-item-dialog__field-wrapper {
  display: flex;
  width: 100%;
  min-width: 0;
  flex-direction: column;
  gap: 8px;
}

.file-browser-new-item-dialog__field {
  display: flex;
  width: 100%;
  min-width: 0;
  align-items: center;
  gap: 8px;
}

.file-browser-new-item-dialog__label {
  color: hsl(var(--foreground));
  font-size: 14px;
  font-weight: 500;
}

.file-browser-new-item-dialog__field .sigma-ui-input {
  width: 100%;
  min-width: 0;
  max-width: 100%;
  box-sizing: border-box;
}

.file-browser-new-item-dialog__input--error {
  border-color: hsl(var(--destructive));
}
</style>
