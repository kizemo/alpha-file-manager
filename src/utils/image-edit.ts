// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 图片右键编辑（左旋 / 右旋 / 左右翻 / 上下翻）的前端封装。
//
// 这一层只做三件事：
// ① 把「右键动作 id」翻译成后端认识的 `operation` 字符串；
// ② 调后端命令 `apply_image_edit`（**原地写回**，无二次确认）；
// ③ 成功 / 失败各弹一个 toast，并让这条目的缩略图缓存失效。
//
// 缓存失效用的是**既有**缓存工具（`use-image-thumbnails.ts` 里的
// `cancelImageThumbnail` / `clearThumbnails`），没有另造一套。

import { invoke } from '@tauri-apps/api/core';
import { toast } from '@/components/ui/toaster';
import { getParentDirectory } from '@/utils/normalize-path';
import type { DirEntry } from '@/types/dir-entry';
import { isEditableImageEntry } from '@/modules/navigator/components/file-browser/utils/context-menu-action-visibility';

/**
 * 文案键。
 *
 * ⚠️ 这里**故意不静态 import `@/localization`**：那会把 17 个语言包
 * 以及 `createI18n` 一起拉进本模块的依赖图，而调用方（两个 composable）
 * 本来就有自己的 `t`。键名集中放在这里，解析交给调用方。
 */
export const IMAGE_EDIT_MESSAGE_KEYS = {
  success: 'fileBrowser.actions.imageEditSucceeded',
  failure: 'fileBrowser.actions.imageEditFailed',
  inProgress: 'fileBrowser.actions.imageEditInProgress',
} as const;

/** 后端命令名 —— 必须与 Rust 侧 `#[tauri::command]` 的函数名逐字一致。 */
export const APPLY_IMAGE_EDIT_COMMAND = 'apply_image_edit';

/** 后端认识的四种操作。 */
export type ImageEditOperation = 'rotate-left' | 'rotate-right' | 'flip-horizontal' | 'flip-vertical';

/** 右键动作 id → 后端 `operation`。 */
export const IMAGE_EDIT_ACTIONS = {
  'rotate-image-left': 'rotate-left',
  'rotate-image-right': 'rotate-right',
  'flip-image-horizontal': 'flip-horizontal',
  'flip-image-vertical': 'flip-vertical',
} as const satisfies Record<string, ImageEditOperation>;

export type ImageEditActionId = keyof typeof IMAGE_EDIT_ACTIONS;

/** 这个右键动作是不是图片编辑四动作之一。 */
export function isImageEditAction(action: string): action is ImageEditActionId {
  return Object.prototype.hasOwnProperty.call(IMAGE_EDIT_ACTIONS, action);
}

/** 右键动作 id → 后端 `operation` 字符串。 */
export function getImageEditOperation(action: ImageEditActionId): ImageEditOperation {
  return IMAGE_EDIT_ACTIONS[action];
}

/** 后端返回的结果（字段名以 Rust 最终实现为准）。 */
export type ImageEditResult = {
  path: string;
  width: number;
  height: number;
  bytes: number;
  format: string;
};

/**
 * 缩略图缓存工具。
 *
 * 与 `use-image-thumbnails.ts` 暴露的那几个方法同形 ——
 * 调用方把共享实例传进来，这里不自己造缓存。
 */
export type ImageThumbnailCache = {
  cancelImageThumbnail: (entry: DirEntry, maxDimension?: number) => void;
  clearThumbnails: () => void;
};

/** 默认的写回质量（0..100）。 */
export const DEFAULT_IMAGE_EDIT_QUALITY = 90;

export type ApplyImageEditOptions = {
  /** 目标条目（必须是单个可编辑图片文件）。 */
  entry: DirEntry;
  /** 后端操作。 */
  operation: ImageEditOperation;
  /** 写回质量，省略时用 {@link DEFAULT_IMAGE_EDIT_QUALITY}。 */
  quality?: number;
  /** 共享的缩略图缓存实例（成功后来让它失效）。 */
  thumbnailCache?: ImageThumbnailCache | null;
  /** 成功提示的标题（可选；不传就用应用语言包里的默认文案）。 */
  successTitle?: string;
  /** 失败提示的标题（可选；不传就用应用语言包里的默认文案）。 */
  failureTitle?: string;
  /** 「处理中」提示的标题（可选；不传就用应用语言包里的默认文案）。 */
  inProgressTitle?: string;
};

