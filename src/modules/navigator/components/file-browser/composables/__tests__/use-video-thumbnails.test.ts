// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import { normalizeVideoThumbnailSize } from '../use-video-thumbnails';

describe('normalizeVideoThumbnailSize', () => {
  it('uses one cache size for video thumbnails', () => {
    expect(normalizeVideoThumbnailSize()).toEqual({
      width: 384,
      height: 271,
    });
  });
});
