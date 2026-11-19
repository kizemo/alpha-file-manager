// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * Windows shell 命名空间里的设备（手机 / MTP 便携设备 / 部分虚拟盘）。
 *
 * 后端 `get_shell_devices` 的返回结构。字段名与 Rust 侧一一对应：
 * - `name`：设备在 shell 里的显示名（「Galaxy S22」这类）
 * - `shellPath`：**不透明**的 shell 路径（`::{GUID}` / `shell:Name`），必须原样保留
 * - `isFolder`：设备根是不是一个可浏览的目录
 */
export type ShellDeviceInfo = {
  name: string;
  shellPath: string;
  isFolder: boolean;
};
