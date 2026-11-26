// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { invoke } from '@tauri-apps/api/core';
import type { ShellDeviceInfo } from '@/types/shell-device';

// FORK-MODIFICATION 2026-11-19（批次 23）：shell 设备清单的**共享缓存**。
//
// ## 为什么需要缓存
//
// 「位置」页的条目由三处生成：
//   1. `readLocationsDirectory()` —— 真正读目录（盘符 + 设备 + 远程连接）
//   2. `buildVirtualDirectoryFromDrives()` —— 盘符变化时**重建**「位置」页
//   3. `getLocationsEntriesDisplaySignature()` —— 与当前显示内容比较，判是否要刷新
//
// 第 2、3 处拿不到「刚刚那次」读到的设备清单，只能读一份**共享状态**。
// 这与远程连接用 `remote-location-refs` 注册表是同一个道理：
// 三处必须用同一份设备清单，否则签名比较永远不相等，
// `use-file-browser-navigation.ts` 里的 `silentRefresh` 会被反复触发（每 1~5 秒一次）。
//
// 这里刻意**不引入 Vue**：本模块被 `virtual-locations.ts` 引入，
// 而后者又被工具栏、地址栏与若干纯函数单测引入 —— 拖进响应式运行时没有必要。

type ShellDevicesListener = (devices: ShellDeviceInfo[]) => void;

let cachedShellDevices: ShellDeviceInfo[] = [];
const listeners = new Set<ShellDevicesListener>();

/** 读当前已知的设备清单。 */
export function getCachedShellDevices(): ShellDeviceInfo[] {
  return cachedShellDevices;
}

/** 订阅设备清单变化（`readLocationsDirectory` 读到新清单时会被通知）。 */
export function subscribeShellDevices(listener: ShellDevicesListener): () => void {
  listeners.add(listener);

  return () => {
    listeners.delete(listener);
  };
}

function setCachedShellDevices(devices: ShellDeviceInfo[]): void {
  cachedShellDevices = devices;

  for (const listener of listeners) {
    listener(devices);
  }
}

/**
 * 向后端要一份最新的设备清单，并写进共享缓存。
 *
 * 失败（后端还没实现这个 command、设备枚举报错）时**清空**缓存而不是保留旧值：
 * 保留旧值会让「显示出来的条目」与「重建出来的条目」对不上，
 * 于是「位置」页每轮盘符轮询都触发一次无谓的刷新。
 */
export async function fetchShellDevices(): Promise<ShellDeviceInfo[]> {
  try {
    const devices = await invoke<ShellDeviceInfo[]>('get_shell_devices');
    setCachedShellDevices(Array.isArray(devices) ? devices : []);
  }
  catch (fetchError: unknown) {
    console.error('Failed to fetch shell devices:', fetchError);
    setCachedShellDevices([]);
  }

  return cachedShellDevices;
}
