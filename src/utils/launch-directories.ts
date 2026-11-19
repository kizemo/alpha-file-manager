// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { DirEntry } from '@/types/dir-entry';
import normalizePath, {
  canonicalizePath,
  getParentDirectory,
  isWindowsDriveRootPath,
} from '@/utils/normalize-path';
import { isShellPath } from '@/utils/shell-paths';
import { LOCATIONS_VIRTUAL_PATH } from '@/utils/virtual-locations';

// FORK-MODIFICATION (2026-11-03)：默认资源管理器改造 L3 —— 启动参数里的「选中」请求。
//
// Windows 的「打开文件所在目录」最终落到 `explorer.exe /select,"C:\dir\file.txt"`，
// 命令行解析后是**一个** argv 元素 `/select,C:\dir\file.txt`。
// 默认文件管理器启动器（`default-file-manager-launcher`）会把它改写成规范形式
// `--sigma-select=<路径>` 再转发过来；这里两种写法都认，
// 这样「不经启动器直接调主程序」也能工作。

/** 启动器转发给主程序的规范形式前缀。 */
export const LAUNCH_SELECT_ARG_PREFIX = '--sigma-select=';

/** Explorer 用的 select 标记。 */
export const LAUNCH_SELECT_FLAG = '/select';

export interface LaunchContext {
  args: string[];
  cwd: string | null;
  executableDir: string | null;
  hadAbsorbedShellPaths: boolean;
  hadDelegatedShellPaths: boolean;
}

export interface LaunchTarget {
  directoryPath: string;
  focusPath: string | null;
}

function getPathDedupeKey(path: string): string {
  return /^[A-Za-z]:/.test(path) ? path.toLowerCase() : path;
}

function trimLaunchPathQuotes(path: string): string {
  const trimmedPath = path.trim();

  if (
    trimmedPath.length >= 2
    && ((trimmedPath.startsWith('"') && trimmedPath.endsWith('"'))
      || (trimmedPath.startsWith('\'') && trimmedPath.endsWith('\'')))
  ) {
    return trimmedPath.slice(1, -1).trim();
  }

  if (trimmedPath.startsWith('"')) {
    return trimmedPath.slice(1);
  }

  if (trimmedPath.endsWith('"')) {
    return trimmedPath.slice(0, -1);
  }

  return trimmedPath;
}

function normalizeLaunchPathCandidate(path: string): string {
  const withoutQuotes = trimLaunchPathQuotes(path);

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 命名空间路径**原样保留**。
  //
  // 手机（MTP）传给 AFM 的真实形态就是这种路径：
  //
  //     ::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\?\usb#vid_22d9&pid_2765#...
  //
  // 它不是文件系统路径：`\` 是 shell 语义，没有可切分的层级。
  // 规范化（`\`→`/`、剥尾斜杠）会把它改坏，后端认不出来 ⇒
  // 候选被丢掉 ⇒ 兜底落到「位置」页 —— 那正是用户报的故障。
  if (isShellPath(withoutQuotes)) {
    return withoutQuotes;
  }

  const normalizedPath = normalizePath(withoutQuotes);

  if (isWindowsDriveRootPath(normalizedPath)) {
    return `${canonicalizePath(normalizedPath)}/`;
  }

  return normalizedPath;
}

export interface LaunchSelectRequest {
  /** 要定位并选中的条目路径。 */
  selectPath: string;
}

export interface ParsedLaunchArguments {
  /** 「打开文件所在目录」要选中的条目路径；没有则 null。 */
  selectPath: string | null;
  /** 除去 select 相关参数之后，剩下的路径类参数（尚未归一化）。 */
  pathArguments: string[];
}

/**
 * 把启动参数拆成「select 目标」与「路径类参数」。
 *
 * 认得三种写法：
 *   - `--sigma-select=<路径>` —— 启动器转发给主程序的规范形式
 *   - `/select,<路径>` —— Explorer 的实际形式（命令行解析后是一个 argv 元素）
 *   - `/select <路径>` —— 两个 argv 元素
 *
 * 多个 select 请求时**先出现的赢**，与动作注册表「先注册者赢」的口径一致。
 */
export function parseLaunchArguments(context: LaunchContext): ParsedLaunchArguments {
  // 注意：这里**不能**把变量叫 `arguments` —— ES 模块是严格模式，
  // `arguments` 是保留字，声明同名变量直接 SyntaxError。
  const launchArguments = context.args.slice(1);
  const pathArguments: string[] = [];
  let selectPath: string | null = null;
  let index = 0;

  while (index < launchArguments.length) {
    const argument = launchArguments[index];
    const trimmed = argument.trim();
    const lower = trimmed.toLowerCase();

    if (lower.startsWith(LAUNCH_SELECT_ARG_PREFIX)) {
      const value = trimmed.slice(LAUNCH_SELECT_ARG_PREFIX.length).trim();

      if (!selectPath && value) {
        selectPath = value;
      }

      index += 1;
      continue;
    }

    if (lower.startsWith(`${LAUNCH_SELECT_FLAG},`)) {
      const value = trimmed.slice(LAUNCH_SELECT_FLAG.length + 1).trim();

      if (!selectPath && value) {
        selectPath = value;
      }

      index += 1;
      continue;
    }

    if (lower === LAUNCH_SELECT_FLAG) {
      const next = launchArguments[index + 1];

      if (next !== undefined) {
        const value = next.trim();

        if (!selectPath && value) {
          selectPath = value;
        }

        index += 2;
        continue;
      }

      index += 1;
      continue;
    }

    pathArguments.push(argument);
    index += 1;
  }

  return {
    selectPath,
    pathArguments,
  };
}

