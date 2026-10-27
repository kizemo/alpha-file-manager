<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
Copyright © 2021 - present Aleksey Hoffman. All rights reserved.
-->

<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import { useFileBrowserContext } from './composables/use-file-browser-context';
import {
  getInlineRenameMoveIntent,
  getInlineRenameSelectionRange,
  isValidInlineRenameName,
} from './utils/file-browser-inline-rename';

const props = defineProps<{
  entry: DirEntry;
}>();

const ctx = useFileBrowserContext();

// FORK-MODIFICATION 2026-10-27：草稿文本放在组件本地 ——
// 列表视图的行有 `v-memo` 缓存，父级传下来的受控值更新会被缓存吞掉；
// 子组件自己持有 draft 才能保证输入框里看到的就是用户敲的字。
const draft = ref(props.entry.name);
const inputRef = ref<HTMLInputElement | null>(null);
const isInvalid = ref(false);
/**
 * 已经提交过或取消过。
 *
 * 输入框被卸载时浏览器**不一定**发 blur，但发过一次就再来一次会重复提交 ——
 * 所以处理完（回车 / Esc / 方向键 / 失焦）先置位，后来的 blur 直接跳过。
 */
const isSettled = ref(false);

function refreshValidity() {
  isInvalid.value = draft.value.length > 0 && !isValidInlineRenameName(draft.value);
}

function handleInput(event: Event) {
  const target = event.target as HTMLInputElement;

  draft.value = target.value;
  refreshValidity();
}

async function focusInput() {
  await nextTick();

  // FORK-MODIFICATION 2026-10-27：网格卡片上有 `content-visibility: auto`，
  // 首帧可能还没布局完 —— 焦点要再等一帧才给得准。
  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      const element = inputRef.value;

      if (!element) {
        return;
      }

      element.focus();

      const range = getInlineRenameSelectionRange(element.value, props.entry.is_dir);

      element.setSelectionRange(range.start, range.end);
    });
  });
}

onMounted(() => {
  void focusInput();
});

async function submit(trigger: 'enter' | 'blur') {
  if (!isValidInlineRenameName(draft.value)) {
    isInvalid.value = true;
    return;
  }

  await ctx.inlineRename.submit(draft.value, trigger);
}

function handleKeydown(event: KeyboardEvent) {
  // FORK-MODIFICATION 2026-10-27：输入框里的按键**不**往条目层冒泡 ——
  // 否则 Space 会被当成「打开条目」，↑/↓ 会被当成「移动选中项」。
  event.stopPropagation();

  if (event.key === 'Enter') {
    event.preventDefault();
    isSettled.value = true;
    void submit('enter');
    return;
  }

  if (event.key === 'Escape') {
    event.preventDefault();
    isSettled.value = true;
    ctx.inlineRename.cancel();
    return;
  }

  const moveIntent = getInlineRenameMoveIntent(event.key);

  if (moveIntent) {
    event.preventDefault();
    isSettled.value = true;
    void ctx.inlineRename.move(moveIntent, draft.value);
  }
}

function handleBlur() {
  if (isSettled.value) {
    return;
  }

  isSettled.value = true;

  // 名字非法 ⇒ 当作放弃（不能提交一个改不了的名字）。
  if (!isValidInlineRenameName(draft.value)) {
    ctx.inlineRename.cancel();
    return;
  }

  void submit('blur');
}
</script>

<template>
  <input
    ref="inputRef"
    class="file-browser-inline-rename-input"
    :class="{ 'file-browser-inline-rename-input--invalid': isInvalid }"
    type="text"
    :value="draft"
    autocomplete="off"
    spellcheck="false"
    @input="handleInput"
    @keydown="handleKeydown"
    @blur="handleBlur"
    @mousedown.stop
    @mouseup.stop
    @click.stop
  >
</template>

<style scoped>
.file-browser-inline-rename-input {
  width: 100%;
  min-width: 0;
  box-sizing: border-box;
  margin: 0;
  padding: 0 2px;
  font: inherit;
  line-height: inherit;
  color: inherit;
  background-color: transparent;
  border: 1px solid currentColor;
  border-radius: 3px;
}

.file-browser-inline-rename-input--invalid {
  border-color: #dc2626;
}
</style>
