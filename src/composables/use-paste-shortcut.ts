// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { markRaw } from 'vue';
import { useI18n } from 'vue-i18n';
import { invoke } from '@tauri-apps/api/core';
import { useClipboardStore } from '@/stores/runtime/clipboard';
import { useWorkspacesStore } from '@/stores/storage/workspaces';
import { toast, ToastStatic } from '@/components/ui/toaster';
import type { CreateLinksResult } from '@/utils/link-operations';

/**
 * FORK-MODIFICATION 2026-11-06：「粘贴快捷方式」。
 *
 * 把剪贴板里的文件 / 目录以 `.lnk` 快捷方式的形式落到目标目录。
 *
 * 语义：
 * - **源**优先取系统剪贴板（用户在别的程序里复制的文件），
 *   没有就退回应用内剪贴板（在 AFM 里复制过的条目）；
 * - **不销毁剪贴板** —— 可以往不同目录反复粘贴；
 * - 走已有的 `create_links` command（`linkKind: 'shortcut'`），不新增后端命令。
 */
export function usePasteShortcut() {
  const { t } = useI18n();
  const clipboardStore = useClipboardStore();
  const workspacesStore = useWorkspacesStore();

  /** 剪贴板里能拿来建快捷方式的源路径。 */
  async function resolveSourcePaths(): Promise<string[]> {
    const systemClipboard = await clipboardStore.readSystemClipboardFiles();

    if (systemClipboard?.paths.length) {
      return systemClipboard.paths;
    }

    if (clipboardStore.hasFileItems) {
      return clipboardStore.clipboardItems.map(item => item.path);
    }

    return [];
  }

  /** 把快捷方式建到 `destinationPath`。剪贴板里没有文件时返回 null。 */
  async function pasteShortcut(destinationPath: string): Promise<CreateLinksResult | null> {
    if (!destinationPath) {
      return null;
    }

    const sourcePaths = await resolveSourcePaths();

    if (!sourcePaths.length) {
      return null;
    }

    try {
      const result = await invoke<CreateLinksResult>('create_links', {
        sourcePaths,
        destinationPath,
        linkKind: 'shortcut',
      });

      if (result.createdPaths.length > 0) {
        workspacesStore.handleDirectoryContentsChanged([destinationPath]);

        toast.custom(markRaw(ToastStatic), {
          componentProps: {
            data: {
              title: t('notifications.createdNLinks', result.createdPaths.length),
              description: '',
            },
          },
        });

        return result;
      }

      const firstError = result.failedItems[0]?.error || t('fileBrowser.createLinkFailed');

      toast.custom(markRaw(ToastStatic), {
        componentProps: {
          data: {
            title: t('fileBrowser.createLinkFailed'),
            description: firstError,
          },
        },
      });

      return result;
    }
    catch (error) {
      toast.custom(markRaw(ToastStatic), {
        componentProps: {
          data: {
            title: t('fileBrowser.createLinkFailed'),
            description: String(error),
          },
        },
      });

      return null;
    }
  }

  return {
    resolveSourcePaths,
    pasteShortcut,
  };
}
