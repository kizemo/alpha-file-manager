// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { openPath } from '@tauri-apps/plugin-opener';

/**
 * 把一个位置交给**系统**资源管理器打开。
 *
 * 应用内的 shell 管线（`read_dir` / `get_dir_entry`）对某些设备不生效：
 * 设备已拔出、后端枚举失败、MTP 还没就绪 —— 这时候应用内没有任何东西可看。
 * 系统自己的资源管理器对 `::{GUID}` 这类 shell 命名空间路径是原生支持的，
 * 所以它是唯一能给用户看的出路。
 *
 * 失败只记日志：这是一个**补救**动作，不该再弹一个错误盖住原来的错误。
 */
export async function openInSystemExplorer(path: string): Promise<void> {
  try {
    await openPath(path);
  }
  catch (openError: unknown) {
    console.error('Failed to open the location in the system file manager:', openError);
  }
}
