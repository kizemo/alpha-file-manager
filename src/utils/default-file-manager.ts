// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { invoke } from '@tauri-apps/api/core';

export async function isDefaultFileManager(): Promise<boolean> {
  return await invoke<boolean>('is_default_file_manager');
}

export async function setDefaultFileManager(enabled: boolean): Promise<boolean> {
  return await invoke<boolean>('set_default_file_manager', { enabled });
}

/**
 * FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L2。
 *
 * 「我是不是默认资源管理器」这个读取**不能**把异常直接抛给调用方：
 * 调用方拿它决定「打开文件所在目录」走应用内还是走系统 Explorer，
 * 一旦读取失败就应当**保守地**按「不是默认资源管理器」处理（继续走 Explorer），
 * 而不是让整个动作失败。
 */
export async function isDefaultFileManagerSafe(): Promise<boolean> {
  try {
    return await isDefaultFileManager();
  }
  catch (error) {
    console.error('Failed to read default file manager state:', error);
    return false;
  }
}
