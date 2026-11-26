// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { isVirtualLocationPath } from '@/utils/virtual-path-constants';
import { getShellParentPath, isShellPath } from '@/utils/shell-paths';

// FORK-MODIFICATION 2026-11-19（批次 23）：shell 命名空间路径**不规范化**。
//
// 手机（MTP）的路径长这样：
//
//     ::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\?\usb#vid_22d9&pid_2765&mi_00#...
//
// 里面的 `\` 是 shell 语义，不是路径分隔符；整体是一个不透明标识符。
// 把它 `\`→`/`、剥尾斜杠、切段，后端就认不出来了 —— 用户点手机会看到
// 「双击手机 → AFM 跳到位置页」。所以下面每个规范化入口都要先问一句
// 「这是不是 shell 路径」，是的话原样返回。

export default function normalizePath(path: string): string {
  if (isShellPath(path)) {
    return path;
  }

  return path.replace(/\\/g, '/');
}

export function canonicalizePath(path: string): string {
  // shell 路径连尾斜杠都不能剥 —— 它整体就是一个标识符。
  if (isShellPath(path)) {
    return path;
  }

  const normalizedPath = normalizePath(path);
  return normalizedPath.replace(/\/+$/, '') || (normalizedPath.startsWith('/') ? '/' : '');
}

export function stripTrailingSlashesPreservingRoot(path: string): string {
  return canonicalizePath(path);
}

export function isUnixFilesystemRoot(path: string): boolean {
  return canonicalizePath(path) === '/';
}

export function isUncPath(path: string): boolean {
  const normalizedPath = normalizePath(path);
  return normalizedPath.startsWith('//') && !normalizedPath.startsWith('///');
}

export function isWslPath(path: string): boolean {
  const normalizedPath = normalizePath(path).toLowerCase();

  if (!isUncPath(normalizedPath)) {
    return false;
  }

  const firstSegment = normalizedPath.slice(2).split('/').filter(Boolean)[0] ?? '';
  return firstSegment === 'wsl.localhost' || firstSegment === 'wsl$';
}

export function isWslHostRootUncPath(path: string): boolean {
  const normalizedPath = canonicalizePath(path).toLowerCase();

  if (!isUncPath(normalizedPath)) {
    return false;
  }

  const segments = normalizedPath.slice(2).split('/').filter(Boolean);

  if (segments.length !== 1) {
    return false;
  }

  return segments[0] === 'wsl.localhost' || segments[0] === 'wsl$';
}

export function isWindowsDriveRootPath(path: string): boolean {
  const normalizedPath = canonicalizePath(path);
  return /^[A-Za-z]:$/.test(normalizedPath);
}

export function isUncShareRootPath(path: string): boolean {
  const normalizedPath = canonicalizePath(path);

  if (!isUncPath(normalizedPath)) {
    return false;
  }

  return normalizedPath.slice(2).split('/').filter(Boolean).length === 2;
}

export function getPathSegments(path: string): string[] {
  // shell 路径没有可切分的目录层级 —— 切出来只会得到一堆看不懂的段。
  if (isShellPath(path)) {
    return [];
  }

  return canonicalizePath(path).split('/').filter(Boolean);
}

export function getAddressBarSegments(path: string): string[] {
  const canonicalPath = canonicalizePath(path);

  if (isUnixFilesystemRoot(canonicalPath)) {
    return ['/'];
  }

  return getPathSegments(canonicalPath);
}

export function getPathLeafName(path: string): string {
  // shell 路径的「末段」是设备标识符的一部分，不是名字。
  if (isShellPath(path)) {
    return '';
  }

  const canonicalPath = canonicalizePath(path);

  if (!canonicalPath) {
    return '';
  }

  const pathSegments = getPathSegments(canonicalPath);
  return pathSegments[pathSegments.length - 1] || canonicalPath;
}

export function getParentPath(path: string): string | null {
  // shell 路径的上一层是「前缀截断」，不是常规的目录切分。
  if (isShellPath(path)) {
    return getShellParentPath(path);
  }

  const canonicalPath = canonicalizePath(path);

  if (!canonicalPath || isUnixFilesystemRoot(canonicalPath)) {
    return null;
  }

  if (isUncPath(canonicalPath)) {
    const pathSegments = canonicalPath.slice(2).split('/').filter(Boolean);

    if (pathSegments.length <= 1) {
      return null;
    }

    return `//${pathSegments.slice(0, -1).join('/')}`;
  }

  const slashIndex = canonicalPath.lastIndexOf('/');

  if (slashIndex < 0) {
    return null;
  }

  if (slashIndex === 0) {
    return '/';
  }

  const parentPath = canonicalPath.substring(0, slashIndex);
  return parentPath.endsWith(':') ? `${parentPath}/` : parentPath;
}

