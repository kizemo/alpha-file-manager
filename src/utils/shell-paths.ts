// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-19（批次 23）：Windows shell 命名空间路径。
//
// 手机（MTP）在 Windows 里**不是卷**，它是 shell 命名空间里的一个对象，路径形态是：
//
//     ::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\?\usb#vid_22d9&pid_2765&mi_00#...
//
// 这种路径**不是文件系统路径**：
//   · 里面的 `\` 是 shell 语义，不是路径分隔符；
//   · 没有可切分的目录层级（`SID-{10001,,1001925963776}` 这种末段也不是文件夹名）；
//   · 整体是一个不透明标识符，任何「规范化」动作都会把它改坏。
//
// 改坏之后后端认不出来 ⇒ 用户双击手机，AFM 跳到「位置」页 —— 那正是本轮要修的故障。
//
// 所以这个模块是**唯一**判定 shell 路径的地方，其它模块一律原样透传。

/** shell 命名空间路径的前缀（大小写不敏感）。 */
const SHELL_NAMESPACE_PREFIXES = ['::{', 'shell:'];

/** 判定一个路径是不是 Windows shell 命名空间路径。 */
export function isShellPath(path: string): boolean {
  if (!path) {
    return false;
  }

  const lowerPath = path.trim().toLowerCase();

  return SHELL_NAMESPACE_PREFIXES.some(prefix => lowerPath.startsWith(prefix));
}

/**
 * 取 shell 路径的**显示名**。
 *
 * `shell:MyFolder` → `MyFolder`（shell 名字后面那一段本身就是名字）；
 * `::{GUID}` → 兜底标签（GUID 不是名字，显示出来只会是一串噪音）。
 */
export function getShellPathDisplayName(path: string, fallbackLabel: string): string {
  if (!isShellPath(path)) {
    return fallbackLabel;
  }

  const trimmedPath = path.trim();

  if (trimmedPath.toLowerCase().startsWith('shell:')) {
    const shellName = trimmedPath.slice('shell:'.length).replace(/[\\/]+$/, '').trim();

    if (shellName) {
      return shellName;
    }
  }

  return fallbackLabel;
}

/**
 * shell 路径的上一层。
 *
 * shell 路径没有常规的目录层级，所以这里只做**前缀截断**：
 * 去掉最后一段，剩下的前缀仍然必须是 shell 路径，否则那是一个用户从没去过的路径。
 *
 * 两个必须处理的边界：
 *   · `::{GUID}`（裸节点，例如「此电脑」）没有更上一级 ⇒ null（调用方回落到「位置」页）。
 *   · 设备路径形如 `::{GUID}\?\usb#vid_22d9&pid_2765` —— `?` 是 shell 解析名的
 *     **分隔标记**，不是一层目录。照字面截断会得到 `::{GUID}\?` 这种垃圾路径
 *     （后端解析不出来，面包屑也没有名字可显示）。所以截断后要把它去掉，
 *     剩下裸 GUID 时同样判定为「没有更上一级」。
 */
export function getShellParentPath(path: string): string | null {
  if (!isShellPath(path)) {
    return null;
  }

  const trimmedPath = path.trim();

  // 裸节点：`::{GUID}` / `shell:Name` —— 已经是根。
  if (/^(?:::\{[^}]*\}|shell:[^\\/]*)$/i.test(trimmedPath)) {
    return null;
  }

  const parentMatch = trimmedPath.match(/^(.*[\\/])([^\\/]+)[\\/]*$/);

  if (!parentMatch) {
    return null;
  }

  // 尾部的分隔符与 `?` 标记都不是一层目录。
  // 顺序很重要：先剥 `?` 再剥分隔符，`::{GUID}\?\` 才会剥成 `::{GUID}`；
  // 反过来先剥分隔符就会留下 `::{GUID}\?`，再剥 `?` 又变成 `::{GUID}\` —— 尾部反斜杠回来了。
  // 所以这里循环剥到稳定为止。
  let parentPath = parentMatch[1];
  let strippedParentPath = '';

  while (parentPath !== strippedParentPath) {
    strippedParentPath = parentPath;
    parentPath = parentPath.replace(/\?+$/, '').replace(/[\\/]+$/, '');
  }

  if (!parentPath || !isShellPath(parentPath)) {
    return null;
  }

  // 截断后是裸 GUID（「此电脑」）：它不在设备清单里，也没有可显示的名字 ⇒ 没有更上一级。
  if (/^(?:::\{[^}]*\}|shell:[^\\/]*)$/i.test(parentPath)) {
    return null;
  }

  return parentPath;
}
