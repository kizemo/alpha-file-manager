// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 58 —— 收藏「标题 / 路径」编辑的纯逻辑单测。
//
// 存在性判定要问后端，所以 `checkFavoritePath` 把两个探针做成可注入的依赖；
// 这里塞假实现，于是整套规则可以纯离线地测。

import { describe, expect, it } from 'vitest';
import {
  checkFavoritePath,
  findDuplicateFavoritePath,
  normalizeFavoritePathInput,
} from '@/utils/favorite-edit';

describe('收藏路径归一化', () => {
  it('去掉首尾空白与成对的双引号', () => {
    expect(normalizeFavoritePathInput('  "C:\\Users\\test"  ')).toBe('C:\\Users\\test');
  });

  it('只去掉首尾各一个引号', () => {
    // 两端各有两个引号时，只脱掉最外面的各一个。
    expect(normalizeFavoritePathInput('""C:\\a""')).toBe('"C:\\a"');
  });

  it('空串归一化后仍是空串', () => {
    expect(normalizeFavoritePathInput('   ')).toBe('');
  });
});

describe('收藏路径重复判定', () => {
  it('归一化之后相同就算重复', () => {
    expect(findDuplicateFavoritePath('"C:/a"', ['C:/a'])).toBe('C:/a');
  });

  it('不重复时返回 null', () => {
    expect(findDuplicateFavoritePath('C:/b', ['C:/a'])).toBeNull();
  });

  it('空候选不算重复', () => {
    expect(findDuplicateFavoritePath('   ', ['C:/a'])).toBeNull();
  });
});

describe('收藏路径校验', () => {
  it('重复路径先于存在性判定被挡下（不必问后端）', async () => {
    let probed = false;

    const result = await checkFavoritePath('C:/a', ['C:/a'], {
      exists: async () => {
        probed = true;
        return true;
      },
      isRegularFile: async () => false,
    });

    expect(result.status).toBe('duplicate');
    expect(probed).toBe(false);
  });

  it('空路径直接判 missing', async () => {
    const result = await checkFavoritePath('   ', []);

    expect(result.status).toBe('missing');
  });

  it('路径不存在时返回 missing', async () => {
    const result = await checkFavoritePath('C:/missing', [], {
      exists: async () => false,
      isRegularFile: async () => false,
    });

    expect(result.status).toBe('missing');
  });

  it('存在且是普通文件时返回 ok 与 isFile=true', async () => {
    const result = await checkFavoritePath('C:/a.txt', [], {
      exists: async () => true,
      isRegularFile: async () => true,
    });

    expect(result.status).toBe('ok');
    expect(result.isFile).toBe(true);
  });

  it('存在且是目录时返回 ok 与 isFile=false', async () => {
    const result = await checkFavoritePath('C:/dir', [], {
      exists: async () => true,
      isRegularFile: async () => false,
    });

    expect(result.status).toBe('ok');
    expect(result.isFile).toBe(false);
  });

  it('存在性探针抛错时按「不存在」处理', async () => {
    const result = await checkFavoritePath('C:/a', [], {
      exists: async () => {
        throw new Error('boom');
      },
      isRegularFile: async () => false,
    });

    expect(result.status).toBe('missing');
  });

  it('文件判定抛错时按目录处理，不误判成文件', async () => {
    const result = await checkFavoritePath('C:/a', [], {
      exists: async () => true,
      isRegularFile: async () => {
        throw new Error('boom');
      },
    });

    expect(result.status).toBe('ok');
    expect(result.isFile).toBe(false);
  });
});
