// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import { dropHistoryEntriesForVersion } from '../release-history.mjs';

describe('dropHistoryEntriesForVersion', () => {
  it('去掉同版本的全部条目，其它版本原样保留', () => {
    const history = [
      { version: '3.12.0', url: 'a' },
      { version: '3.12.0', url: 'b' },
      { version: '3.11.0', url: 'c' },
    ];

    const result = dropHistoryEntriesForVersion(history, '3.12.0');

    expect(result.removed).toBe(2);
    expect(result.history.map(entry => entry.url)).toEqual(['c']);
  });

  it('没有同版本条目时不动原数组', () => {
    const history = [{ version: '3.11.0', url: 'c' }];

    const result = dropHistoryEntriesForVersion(history, '3.12.0');

    expect(result.removed).toBe(0);
    expect(result.history).toEqual(history);
    expect(result.history).not.toBe(history);
  });

  it('history 缺失或是 null 时返回空数组而不是抛错', () => {
    expect(dropHistoryEntriesForVersion(undefined, '3.12.0')).toEqual({ history: [], removed: 0 });
    expect(dropHistoryEntriesForVersion(null, '3.12.0')).toEqual({ history: [], removed: 0 });
  });

  it('版本号缺失时不去重，避免误删没有版本号的条目', () => {
    const history = [{ url: 'x' }, { version: '3.12.0', url: 'y' }];

    const result = dropHistoryEntriesForVersion(history, undefined);

    expect(result.removed).toBe(0);
    expect(result.history).toHaveLength(2);
  });

  it('容忍 history 里的 null 元素', () => {
    const history = [null, { version: '3.12.0', url: 'y' }];

    const result = dropHistoryEntriesForVersion(history, '3.12.0');

    expect(result.removed).toBe(1);
    expect(result.history).toEqual([null]);
  });
});