/**
 * 让这条目的缩略图缓存失效。
 *
 * 既有缓存工具（`use-image-thumbnails.ts`）：`cancelImageThumbnail` 会把这一条
 * 排队中 / 进行中的**旧**请求丢掉。
 *
 * 缓存键本身是 `path|modified_time|size|maxDimension`，原地写回后 mtime / size
 * 都会变 ⇒ 新键天然取不到旧图；这里再把旧请求取消掉，避免它白跑一趟。
 */
export function invalidateImageCaches(entry: DirEntry, cache?: ImageThumbnailCache | null): void {
  if (!cache) {
    return;
  }

  cache.cancelImageThumbnail(entry);
}

/**
 * 目录内容变了 ⇒ 让当前所在目录重读一遍。
 *
 * 重读之后条目带上新的 mtime / size ⇒ 缩略图缓存键变了 ⇒ 界面上出现的是新图。
 * 复用的是既有的 `workspacesStore.handleDirectoryContentsChanged` 通道
 * （`use-file-browser-navigation.ts` 里 watch 它并 `readDir`）。
 */
export async function refreshParentDirectory(entry: DirEntry): Promise<void> {
  const parentPath = getParentDirectory(entry.path);

  if (!parentPath) {
    return;
  }

  // The store is imported dynamically on purpose: a static import would drag
  // the router (and with it the whole route table) into the dependency graph
  // of every module that uses this file. That breaks the unit tests of the
  // file browser composables with a "createI18n is not defined" style error.
  const { useWorkspacesStore } = await import('@/stores/storage/workspaces');
  const workspacesStore = useWorkspacesStore();
  workspacesStore.handleDirectoryContentsChanged([parentPath]);
}

/**
 * 解析 toast 标题。
 *
 * 语言包是**动态**引入的：静态 import `@/localization` 会把 17 个语言包
 * 连同 `createI18n` 一起拉进每个使用方的依赖图。
 */
async function resolveMessageTitles(
  successTitle?: string,
  failureTitle?: string,
  inProgressTitle?: string,
): Promise<{
  success: string;
  failure: string;
  inProgress: string;
}> {
  let success = successTitle;
  let failure = failureTitle;
  let inProgress = inProgressTitle;

  if (!success || !failure || !inProgress) {
    try {
      const { i18n } = await import('@/localization');

      success = success ?? String(i18n.global.t(IMAGE_EDIT_MESSAGE_KEYS.success));
      failure = failure ?? String(i18n.global.t(IMAGE_EDIT_MESSAGE_KEYS.failure));
      inProgress = inProgress ?? String(i18n.global.t(IMAGE_EDIT_MESSAGE_KEYS.inProgress));
    }
    catch {
      // The language pack is unavailable: the raw key is used as the text.
    }
  }

  return {
    success: success ?? IMAGE_EDIT_MESSAGE_KEYS.success,
    failure: failure ?? IMAGE_EDIT_MESSAGE_KEYS.failure,
    inProgress: inProgress ?? IMAGE_EDIT_MESSAGE_KEYS.inProgress,
  };
}

/**
 * 执行一次图片编辑。
 *
 * - 原地写回，**无二次确认**；
 * - 失败弹 toast，**不向上冒泡**（不 reject，调用方不用 try/catch）。
 */
export async function applyImageEdit(options: ApplyImageEditOptions): Promise<boolean> {
  const {
    entry,
    operation,
    quality,
    thumbnailCache,
    successTitle,
    failureTitle,
    inProgressTitle,
  } = options;
  const titles = await resolveMessageTitles(successTitle, failureTitle, inProgressTitle);

  if (!isEditableImageEntry(entry)) {
    toast.error(titles.failure, {
      description: entry.name,
    });

    return false;
  }

  // A large photo needs a few seconds to be decoded, transformed and encoded
  // again. The toast tells the user that something is happening instead of
  // leaving the menu entry looking like it did nothing.
  const progressToastId = toast.loading(titles.inProgress, {
    description: entry.name,
  });

  try {
    const result = await invoke<ImageEditResult>(APPLY_IMAGE_EDIT_COMMAND, {
      path: entry.path,
      operation,
      quality: typeof quality === 'number' ? quality : DEFAULT_IMAGE_EDIT_QUALITY,
    });

    invalidateImageCaches(entry, thumbnailCache);
    await refreshParentDirectory(entry);

    toast.success(titles.success, {
      description: entry.name,
    });

    console.info('apply_image_edit succeeded:', result);

    return true;
  }
  catch (error) {
    console.error('apply_image_edit failed:', error);

    toast.error(titles.failure, {
      description: error instanceof Error ? error.message : String(error),
    });

    return false;
  }
  finally {
    toast.dismiss(progressToastId);
  }
}
