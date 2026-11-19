// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION: 批次 25 缺陷 B25-2（2026-11-19）——U 盘安全弹出。
 *
 * 后端（`src-tauri/src/device_events.rs`）收到 `WM_DEVICECHANGE` 后会把本进程占着
 * 那块盘的目录 watch 全部放掉，并向前端发一帧 `volume-change`。前端必须**立刻**
 * 关掉落在这块盘上的标签页：只要标签页还开着，导航轮询 / 目录读取就会重新
 * `CreateFile` 打开该盘的句柄，Windows 弹出时又会拿到 `ERROR_REMOVE_VETOED (23)`。
 *
 * 这里刻意写成**纯函数、不依赖 pinia / vue**：判定逻辑（哪些页属于这块盘、
 * 该关整组还是只关一格）是本次修复里最容易出错的部分，必须能在单测里用
 * 假数据直接断言。
 */

/** 只要求关闭逻辑真正需要的两个字段，方便单测传轻量假对象。 */
export type VolumeClosableTab = {
  id: string;
  path: string;
};

export type VolumeCloseTargets = {
  /** 整组都在这块盘上 → 关整组；取值为该组**首个** tab 的 id（见下方说明）。 */
  groupIds: string[];
  /** 分屏混合组里落在这块盘上的那一格 → 只关这格，其余 pane 不动。 */
  tabIds: string[];
};

const DRIVE_LETTER_PATTERN = /^([A-Za-z]):[\\/]/;

function normalizeDriveLetter(letter: string | null | undefined): string | null {
  if (!letter) {
    return null;
  }

  const trimmed = letter.trim().toUpperCase();

  return trimmed.length === 1 && trimmed >= 'A' && trimmed <= 'Z' ? trimmed : null;
}

/** 取路径所属盘符（大写）；UNC、Unix 路径、虚拟位置、空值一律返回 `null`。 */
export function driveLetterOfPath(path: string | null | undefined): string | null {
  const match = path ? DRIVE_LETTER_PATTERN.exec(path) : null;

  return match ? match[1].toUpperCase() : null;
}

/** 路径是否落在指定盘符下（盘符大小写不敏感）。 */
export function isPathOnDrive(
  path: string | null | undefined,
  driveLetter: string | null | undefined,
): boolean {
  const target = normalizeDriveLetter(driveLetter);

  if (!target) {
    return false;
  }

  return driveLetterOfPath(path) === target;
}

/**
 * 把「当前工作区的所有标签组」拆成 (关整组, 只关单格)。
 *
 * 为什么返回 **id** 而不是 `[组索引, 格索引]`：关组会让后面的组索引整体前移，
 * 预计算的坐标在第二个目标起就全是错的。调用方必须在每次关闭后按 id 重新定位。
 *
 * `groupIds` 用该组首个 tab 的 id —— 与 `closeTabGroup` 自己的定位方式
 * （`tabGroups.findIndex(g => g[0].id === target[0].id)`）保持同一口径。
 */
export function collectVolumeCloseTargets<T extends VolumeClosableTab>(
  tabGroups: readonly (readonly T[] | null | undefined)[] | null | undefined,
  driveLetter: string | null | undefined,
): VolumeCloseTargets {
  const groupIds: string[] = [];
  const tabIds: string[] = [];

  if (!tabGroups) {
    return {
      groupIds,
      tabIds,
    };
  }

  for (const tabGroup of tabGroups) {
    const firstTab = tabGroup?.[0];

    // 空组绝不能进名单：closeTabGroup 会读 tabGroup[0].id，空数组直接抛。
    if (!firstTab) {
      continue;
    }

    const tabsOnDrive = tabGroup.filter(tab => isPathOnDrive(tab?.path, driveLetter));

    if (tabsOnDrive.length === 0) {
      continue;
    }

    if (tabsOnDrive.length === tabGroup.length) {
      groupIds.push(firstTab.id);
      continue;
    }

    for (const tab of tabsOnDrive) {
      tabIds.push(tab.id);
    }
  }

  return {
    groupIds,
    tabIds,
  };
}
