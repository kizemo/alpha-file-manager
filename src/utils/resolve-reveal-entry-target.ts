// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L1 —— 快捷方式按**目标**定位。
//
// 「打开文件所在目录」如果作用在一个指向目录的 `.lnk` 上，用户要的是
// **快捷方式指向的那个目录**，而不是 `.lnk` 自己所在的文件夹。
//
// 口径（断链不报错）：
//   - 解析出目标且目标还在 → 打开目标的父目录 + 选中目标。
//   - 解析不出（不是快捷方式 / 目标不是目录 / 目标已删 / 调用失败）→
//     回落成「打开 `.lnk` 自己的父目录 + 选中 `.lnk` 本身」，**不弹错误**。
//
// 依赖用参数注入是为了能单测：默认实现走 Tauri invoke，测试里换成假实现。

import { invoke } from '@tauri-apps/api/core';
import normalizePath, { getParentDirectory } from '@/utils/normalize-path';

export interface RevealEntryTarget {
  /** 要打开的目录。 */
  directoryPath: string;
  /** 要选中的条目。 */
  focusPath: string;
}

export interface RevealEntryTargetDeps {
  /** 解析 Windows 目录快捷方式的目标路径；解析不出返回 null。 */
  resolveShortcutTarget: (path: string) => Promise<string | null>;
  /** 目标路径是否还存在（目录也算）。 */
  pathExists: (path: string) => Promise<boolean>;
}

export function isWindowsShortcutPath(path: string): boolean {
  return path.toLowerCase().endsWith('.lnk');
}

export async function resolveWindowsShortcutTarget(path: string): Promise<string | null> {
  try {
    return await invoke<string | null>('resolve_windows_directory_shortcut', { path });
  }
  catch (error) {
    console.error('Failed to resolve Windows directory shortcut:', error);
    return null;
  }
}

export async function pathExists(path: string): Promise<boolean> {
  try {
    return await invoke<boolean>('path_exists', { path });
  }
  catch (error) {
    console.error('Failed to check path existence:', error);
    return false;
  }
}

export function createRevealEntryTargetDeps(): RevealEntryTargetDeps {
  return {
    resolveShortcutTarget: resolveWindowsShortcutTarget,
    pathExists,
  };
}

export async function resolveRevealEntryTarget(
  entryPath: string,
  deps: RevealEntryTargetDeps = createRevealEntryTargetDeps(),
): Promise<RevealEntryTarget> {
  const fallbackTarget: RevealEntryTarget = {
    directoryPath: getParentDirectory(entryPath),
    focusPath: normalizePath(entryPath),
  };

  if (!isWindowsShortcutPath(entryPath)) {
    return fallbackTarget;
  }

  const shortcutTarget = await deps.resolveShortcutTarget(entryPath);

  if (!shortcutTarget) {
    return fallbackTarget;
  }

  // 目标可能刚好在解析之后被删掉 —— 这时同样回落，不报错。
  if (!await deps.pathExists(shortcutTarget)) {
    return fallbackTarget;
  }

  return {
    directoryPath: getParentDirectory(shortcutTarget),
    focusPath: normalizePath(shortcutTarget),
  };
}
