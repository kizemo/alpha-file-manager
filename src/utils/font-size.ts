// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 字体大小三档（需求 3，2026-10-31）。
//
// 需求原文：「以当前字体大小为"默认"，增加"大"、"特大"两级。
// 分别比当前默认增大 2px、4px。」
//
// ⚠️ 这里用的是**绝对 px 偏移**，不是比例缩放。
// 比例缩放会把 13px 变成 14.6px / 16.25px —— 也就是 +1.6 / +3.25，
// 与「+2px / +4px」的字面要求对不上，而且列表 / 网格的行高换算会变得没法口算。
// 偏移量是 0 / 2 / 4 三个整数，行高因此也是「原值 + 偏移」这种一眼能验的算式。
//
// 基准正文字号 = 16px，出处是 src/styles/preflight.css 的 `html, :host` 块：
// 那里只设了 font-family 与 line-height: 1.5，**没有** font-size，
// 所以浏览器用的是 UA 默认的 16px（也就是 1rem = 16px）。
//
// 生效链路（两条，各乘一次，不要重复放大）：
//   · 字号：font-size: calc(13px + var(--font-size-offset, 0px));
//   · 行高：base + offset（JS 侧算，参数从 store 一路传到度量来源）。
//
// ⚠️ 不要改 `html { font-size }` —— 全仓有几百处 rem 布局（含固定高度的侧栏与工具栏），
// 改根字号会把那些布局一起顶开。字号只按上面的方式逐处显式放大。

/** 三档：默认 / 大 / 特大。 */
export type FontSizeTier = 'default' | 'large' | 'extra-large';

/** 全部档位（设置页下拉的顺序）。 */
export const FONT_SIZE_TIERS: readonly FontSizeTier[] = ['default', 'large', 'extra-large'];

/**
 * 每档相对当前默认字号增加的**绝对像素数**。
 *
 * 默认 = 0（与现状完全一致）；大 = +2px；特大 = +4px。
 */
const FONT_SIZE_OFFSETS: Record<FontSizeTier, number> = {
  'default': 0,
  'large': 2,
  'extra-large': 4,
};

/**
 * 判断一个值是不是合法档位。
 *
 * 用户设置可能来自被手改过的配置文件，运行时不能假定它落在三档内。
 */
export function isFontSizeTier(value: unknown): value is FontSizeTier {
  return typeof value === 'string' && Object.hasOwn(FONT_SIZE_OFFSETS, value);
}

/**
 * 把档位解析成偏移量（0 / 2 / 4）。
 *
 * 非法值一律回落到 0（默认档）—— 宁可什么都不放大，也不能让布局炸掉。
 */
export function resolveFontSizeOffset(tier: FontSizeTier | string | undefined | null): number {
  if (!isFontSizeTier(tier)) {
    return FONT_SIZE_OFFSETS['default'];
  }

  return FONT_SIZE_OFFSETS[tier];
}
