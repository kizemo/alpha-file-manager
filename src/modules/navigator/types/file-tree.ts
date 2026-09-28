// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/** 树节点(可展开目录或叶子文件) */
// FORK-MODIFICATION: new file (tree view). Keep during upstream sync.
// Tracking issue: aleksey-hoffman/sigma-file-manager#499
export interface FileTreeNode {
  path: string;
  name: string;
  isDirectory: boolean;
  isExpanded: boolean;
  isLoaded: boolean;
  depth: number;
  children: FileTreeNode[] | null;
}

/** 扁平化的一行(用于虚拟滚动) */
export interface FileTreeFlatRow {
  path: string;
  name: string;
  depth: number;
  isDirectory: boolean;
  hasChildren: boolean;
  isExpanded: boolean;
}