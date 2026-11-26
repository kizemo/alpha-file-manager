// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { ref, watch } from 'vue';
import type { ShellPathCrumb } from '@/types/shell-device';
import { isShellPath } from '@/utils/shell-paths';
import { fetchShellPathChain, getCachedShellPathChain } from '@/utils/shell-path-chain';

// FORK-MODIFICATION 2026-11-19（批次 24）：shell 祖先链的响应式外壳。
//
// 结构与 `use-shell-devices` 一致：状态在纯模块（`@/utils/shell-path-chain`）里，
// 这里只把「当前路径」映射成「一条链」。
//
// 两个不能省的守卫：
//   · **缓存**：同一个目录来回进出（返回上一级、前进、后退）不该反复问后端。
//   · **竞态**：快速连续换路径时，先发出的请求可能后回来。链是按路径取的，
//     把 A 设备的链贴到 B 设备的面包屑上比不贴更糟，所以写回前同时比对
//     「本次请求的序号」与「路径还是不是当初那条」。

/**
 * 跟随 `currentPath()` 取 shell 路径的祖先链。
 *
 * 非 shell 路径直接给出空链，**不会**调用后端命令。
 * 链拿不到（加载中 / 报错 / 陈旧）时同样是空链 —— 调用方据此退回两段式降级形态。
 */
export function useShellPathChain(currentPath: () => string) {
  const shellPathChain = ref<ShellPathCrumb[]>([]);
  let requestSequence = 0;

  watch(currentPath, (path) => {
    requestSequence = requestSequence + 1;
    const requestId = requestSequence;

    if (!isShellPath(path)) {
      shellPathChain.value = [];

      return;
    }

    const cached = getCachedShellPathChain(path);

    if (cached !== null) {
      shellPathChain.value = cached;

      return;
    }

    shellPathChain.value = [];

    void fetchShellPathChain(path).then((chain) => {
      // 已经有更新的请求发出，或者用户已经离开了那条路径 ⇒ 丢掉这次结果。
      if (requestId !== requestSequence || currentPath() !== path) {
        return;
      }

      shellPathChain.value = chain;
    });
  }, {
    immediate: true,
  });

  return {
    shellPathChain,
  };
}
