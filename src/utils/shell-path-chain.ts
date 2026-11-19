// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { invoke } from '@tauri-apps/api/core';
import type { ShellPathCrumb } from '@/types/shell-device';

// FORK-MODIFICATION 2026-11-19（批次 24）：shell 路径祖先链的**共享缓存**。
//
// ## 为什么层级必须由后端给
//
// 手机（MTP）在 Windows shell 命名空间里的路径长这样：
//
//     ::{20D04FE0-...}\\\?\usb#vid_22d9&pid_2765&mi_00#6&...#{6ac27878-...}\SID-{10001,,1001925963776}
//
// 里面的 `\` 是 shell 语义而不是目录分隔符，段内还可能出现 `,` `{` `}` `#`（`SID-` 段就是例子）。
// 前端按字符串切分必然切错，批次 23 就是这么把「内部共享存储空间」降级成了笼统的「设备」。
// 后端用 `IShellItem::GetParent()` 逐级上溯，每一级拿的是 shell **自己**的显示名，
// 所以这里唯一的职责就是「按 path 取一次、缓存住、别改写字段」。
//
// ## 为什么放在纯模块里
//
// 与 `shell-devices.ts` 同一个理由：本模块被非响应式的工具链引入时不该拖进 Vue 运行时，
// 而响应式的外壳（`use-shell-path-chain`）只是一层薄包装。

/**
 * 缓存条数上限。一次浏览会在设备里进出若干目录，超过上限后丢最旧的一条：
 * 链是可以随时向后端重取的，留着无限增长才是问题。
 */
const MAX_CACHED_CHAINS = 64;

const chainCache = new Map<string, ShellPathCrumb[]>();

function rememberChain(path: string, chain: ShellPathCrumb[]): void {
  if (chainCache.size >= MAX_CACHED_CHAINS && !chainCache.has(path)) {
    const oldestPath = chainCache.keys().next().value;

    if (oldestPath !== undefined) {
      chainCache.delete(oldestPath);
    }
  }

  chainCache.set(path, chain);
}

/**
 * 清空祖先链缓存。
 *
 * 设备被弹出时调用方可以主动作废，测试也必须用它隔离用例 ——
 * 缓存是模块级的，同一份夹具路径在下一条用例里会被同步命中，根本走不到后端。
 */
export function clearShellPathChainCache(): void {
  chainCache.clear();
}

/**
 * 取已缓存的祖先链；没缓存过返回 `null`（而不是空数组）。
 *
 * `null` 与 `[]` 必须可区分：调用方据此决定「同步用上缓存」还是「先去问后端」。
 */
export function getCachedShellPathChain(path: string): ShellPathCrumb[] | null {
  return chainCache.get(path) ?? null;
}

/**
 * 向后端要一条 shell 路径的祖先链（外 → 内，末项逐字等于 `path`）。
 *
 * 三种「拿不到」的情形一律按空链处理，绝不抛给调用方：
 *   · 命令报错（设备刚被拔掉 / shell 解析失败）；
 *   · 后端返回的不是数组（契约被破坏时宁可退回降级形态）；
 *   · 链中某项没有 `path`（点不出一个不存在的路径）。
 *
 * 失败与畸形结果**不写缓存**：设备可能稍后才出现，缓存成空链会让它永远出不来。
 */
export async function fetchShellPathChain(path: string): Promise<ShellPathCrumb[]> {
  const cached = chainCache.get(path);

  if (cached !== undefined) {
    return cached;
  }

  try {
    const chain = await invoke<ShellPathCrumb[]>('resolve_shell_path_chain', { path });

    if (!Array.isArray(chain)) {
      console.error('Unexpected shell path chain payload:', chain);

      return [];
    }

    const usableChain = chain.filter(crumb => typeof crumb?.path === 'string' && crumb.path !== '');

    if (usableChain.length === 0) {
      return [];
    }

    rememberChain(path, usableChain);

    return usableChain;
  }
  catch (resolveError: unknown) {
    console.error('Failed to resolve shell path chain:', resolveError);

    return [];
  }
}
