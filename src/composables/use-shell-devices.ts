// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { onMounted, onUnmounted, ref } from 'vue';
import type { ShellDeviceInfo } from '@/types/shell-device';
import {
  fetchShellDevices,
  getCachedShellDevices,
  subscribeShellDevices,
} from '@/utils/shell-devices';

// FORK-MODIFICATION 2026-11-19（批次 23）：shell 设备的响应式外壳。
//
// 「位置」页与地址栏下拉都要显示设备，两边必须看到**同一份**清单。
// 真正的状态放在 `@/utils/shell-devices`（纯模块，无 Vue 依赖），
// 这里只做一层响应式包装：`readLocationsDirectory` 读到新清单时，
// 本模块的 ref 会同步更新，地址栏的下拉因此会自己刷新。

const shellDevices = ref<ShellDeviceInfo[]>(getCachedShellDevices());

let subscriberCount = 0;
let unsubscribe: (() => void) | null = null;

export function useShellDevices() {
  onMounted(() => {
    subscriberCount++;

    if (subscriberCount === 1) {
      unsubscribe = subscribeShellDevices((devices) => {
        shellDevices.value = devices;
      });

      shellDevices.value = getCachedShellDevices();
      void fetchShellDevices();
    }
  });

  onUnmounted(() => {
    subscriberCount--;

    if (subscriberCount === 0) {
      unsubscribe?.();
      unsubscribe = null;
    }
  });

  return {
    shellDevices,
  };
}
