// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  getAddressBarSegments,
  isUncPath,
  isUnixFilesystemRoot,
  canonicalizePath,
} from '@/utils/normalize-path';
import {
  isVirtualDirectoryPath,
  isVirtualLocationPath,
  LOCATIONS_VIRTUAL_PATH,
  shouldPrependLocationsCrumb,
} from '@/utils/virtual-locations';
import {
  buildRemoteConnectionPath,
  buildRemotePath,
  isRemotePath,
  parseRemotePath,
  REMOTE_SCHEME,
} from '@/utils/remote-paths';
import { getShellPathDisplayName, isShellPath } from '@/utils/shell-paths';
import type { ShellPathCrumb } from '@/types/shell-device';

export type AddressBarPart = {
  path: string;
  name: string;
  isLast: boolean;
  disableDropTarget?: boolean;
  displayRootIcon?: boolean;
  useLocationsDropdown?: boolean;
};

/**
 * 远程面包屑需要的额外上下文。
 *
 * **刻意做成参数而不是在模块里读 store**：`buildAddressBarParts` 至今是一个纯函数，
 * 它的单测不装 pinia、不建 Tauri 桩。连接名一旦从这里自己去拿 store，
 * 这些测试就得整套重写 —— 而面包屑的判别点（路径拼得对不对）跟连接名从哪来无关。
 */
export interface RemoteBreadcrumbContext {
  /** `sfm://remote` 那一级的显示名（i18n）。 */
  rootLabel: string;
  /** 连接 id → 连接名。查不到时退回 id 本身。 */
  connectionNames: Record<string, string>;
}

const NO_REMOTE_CONTEXT: RemoteBreadcrumbContext = {
  rootLabel: REMOTE_SCHEME,
  connectionNames: {},
};

/**
 * shell 设备面包屑需要的额外上下文（批次 23）。
 *
 * 同 `RemoteBreadcrumbContext` 的理由：这里是一个纯函数，不读 store。
 * 设备名由调用方（地址栏）从设备清单里查好传进来。
 */
export interface ShellBreadcrumbContext {
  /** shell 路径 → 设备显示名。查不到时退回 `fallbackLabel`。 */
  deviceNames: Record<string, string>;
  /** 查不到设备名时的兜底短标签（i18n）。绝不显示 `::{...}` 整串。 */
  fallbackLabel: string;
  /**
   * 后端 `resolve_shell_path_chain` 的祖先链（外 → 内，末项 `path` 逐字等于当前路径）。
   *
   * 可选：链是异步拿到的，没拿到（加载中 / 命令报错 / 老调用方）时用两段式降级形态，
   * 用户看到的不会比批次 23 更差。
   */
  pathChain?: ShellPathCrumb[];
}

const NO_SHELL_CONTEXT: ShellBreadcrumbContext = {
  deviceNames: {},
  fallbackLabel: 'Device',
  pathChain: [],
};

/**
 * FORK-MODIFICATION 2026-11-19（批次 23）：shell 设备面包屑。
 * FORK-MODIFICATION 2026-11-19（批次 24）：层级改由后端祖先链决定。
 *
 * 手机（MTP）的路径是 `::{20D04FE0-...}\?\usb#...` —— 一个不透明标识符。
 * 通用分支会把它切成一堆看不懂的段（还会把 `::{` 当成盘符），所以批次 23 把整条当一级：
 *
 *   位置 > Galaxy S22
 *
 * 但一台手机在 shell 里其实不止一层：设备根下面是「内部共享存储空间」这类存储容器，
 * 再往下才是文件夹。批次 23 把整条压成一级，用户只能看到设备名，点不回上一层
 * （缺陷 D1：中间那层曾被前端字符串切分降级成笼统的「设备」）。
 *
 * 现在层级只认后端给的 `pathChain`（`resolve_shell_path_chain`，外 → 内）：
 *
 *   位置 > 一加 Ace 3 > 内部共享存储空间 > .7934039a
 *
 * 每级的 `path` 用链项自己的 `path` —— 前端一个字符都不改，
 * 因为 shell 路径里的 `\` 不是目录分隔符，段内还会带 `,` `{` `}` `#`。
 *
 * 链没到位（异步加载里、命令报错、路径已换）时退回批次 23 的两段式；
 * 设备名来自设备清单（`shellDeviceNames`），查不到只显示一个短标签，
 * **绝不**把 `::{...}` 整串糊在界面上。
 */
