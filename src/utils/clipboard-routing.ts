// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { isRemotePath } from '@/utils/remote-paths';

/**
 * 剪贴板路由判定：什么时候**不该**把选中的条目写进系统剪贴板。
 *
 * 抽成纯函数的原因（T9 2026-11-11）：这段判断是「网盘 Ctrl+C / Ctrl+V 无效」的
 * 真正根因，它必须能被单测直接断言。留在 `clipboard.ts` 里就得 mock Tauri
 * `invoke` 才能测，而 mock 掉 invoke 之后，测试断言的已经不是行为了。
 *
 * ## 为什么远程条目不能进系统剪贴板
 *
 * 系统剪贴板的文件列表是 CF_HDROP —— 只认真实绝对路径。
 * `sfm://remote/conn-a/pub/x.jpg` 不是本机路径，写进去是**静默失败**：
 * 不报错、不留痕，用户以为复制成功了。更糟的是下一次粘贴时，
 * `pasteItems` 会先读系统剪贴板，读到上次真实复制的本地文件
 * ⇒ 应用内剪贴板里的远程条目被**遮蔽**，Ctrl+V 粘出完全无关的东西。
 *
 * 所以：只要选区里**有任意一个**远程条目，就整批不同步到系统剪贴板
 * （混合选区也不做「一半同步一半跳过」——那只会让遮蔽问题更难排查）。
 * 远程条目的复制粘贴完全走应用内剪贴板，粘贴时由
 * `handleRemotePaste → downloadItemsFromRemote` 落地成真实下载。
 */

/**
 * 这批路径是否值得同步到系统剪贴板。
 *
 * 空选区 ⇒ `false`（没东西可同步）；含远程条目 ⇒ `false`（同步了也是假的）。
 */
export function shouldSyncPathsToSystemClipboard(paths: readonly string[]): boolean {
  if (!paths.length) {
    return false;
  }

  return !paths.some(path => isRemotePath(path));
}

/**
 * 一组路径里是否混进了网盘 URI。
 *
 * `pasteItems` 拿到的是系统剪贴板读回的 `string[]`（CF_HDROP 的原始路径），
 * 不是 `{ path }` 条目；单独提供这个入口，避免调用方为判定伪造对象。
 */
export function clipboardPathsHoldRemote(paths: readonly string[]): boolean {
  return paths.some(path => isRemotePath(path));
}

/**
 * 应用内剪贴板里是否装着远程条目。
 *
 * `pasteItems` 用它决定优先级：装着远程条目时**不能**让系统剪贴板的
 * 残留本地文件抢走粘贴结果。
 */
export function clipboardHoldsRemoteItems(items: readonly { path: string }[]): boolean {
  return clipboardPathsHoldRemote(items.map(item => item.path));
}
