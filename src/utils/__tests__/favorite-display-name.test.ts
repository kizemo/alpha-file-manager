// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 37（2026-11-22）—— 收藏显示名（别名）。
//
// 这个函数是仪表盘收藏卡与侧边栏「快速访问」共用的显示名来源，
// 所以它必须同时守住两条：别名优先、没有别名时**完全等于**原来的行为。

import { describe, expect, it } from 'vitest';
import { resolveFavoriteDisplayName } from '../favorite-display-name';

describe('resolveFavoriteDisplayName', () => {
  it('有别名时用别名', () => {
    expect(resolveFavoriteDisplayName('/home/user/notes.md', '我的笔记')).toBe('我的笔记');
  });

  it('别名为空串时回退到原始文件名', () => {
    expect(resolveFavoriteDisplayName('/home/user/notes.md', '')).toBe('notes.md');
  });

  it('别名缺省或为 null 时回退到原始文件名', () => {
    expect(resolveFavoriteDisplayName('/home/user/notes.md', undefined)).toBe('notes.md');
    expect(resolveFavoriteDisplayName('/home/user/notes.md', null)).toBe('notes.md');
  });

  it('别名只有空白时也算没有别名', () => {
    expect(resolveFavoriteDisplayName('/home/user/notes.md', '   ')).toBe('notes.md');
  });

  it('别名去掉首尾空白', () => {
    expect(resolveFavoriteDisplayName('/home/user/notes.md', '  我的笔记  ')).toBe('我的笔记');
  });

  it('路径为空时返回空串', () => {
    expect(resolveFavoriteDisplayName('', '')).toBe('');
    expect(resolveFavoriteDisplayName('', undefined)).toBe('');
  });

  it('没有别名时把翻译函数交给 getPathDisplayName（虚拟位置用得到）', () => {
    function translate(key: string) {
      return `T:${key}`;
    }

    expect(resolveFavoriteDisplayName('sfm://locations', '', translate)).toBe('T:locations');
  });

  // 判别性用例：虚拟位置没有文件名，别名必须压过翻译出来的标签。
  it('有别名时压过 getPathDisplayName 给出的名字', () => {
    function translate(key: string) {
      return `T:${key}`;
    }

    expect(resolveFavoriteDisplayName('sfm://locations', '我的位置', translate)).toBe('我的位置');
  });
});