export function getParentDirectory(filePath: string): string {
  return getParentPath(filePath) ?? canonicalizePath(filePath);
}

export function joinPath(parentPath: string, childName: string): string {
  const canonicalParentPath = canonicalizePath(parentPath);
  const trimmedChildName = childName.replace(/^\/+|\/+$/g, '');

  if (!trimmedChildName) {
    return canonicalParentPath;
  }

  if (!canonicalParentPath) {
    return trimmedChildName;
  }

  if (isUnixFilesystemRoot(canonicalParentPath)) {
    return `/${trimmedChildName}`;
  }

  return `${canonicalParentPath}/${trimmedChildName}`;
}

export function isSameOrDescendantPath(childPath: string, parentPath: string): boolean {
  // shell 路径用的是 `\` 分隔，常规的 `/` 前缀判断对它一律不成立。
  if (isShellPath(childPath) || isShellPath(parentPath)) {
    if (!childPath || !parentPath) {
      return false;
    }

    if (childPath === parentPath) {
      return true;
    }

    return childPath.startsWith(`${parentPath}\\`) || childPath.startsWith(`${parentPath}/`);
  }

  const canonicalChildPath = canonicalizePath(childPath);
  const canonicalParentPath = canonicalizePath(parentPath);

  if (!canonicalChildPath || !canonicalParentPath) {
    return false;
  }

  if (canonicalChildPath === canonicalParentPath) {
    return true;
  }

  if (isUnixFilesystemRoot(canonicalParentPath)) {
    return canonicalChildPath.startsWith('/');
  }

  return canonicalChildPath.startsWith(`${canonicalParentPath}/`);
}

export function getPathDisplayName(path: string, translate?: (key: string) => string): string {
  if (isVirtualLocationPath(path)) {
    if (translate) {
      return translate('locations');
    }

    return 'Locations';
  }

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 路径显示**短标签**。
  // `::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\?\usb#...` 整串糊在界面上没人认得，
  // 设备真正的名字由「位置」页的条目给出（见 `shellDeviceInfoToDirEntry`）。
  if (isShellPath(path)) {
    return translate ? translate('fileBrowser.device') : 'Device';
  }

  const normalizedPath = normalizePath(path);

  if (!normalizedPath) {
    return '';
  }

  if (isUnixFilesystemRoot(normalizedPath)) {
    return '/';
  }

  const leafName = getPathLeafName(normalizedPath);
  return leafName || normalizedPath;
}

export function getPathDisplayValue(path: string): string {
  // shell 路径原样显示：地址栏输入框里的内容要能被复制走，不能被改写。
  if (isShellPath(path)) {
    return path;
  }

  const normalizedPath = normalizePath(path);

  if (!normalizedPath) {
    return '';
  }

  if (isUnixFilesystemRoot(normalizedPath)) {
    return '/';
  }

  if (isWindowsDriveRootPath(normalizedPath) || isUncShareRootPath(normalizedPath)) {
    return canonicalizePath(normalizedPath);
  }

  return normalizedPath;
}

/**
 * 把路径转成**当前平台的原生分隔符**。
 *
 * 应用内部统一用正斜杠 —— 后端 `normalize_path` 与前端 `normalizePath`
 * 都把 `\` 归一成 `/`。那是**内部表示**，因为比较键、缓存键、远程路径
 * （`sfm://remote/...` 里的 `/` 是协议语义）全都依赖它。
 *
 * 但**给人看的**路径、**复制到剪贴板的**路径必须是原生写法：
 * Windows 是 `C:\Users\...`，macOS / Linux 是 `/Users/...`。
 *
 * ⚠️ 带 `://` 的路径（`sfm://remote/...`、`sfm://locations`）原样保留 ——
 * 那里面的 `/` 不是本地路径分隔符，换成 `\` 会得到一个根本打不开的路径。
 */
export function toNativePathSeparators(path: string, isWindows: boolean): string {
  if (!isWindows || !path) {
    return path;
  }

  if (path.includes('://')) {
    return path;
  }

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 路径的 `\` 是 shell 语义，
  // 不是 Windows 路径分隔符 —— 换成 `/` 之后后端就认不出来了。
  if (isShellPath(path)) {
    return path;
  }

  return path.replace(/\//g, '\\');
}

/**
 * 展示用的路径值（原生分隔符）。
 *
 * = `getPathDisplayValue` 的语义 + 原生分隔符。
 */
export function getNativePathDisplayValue(path: string, isWindows: boolean): string {
  return toNativePathSeparators(getPathDisplayValue(path), isWindows);
}
