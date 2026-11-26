// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 58（2026-11-25）—— 收藏条目的「标题 / 路径」直接编辑。
//
// 两处入口共用这套判定：
//   1. 快速访问（仪表盘）收藏卡上直接点标题 / 路径就地编辑；
//   2. 侧边栏「快速访问」悬停面板上的小铅笔 → 收藏编辑弹窗。
//
// 判定为什么要走「存在性 + 重复」两道：
//   - 路径必须真实存在，否则收藏打开会失败，用户却看不出原因；
//   - 路径不能与另一个收藏重复，否则两条收藏指向同一处，「移除收藏」会连坐。
//
// 存在性判定要问后端（`path_exists` / `path_is_regular_file`），所以这里把
// 两个探针做成**可注入的依赖**：默认走 Tauri，单测里塞假实现，
// 于是这套规则可以纯离线地测。

/**
 * 归一化用户输入的路径。
 *
 * 去掉首尾空白与**首尾各一个**双引号 —— 从资源管理器复制路径出来常常带着引号，
 * 直接用会得到一个「看起来存在、其实找不到」的路径。
 *
 * 只脱一层而不是「把所有连续引号都脱掉」：路径本身在 Windows 上是允许含引号
 * 字符的（只是不能出现在文件名里），多层引号更可能是用户自己多敲的，
 * 脱一层既够用又不会把路径改坏。
 */
export function normalizeFavoritePathInput(raw: string): string {
  return raw.trim().replace(/^"|"$/g, '').trim();
}

/**
 * 找出与候选路径重复的既有收藏路径；没有重复则返回 null。
 *
 * `existingPaths` 应当是**除本条收藏之外**的全部收藏路径 ——
 * 编辑一条收藏的路径时，它自己不算重复。
 */
export function findDuplicateFavoritePath(
  candidate: string,
  existingPaths: readonly string[],
): string | null {
  const normalized = normalizeFavoritePathInput(candidate);

  if (!normalized) {
    return null;
  }

  return existingPaths.find(path => normalizeFavoritePathInput(path) === normalized) ?? null;
}

export type FavoritePathCheckStatus = 'ok' | 'missing' | 'duplicate';

export interface FavoritePathCheckResult {
  status: FavoritePathCheckStatus;
  /** 仅 status === 'ok' 时有意义：目标到底是文件还是目录。 */
  isFile: boolean;
}

/** 存在性探针：路径是否真实存在。 */
export type FavoritePathExistsProbe = (path: string) => Promise<boolean>;

/** 文件判定探针：路径是不是一个普通文件（不是目录）。 */
export type FavoritePathIsFileProbe = (path: string) => Promise<boolean>;

export interface FavoritePathCheckOptions {
  exists?: FavoritePathExistsProbe;
  isRegularFile?: FavoritePathIsFileProbe;
}

/**
 * 校验一条收藏的新路径能不能用。
 *
 * 判定顺序是刻意的：**先查重复，再问后端**。重复是纯本地判定，
 * 一次 IPC 就能挡掉；存在性才需要真的去问文件系统。
 */
export async function checkFavoritePath(
  candidate: string,
  existingPaths: readonly string[],
  options: FavoritePathCheckOptions = {},
): Promise<FavoritePathCheckResult> {
  const path = normalizeFavoritePathInput(candidate);

  if (!path) {
    return {
      status: 'missing',
      isFile: false,
    };
  }

  if (findDuplicateFavoritePath(path, existingPaths)) {
    return {
      status: 'duplicate',
      isFile: false,
    };
  }

  const exists = options.exists ?? defaultExistsProbe;
  const isRegularFile = options.isRegularFile ?? defaultIsFileProbe;

  let pathExists = false;

  try {
    pathExists = await exists(path);
  }
  catch {
    pathExists = false;
  }

  if (!pathExists) {
    return {
      status: 'missing',
      isFile: false,
    };
  }

  let isFile = false;

  try {
    isFile = await isRegularFile(path);
  }
  catch {
    // 判定不了就按目录处理：目录是收藏里更常见的那一类，
    // 且误判成目录只会让打开动作走错分支，不会造成数据损坏。
    isFile = false;
  }

  return {
    status: 'ok',
    isFile,
  };
}

async function defaultExistsProbe(path: string): Promise<boolean> {
  const { invoke } = await import('@tauri-apps/api/core');

  return invoke<boolean>('path_exists', { path });
}

async function defaultIsFileProbe(path: string): Promise<boolean> {
  const { invoke } = await import('@tauri-apps/api/core');

  return invoke<boolean>('path_is_regular_file', { path });
}
