// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 字体大小三档（需求 3，2026-10-31）。

import { describe, expect, it } from 'vitest';
import {
  FONT_SIZE_TIERS,
  isFontSizeTier,
  resolveFontSizeOffset,
} from '@/utils/font-size';

describe('font size tiers', () => {
  it('exposes exactly three tiers in display order', () => {
    expect(FONT_SIZE_TIERS).toEqual(['default', 'large', 'extra-large']);
  });

  it('maps each tier to its absolute pixel offset', () => {
    expect(resolveFontSizeOffset('default')).toBe(0);
    expect(resolveFontSizeOffset('large')).toBe(2);
    expect(resolveFontSizeOffset('extra-large')).toBe(4);
  });

  it('falls back to the default offset for unknown values', () => {
    expect(resolveFontSizeOffset(undefined)).toBe(0);
    expect(resolveFontSizeOffset(null)).toBe(0);
    expect(resolveFontSizeOffset('')).toBe(0);
    expect(resolveFontSizeOffset('xlarge')).toBe(0);
    expect(resolveFontSizeOffset('extraLarge')).toBe(0);
    expect(resolveFontSizeOffset('LARGE')).toBe(0);
  });

  it('recognizes valid tier names only', () => {
    for (const tier of FONT_SIZE_TIERS) {
      expect(isFontSizeTier(tier)).toBe(true);
    }

    expect(isFontSizeTier('xlarge')).toBe(false);
    expect(isFontSizeTier('toString')).toBe(false);
    expect(isFontSizeTier(undefined)).toBe(false);
    expect(isFontSizeTier(2)).toBe(false);
  });
});
