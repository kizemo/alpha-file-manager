// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 2026-10-31 —— 拖放落点的判定。
//
// 判定规则只有一条：**沿排列方向分成两半**。
//   - 收藏区是 grid（行优先排列）→ 用水平方向分左右两半：左半 = 插到前面，右半 = 插到后面。
//   - 收藏分类是竖排列表 → 用垂直方向分上下两半：上半 = 插到前面，下半 = 插到后面。
//
// 为什么不用「按 Y 轴行中点」那套：`components/sortable-list/sortable-geometry.ts`
// 的行中点是**从列表顶部逐项累加高度**算出来的，N 列并排时中点会变成
// 「N 倍高」的假坐标，横向跨列完全无感知。仪表盘收藏恰好就是多列 grid，
// 所以这里改成「指针落在目标框的哪一半」，与列数无关。

export type DropAxis = 'horizontal' | 'vertical';

export interface DropRect {
  left: number;
  right: number;
  top: number;
  bottom: number;
}

export interface DropPointer {
  x: number;
  y: number;
}

/**
 * 指针落在目标框的前半还是后半。
 *
 * 落在正中线上时算「后面」：网格里相邻两格的接缝正好落在正中线上，
 * 归给哪一边都不比另一边更自然，取一个确定的值即可。
 */
export function getDropPlacement(
  pointer: DropPointer,
  rect: DropRect,
  axis: DropAxis,
): 'before' | 'after' {
  const start = axis === 'horizontal' ? rect.left : rect.top;
  const end = axis === 'horizontal' ? rect.right : rect.bottom;
  const value = axis === 'horizontal' ? pointer.x : pointer.y;
  const center = start + ((end - start) / 2);

  return value < center ? 'before' : 'after';
}

/**
 * 取元素的客户区矩形。元素不存在时返回 null（拖放过程中节点可能已被卸载）。
 */
export function getElementRect(element: Element | null): DropRect | null {
  if (!element) {
    return null;
  }

  const rect = element.getBoundingClientRect();

  return {
    left: rect.left,
    right: rect.right,
    top: rect.top,
    bottom: rect.bottom,
  };
}
