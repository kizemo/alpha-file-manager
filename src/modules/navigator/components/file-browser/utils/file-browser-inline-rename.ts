// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION 2026-10-27：就地重命名（F2 / 右键「重命名」）的纯逻辑。
 *
 * 需求：目录窗口的**列表视图**与**缩略图视图**里按 F2 或右键「重命名」时，
 * 不弹模态框，直接在条目上编辑文件名；提交之后用 ↑/↓ 切到相邻条目时，
 * 落点条目自动进入可编辑状态（便于手工连续批量改名）。
 *
 * 本模块只放**纯函数**，不碰 Vue、不碰 DOM：
 * 校验规则、选区规则、方向键意图、连续改名的状态机。
 * 校验与选区规则**原样复用** `file-browser-rename-dialog.vue` 的语义
 * （非法字符正则、`lastIndexOf('.') > 0` 才算扩展名），两边不许各写一套。
 */

/** 非法字符正则 —— 与 `file-browser-rename-dialog.vue` 里的那一份完全一致。 */
export const INLINE_RENAME_INVALID_CHARS_PATTERN = /[<>:"/\\|?*\x00-\x1f]/;

/** 编辑态里按 ↑/↓ 的意图。 */
export type InlineRenameMoveIntent = 'up' | 'down';

/** 提交改名是「怎么触发的」——不同触发方式对连续模式的影响不一样。 */
export type InlineRenameCommitTrigger = 'enter' | 'blur' | 'move';

/** 提交改名的结果。 */
export type InlineRenameCommitOutcome = 'renamed' | 'unchanged' | 'failed';

/** 连续改名状态机的状态。 */
export interface InlineRenameChainState {
  /** 当前是否处于就地编辑态（输入框是否渲染）。 */
  isEditing: boolean;
  /** 是否处于「连续改名模式」——落点条目会自动进入编辑态。 */
  isChainActive: boolean;
}

/**
 * 取「不含扩展名」的部分。
 *
 * 语义与 `file-browser-rename-dialog.vue` 里的 `getNameWithoutExtension` 一致：
 * 只有 `lastIndexOf('.') > 0` 才算扩展名，所以 `.gitignore` 会被整体当成名字。
 */
export function getInlineRenameNameWithoutExtension(name: string): string {
  const lastDotIndex = name.lastIndexOf('.');

  if (lastDotIndex > 0) {
    return name.substring(0, lastDotIndex);
  }

  return name;
}

/** 名字是否合法。空串、空白串、非法字符、`.` 与 `..` 一律不合法。 */
export function isValidInlineRenameName(name: string): boolean {
  const trimmed = name.trim();

  if (!trimmed) {
    return false;
  }

  if (INLINE_RENAME_INVALID_CHARS_PATTERN.test(trimmed)) {
    return false;
  }

  if (trimmed === '.' || trimmed === '..') {
    return false;
  }

  return true;
}

/**
 * 进入编辑态时输入框的选区。
 *
 * - 文件：选中**不含扩展名**的部分（扩展名留在外面不选中）。
 * - 目录：全选。
 */
export function getInlineRenameSelectionRange(
  name: string,
  isDirectory: boolean,
): {
  start: number;
  end: number;
} {
  if (isDirectory) {
    return {
      start: 0,
      end: name.length,
    };
  }

  return {
    start: 0,
    end: getInlineRenameNameWithoutExtension(name).length,
  };
}

/** 方向键意图。`ArrowUp` → `up`，`ArrowDown` → `down`，其它键 → `null`。 */
export function getInlineRenameMoveIntent(key: string): InlineRenameMoveIntent | null {
  if (key === 'ArrowUp') {
    return 'up';
  }

  if (key === 'ArrowDown') {
    return 'down';
  }

  return null;
}

/**
 * 提交改名之后的状态机。
 *
 * - 成功（改名 / 名字没变）：退出编辑态，**保持**连续模式 ⇒ 下一次 ↑/↓ 的落点会自动进入编辑态。
 * - 失败：结束连续模式；`enter` / `move` 留在编辑态让用户继续改，`blur` 已经失去焦点，直接退出。
 * - `blur` 提交（用户点了别处）：提交后**结束**连续模式。
 */
export function getInlineRenameChainStateAfterCommit(
  trigger: InlineRenameCommitTrigger,
  outcome: InlineRenameCommitOutcome,
): InlineRenameChainState {
  if (outcome === 'failed') {
    return {
      isEditing: trigger !== 'blur',
      isChainActive: false,
    };
  }

  if (trigger === 'blur') {
    return {
      isEditing: false,
      isChainActive: false,
    };
  }

  return {
    isEditing: false,
    isChainActive: true,
  };
}

/** 取消（Esc）之后的状态机：编辑态与连续模式一起结束。 */
export function getInlineRenameChainStateAfterCancel(): InlineRenameChainState {
  return {
    isEditing: false,
    isChainActive: false,
  };
}

/**
 * 导航落点要不要自动进入编辑态。
 *
 * 只有「连续模式仍然开着」且「当前布局支持就地编辑」时才进入 ——
 * tree 布局没有就地编辑实现，放它进去只会变成「点了没反应」。
 */
export function shouldEnterInlineRenameOnNavigationTarget(
  isChainActive: boolean,
  canEditInline: boolean,
): boolean {
  return isChainActive && canEditInline;
}
