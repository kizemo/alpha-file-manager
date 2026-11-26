// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { defineStore } from 'pinia';
import { ref } from 'vue';

const IDLE_THRESHOLD_MS = 60 * 1000;
const IDLE_CHECK_INTERVAL_MS = 10 * 1000;

export const useAppStateStore = defineStore('appState', () => {
  // FORK-MODIFICATION 2026-10-30（意见 10）：空闲检测的时间戳不需要响应式。
  // 之前它是 ref，鼠标每移动一下就写一次，触发整条响应式更新链；
  // 改成普通变量后 mousemove 只剩一次赋值。
  let lastActivityTime = Date.now();
  const isUserIdle = ref(false);
  const idleCheckIntervalId = ref<ReturnType<typeof setInterval> | null>(null);
  let onIdleIntervalTick: (() => void) | null = null;

  function recordActivity() {
    lastActivityTime = Date.now();

    // 只有真的从「空闲」回到「活跃」时才写 ref，避免每次 mousemove 都触发更新。
    if (isUserIdle.value) {
      isUserIdle.value = false;
    }
  }

  function getIsUserIdle() {
    return (Date.now() - lastActivityTime) > IDLE_THRESHOLD_MS;
  }

  function tickIdleState() {
    isUserIdle.value = getIsUserIdle();
  }

  function startUserIdleDetection(onIntervalTick?: () => void) {
    onIdleIntervalTick = onIntervalTick ?? null;

    if (idleCheckIntervalId.value !== null) {
      return;
    }

    const activityEvents = ['mousemove', 'mousedown', 'keydown', 'touchstart', 'scroll', 'wheel'];
    activityEvents.forEach((event) => {
      window.addEventListener(event, recordActivity, { passive: true });
    });

    idleCheckIntervalId.value = setInterval(() => {
      tickIdleState();
      onIdleIntervalTick?.();
    }, IDLE_CHECK_INTERVAL_MS);
  }

  function stopUserIdleDetection() {
    if (idleCheckIntervalId.value !== null) {
      clearInterval(idleCheckIntervalId.value);
      idleCheckIntervalId.value = null;
    }

    onIdleIntervalTick = null;
    isUserIdle.value = false;

    const activityEvents = ['mousemove', 'mousedown', 'keydown', 'touchstart', 'scroll', 'wheel'];
    activityEvents.forEach((event) => {
      window.removeEventListener(event, recordActivity);
    });
  }

  return {
    isUserIdle,
    getIsUserIdle,
    startUserIdleDetection,
    stopUserIdleDetection,
  };
});