function buildShellAddressBarParts(
  path: string,
  locationsLabel: string,
  context: ShellBreadcrumbContext,
): AddressBarPart[] {
  const locationsPart: AddressBarPart = {
    path: LOCATIONS_VIRTUAL_PATH,
    name: locationsLabel,
    isLast: false,
    disableDropTarget: true,
    displayRootIcon: true,
    useLocationsDropdown: true,
  };

  const chain = context.pathChain ?? [];
  const lastCrumb = chain.length > 0 ? chain[chain.length - 1] : undefined;

  // 链必须是「通向当前路径」的：末项逐字等于入参才可用。
  // 这条同时兜住了陈旧链（用户已经走掉了，后端还在回上一条路径的链）。
  if (lastCrumb !== undefined && lastCrumb.path === path) {
    return [
      locationsPart,
      ...chain.map((crumb, index): AddressBarPart => ({
        path: crumb.path,
        // 链项的显示名为空是后端边界情况（shell 没给出解析名），退回短标签，
        // 而不是把整条面包屑丢掉。
        name: crumb.name !== '' ? crumb.name : context.fallbackLabel,
        isLast: index === chain.length - 1,
        // shell 路径不是文件系统目录，拖放目标没有意义。
        disableDropTarget: true,
      })),
    ];
  }

  const deviceName = context.deviceNames[path]
    ?? getShellPathDisplayName(path, context.fallbackLabel);

  return [
    locationsPart,
    {
      path,
      name: deviceName,
      isLast: true,
      // shell 路径不是文件系统目录，拖放目标没有意义。
      disableDropTarget: true,
    },
  ];
}

/**
 * FORK-MODIFICATION 2026-10-18：远程面包屑（阶段 2 需求 1）。
 *
 * ## 改动前为什么是错的
 *
 * `sfm://remote/c/pub` 走通用分支时，`getAddressBarSegments` 会切出
 * `['sfm:', 'remote', 'c', 'pub']`，而 `sfm:` 里带冒号，被 `pathSegments[0].includes(':')`
 * 那条当成 **Windows 盘符**，拼成 `sfm:/remote/c/pub` —— 一个用户从没去过的路径。
 * 点面包屑的任何一级都会跳到那里去。
 *
 * ## 这里必须用什么拼
 *
 * **`buildRemotePath`，不是字符串拼接。** 规范化的唯一权威是 `remote-paths.ts`
 * （它又是 Rust 侧 `path.rs` 的镜像），手工拼出来的路径在尾斜杠、连续斜杠、
 * 中文与空格这些边界上会和后端解析出两个结果。
 *
 * ## 层级为什么是「远程 → 连接 → 路径」
 *
 * `getNavigableParentPath` 允许从 `sfm://remote/<id>` 上到 `sfm://remote`，
 * 所以根这一级必须存在，否则「返回上一级」会走进一个没有面包屑的页面。
 */