export function getLaunchSelectRequest(context: LaunchContext): LaunchSelectRequest | null {
  const { selectPath } = parseLaunchArguments(context);

  return selectPath ? { selectPath } : null;
}

export function getLaunchDirectoryCandidates(context: LaunchContext): string[] {
  const candidates: string[] = [];
  const seenPaths = new Set<string>();
  const { pathArguments } = parseLaunchArguments(context);

  for (const arg of pathArguments) {
    const candidatePath = normalizeLaunchPathCandidate(arg);

    if (!candidatePath || candidatePath.startsWith('-')) {
      continue;
    }

    const dedupeKey = getPathDedupeKey(candidatePath);

    if (seenPaths.has(dedupeKey)) {
      continue;
    }

    seenPaths.add(dedupeKey);
    candidates.push(candidatePath);
  }

  // FORK-MODIFICATION 2026-11-19（批次 23）：兜底只在**确实没有任何候选**时兜。
  //
  // shell 路径候选在上面已经收集好了 —— 它本身就是候选，不会被这条兜底顶掉。
  // 改动前 shell 路径被 `normalizeLaunchPathCandidate` 改坏（`\`→`/`），
  // 于是这里判成「没有候选」，把用户丢到「位置」页。
  if (candidates.length === 0 && context.hadAbsorbedShellPaths) {
    candidates.push(LOCATIONS_VIRTUAL_PATH);
  }

  return candidates;
}

/**
 * 启动参数里的 **shell 路径**候选（手机 / MTP 设备）。
 *
 * 调用方用它判断「设备有没有真的打开」：候选在，但一个都没解析出来 ⇒
 * 设备打不开（已拔出 / 枚举报错），这时必须给用户明确提示，
 * 而不是静默停在「位置」页。
 */
export function getLaunchShellPathCandidates(context: LaunchContext): string[] {
  return getLaunchDirectoryCandidates(context).filter(isShellPath);
}

export function hasLaunchDirectoryCandidates(context: LaunchContext | undefined): boolean {
  if (context === undefined) {
    return false;
  }

  // select 请求也算「有启动目标」：否则启动页会盖掉「打开父目录 + 选中条目」的结果。
  if (getLaunchSelectRequest(context)) {
    return true;
  }

  return getLaunchDirectoryCandidates(context).length > 0;
}

export async function resolvePathLaunchTarget(
  rawPath: string,
  getDirEntry: (path: string) => Promise<DirEntry | null>,
): Promise<LaunchTarget | null> {
  const dirEntry = await getDirEntry(rawPath);

  if (!dirEntry) {
    return null;
  }

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 路径原样保留。
  //
  // `normalizePath` 对 `::{GUID}\?\usb#...` 不做任何处理，所以这一行
  // 不会把设备路径改坏。后端已支持对 shell 路径做 `get_dir_entry`，
  // 这里**没有**「只有普通文件路径才解析」的分支 —— 设备与普通目录一视同仁。
  const resolvedPath = normalizePath(dirEntry.path);

  if (dirEntry.is_dir) {
    return {
      directoryPath: resolvedPath,
      focusPath: null,
    };
  }

  if (dirEntry.is_file) {
    return {
      directoryPath: getParentDirectory(resolvedPath),
      focusPath: resolvedPath,
    };
  }

  return null;
}

/**
 * select 请求的目标：**打开条目的父目录 + 选中条目**。
 *
 * 与 `resolvePathLaunchTarget` 的区别：那个函数对目录返回「目录本身 + 无选中」，
 * 那是对的「打开这个目录」语义；select 语义要的是「父目录 + 选中它」，
 * 哪怕选中的本身是个目录。
 */
export async function resolveSelectLaunchTarget(
  rawPath: string,
  getDirEntry: (path: string) => Promise<DirEntry | null>,
): Promise<LaunchTarget | null> {
  const dirEntry = await getDirEntry(rawPath);

  if (!dirEntry) {
    return null;
  }

  const resolvedPath = normalizePath(dirEntry.path);

  return {
    directoryPath: getParentDirectory(resolvedPath),
    focusPath: resolvedPath,
  };
}

export async function resolveLaunchTargetsFromArgs(
  context: LaunchContext,
  getDirEntry: (path: string) => Promise<DirEntry | null>,
): Promise<LaunchTarget[]> {
  const launchTargets: LaunchTarget[] = [];
  const candidatePaths = getLaunchDirectoryCandidates(context);

  for (const candidatePath of candidatePaths) {
    const launchTarget = await resolvePathLaunchTarget(candidatePath, getDirEntry);

    if (launchTarget) {
      launchTargets.push(launchTarget);
    }
  }

  return launchTargets;
}
