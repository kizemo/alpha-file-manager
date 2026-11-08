// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 远程标签页的标题与副标题（2026-11-08）。
//
// 抽成独立模块是为了能测：标签页组件那一圈依赖（工作区 store、快捷键 store、
// 拖拽会话）在测试环境里拖不进完整应用上下文，而这恰恰是用户盯着看的那一行字。
//
// 背景：远程路径长这样 `sfm://remote/<连接id>/<远端路径>`，
// 而 `getPathDisplayName` 对普通路径取的是叶子名 —— 对远程路径来说叶子就是那串
// 连接 id，于是标签页上摆的是「0736c40a51db4fd19f711c0b6934f103」，
// 用户完全看不出那是自己的百度网盘。这里换成**连接名**。

import type { RemotePath } from '@/utils/remote-paths';

export interface RemoteTabLabels {
  /**
   * 标签页标题。
   *
   * 查不到连接名（还没加载完、或连接已被删）时是 `null` ——
   * 由调用方退回原来的显示逻辑。宁可显示 id，也不要整块空白。
   */
  title: string | null;
  /** 悬停提示里的第二行：远端自己的路径（`/`、`/prompts`）。 */
  subtitle: string;
}

/**
 * 由解析好的远程路径与连接名算出标签页的标题与副标题。
 *
 * - 连接根（`sfm://remote/<id>`）→ 标题就是连接名；
 * - 连接下的子目录（`sfm://remote/<id>/prompts`）→ 标题是「连接名 / 目录名」；
 * - 查不到连接名 → 标题为 `null`（调用方决定怎么退化）。
 */
export function buildRemoteTabLabels(
  parsed: RemotePath,
  connectionName: string | undefined,
): RemoteTabLabels {
  const leaf = parsed.remotePath.split('/').filter(segment => segment.length > 0).pop();

  if (!connectionName) {
    return {
      title: null,
      subtitle: parsed.remotePath,
    };
  }

  if (parsed.remotePath === '/' || !leaf) {
    return {
      title: connectionName,
      subtitle: parsed.remotePath,
    };
  }

  return {
    title: `${connectionName} / ${leaf}`,
    subtitle: parsed.remotePath,
  };
}
