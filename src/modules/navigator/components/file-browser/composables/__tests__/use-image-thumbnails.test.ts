// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import { normalizeImageThumbnailMaxDimension } from '../use-image-thumbnails';

describe('normalizeImageThumbnailMaxDimension', () => {
  it('uses the default size when no valid size is provided', () => {
    expect(normalizeImageThumbnailMaxDimension()).toBe(512);
    expect(normalizeImageThumbnailMaxDimension(Number.NaN)).toBe(512);
  });

  it('uses one cache size for every requested thumbnail size', () => {
    expect(normalizeImageThumbnailMaxDimension(1)).toBe(512);
    expect(normalizeImageThumbnailMaxDimension(340)).toBe(512);
    expect(normalizeImageThumbnailMaxDimension(10_000)).toBe(512);
  });
});
