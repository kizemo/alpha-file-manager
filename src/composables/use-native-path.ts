// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { usePlatformStore } from '@/stores/runtime/platform';
import {
  getNativePathDisplayValue,
  toNativePathSeparators,
} from '@/utils/normalize-path';

/**
 * FORK-MODIFICATION 2026-11-06：路径一律按**平台原生分隔符**展示 / 复制。
 *
 * 应用内部统一用正斜杠（后端 `normalize_path`、前端 `normalizePath`），
 * 但用户看到的、复制到剪贴板的路径必须是原生写法：
 * Windows 上是 `C:\Users\...`，macOS / Linux 上是 `/Users/...`。
 *
 * 这个组合式把「是不是 Windows」这层平台判断收在一处，
 * 调用方只管把路径丢进来。
 */
export function useNativePath() {
  const platformStore = usePlatformStore();

  /** 路径 → 原生分隔符写法（剪贴板、提示文案用）。 */
  function toNativePath(path: string): string {
    return toNativePathSeparators(path, platformStore.isWindows);
  }

  /** 路径 → 展示值（根路径归一 + 原生分隔符）。 */
  function toNativeDisplayValue(path: string): string {
    return getNativePathDisplayValue(path, platformStore.isWindows);
  }

  return {
    toNativePath,
    toNativeDisplayValue,
  };
}