function buildRemoteAddressBarParts(
  path: string,
  context: RemoteBreadcrumbContext,
): AddressBarPart[] {
  const rootPart: AddressBarPart = {
    path: REMOTE_SCHEME,
    name: context.rootLabel,
    isLast: false,
    disableDropTarget: true,
  };

  // `sfm://remote` 本身不是一个连接，`parseRemotePath` 对它返回 null，
  // 所以必须先于解析单独处理。
  if (path === REMOTE_SCHEME) {
    return [{
      ...rootPart,
      isLast: true,
    }];
  }

  const parsed = parseRemotePath(path);

  if (parsed === null) {
    // 非法远程路径（含 `.` / `..` 段）不产出面包屑。
    // 与 `getNavigableParentPath` 对同一批路径返回 null 保持一致 ——
    // 一个能点、一个不能点的面包屑比完全没有更糟。
    return [];
  }

  const segments = parsed.remotePath === '/'
    ? []
    : parsed.remotePath.slice(1).split('/');

  const connectionPath = buildRemoteConnectionPath(parsed.connectionId);

  if (connectionPath === null) {
    return [];
  }

  const parts: AddressBarPart[] = [
    rootPart,
    {
      path: connectionPath,
      // 连接名查不到就显示 id：uuid 难看，但至少让用户看得出「是哪一条」，
      // 总好过整条面包屑空着。
      name: context.connectionNames[parsed.connectionId] ?? parsed.connectionId,
      isLast: segments.length === 0,
    },
  ];

  segments.forEach((segment, index) => {
    const partialPath = buildRemotePath(parsed.connectionId, `/${segments.slice(0, index + 1).join('/')}`);

    if (partialPath === null) {
      return;
    }

    parts.push({
      path: partialPath,
      name: segment,
      isLast: index === segments.length - 1,
    });
  });

  return parts;
}

export function buildAddressBarParts(
  currentPath: string,
  platform: string | null,
  locationsLabel: string,
  remoteContext: RemoteBreadcrumbContext = NO_REMOTE_CONTEXT,
  shellContext: ShellBreadcrumbContext = NO_SHELL_CONTEXT,
): AddressBarPart[] {
  if (!currentPath) {
    return [];
  }

  // FORK-MODIFICATION 2026-11-19（批次 23）：shell 命名空间路径（手机 / MTP）。
  //
  // 必须排在 `canonicalizePath` 之前：规范化会把 `::{GUID}\?\usb#...`
  // 改成一个后端认不出来的路径，面包屑随之变成一堆看不懂的段。
  if (isShellPath(currentPath)) {
    return buildShellAddressBarParts(currentPath, locationsLabel, shellContext);
  }

  // 必须排在 `canonicalizePath` 之前：canonicalizePath 会把尾斜杠剥掉，
  // 通用分支会把 `sfm:` 当盘符（见 buildRemoteAddressBarParts 的注释）。
  if (isRemotePath(currentPath)) {
    return buildRemoteAddressBarParts(currentPath, remoteContext);
  }

  if (isVirtualLocationPath(currentPath)) {
    return [{
      path: LOCATIONS_VIRTUAL_PATH,
      name: locationsLabel,
      isLast: true,
      disableDropTarget: true,
      displayRootIcon: true,
      useLocationsDropdown: true,
    }];
  }

  const normalizedPath = canonicalizePath(currentPath);
  const parts = getAddressBarSegments(normalizedPath);
  const uncPath = isUncPath(normalizedPath);
  const formattedParts: AddressBarPart[] = [];

  parts.forEach((part, index) => {
    const pathSegments = parts.slice(0, index + 1);
    let fullPath = '';

    if (uncPath) {
      fullPath = `//${pathSegments.join('/')}`;
    }
    else if (isUnixFilesystemRoot(normalizedPath)) {
      fullPath = '/';
    }
    else if (normalizedPath.startsWith('/')) {
      fullPath = `/${pathSegments.join('/')}`;
    }
    else if (pathSegments[0]?.includes(':')) {
      fullPath = `${pathSegments[0]}/`;

      if (pathSegments.length > 1) {
        fullPath += pathSegments.slice(1).join('/');
      }
    }
    else {
      fullPath = pathSegments.join('/');
    }

    formattedParts.push({
      path: fullPath,
      name: part,
      isLast: index === parts.length - 1,
      disableDropTarget: isVirtualDirectoryPath(fullPath),
    });
  });

  if (!shouldPrependLocationsCrumb(currentPath, platform)) {
    return formattedParts;
  }

  return [
    {
      path: LOCATIONS_VIRTUAL_PATH,
      name: locationsLabel,
      isLast: false,
      disableDropTarget: true,
      displayRootIcon: true,
      useLocationsDropdown: true,
    },
    ...formattedParts,
  ];
}
