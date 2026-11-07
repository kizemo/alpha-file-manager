// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION (2026-11-07)：快捷方式「打开文件所在目录」的落点解析。
//
// 2026-11-03 的 L1 版本在前端分两步做：
//   ① `resolve_windows_directory_shortcut` → ② `path_exists`。
// 两个问题都在 2026-11-07 修掉：
//   1. ① 只认**目录**目标 ⇒ 指向**文件**的快捷方式（绝大多数）一律解析不出来，
//      回落成「打开 `.lnk` 自己所在的目录」—— 用户看到的现象就是
//      「点了没反应，没跳到快捷方式指向的文件所在目录」。
//   2. ①② 都是**同步** Tauri command，跑在 Tauri 主线程上。COM 解析 / 网络盘 stat
//      一慢就阻塞 tao 事件循环 ⇒ WebView2 心跳断掉 ⇒ 看门狗判「窗口卡死」
//      ⇒ 渲染进程崩溃（用户截图里的「界面渲染进程已崩溃（窗口 main）」），
//      右键菜单的子菜单加载不出来也是同一个冻结的表现。
//
// 现在改成**后端一次往返**：`resolve_reveal_entry_target` 内部 `spawn_blocking`
// + 超时兜底，断链 / 超时 / 调用失败一律静默回落（不弹错误、不冻界面）。

import { invoke } from '@tauri-apps/api/core';
import normalizePath, { getParentDirectory } from '@/utils/normalize-path';

export interface RevealEntryTarget {
  /** 要打开的目录。 */
  directoryPath: string;
  /** 要选中的条目。 */
  focusPath: string;
  /**
   * 落点是不是从快捷方式解析出来的。
   *
   * `false` 且条目本身是 `.lnk` ⇒ 快捷方式**解析不出目标**
   * （断链 / COM 读不了 / 环境变量没展开）。
   * 此时 `focusPath` 就是 `.lnk` 自己 —— 绝不能把它交给 Windows Explorer：
   * Explorer 会为解析不了的快捷方式弹「已中止操作」的模态框。
   */
  resolvedFromShortcut: boolean;
}

/** 后端命令的原始返回（serde `camelCase`）。 */
export interface RevealEntryTargetPayload {
  directoryPath: string;
  focusPath: string;
  resolvedFromShortcut: boolean;
}

export interface RevealEntryTargetDeps {
  /**
   * 后端一次性解析落点。
   * 解析不出（断链 / 超时 / 调用失败）返回 null —— 回落由调用方负责。
   */
  resolveTarget: (path: string) => Promise<RevealEntryTarget | null>;
}

export async function resolveRevealEntryTargetFromBackend(
  path: string,
): Promise<RevealEntryTarget | null> {
  try {
    const payload = await invoke<RevealEntryTargetPayload | null>(
      'resolve_reveal_entry_target',
      { path },
    );

    if (!payload?.directoryPath || !payload?.focusPath) {
      return null;
    }

    return {
      directoryPath: payload.directoryPath,
      focusPath: payload.focusPath,
      resolvedFromShortcut: payload.resolvedFromShortcut === true,
    };
  }
  catch (error) {
    console.error('Failed to resolve reveal entry target:', error);
    return null;
  }
}

export function createRevealEntryTargetDeps(): RevealEntryTargetDeps {
  return {
    resolveTarget: resolveRevealEntryTargetFromBackend,
  };
}

/**
 * 解析「打开文件所在目录」的落点。
 *
 * 正常路径走后端命令（一次往返拿到「目录 + 选中条目」）；
 * 后端解析不出 / 超时 / 调用失败时**静默回落**成
 * 「打开条目自己的父目录 + 选中条目自己」，不抛错、不冻界面。
 */
export async function resolveRevealEntryTarget(
  entryPath: string,
  deps: RevealEntryTargetDeps = createRevealEntryTargetDeps(),
): Promise<RevealEntryTarget> {
  const fallbackTarget: RevealEntryTarget = {
    directoryPath: getParentDirectory(entryPath),
    focusPath: normalizePath(entryPath),
    resolvedFromShortcut: false,
  };

  const resolvedTarget = await deps.resolveTarget(entryPath);

  if (!resolvedTarget) {
    return fallbackTarget;
  }

  return resolvedTarget;
}
