// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：锁住「内置背景全部随包分发」这条不变量。
//
// 这条不变量被破坏过一次：上游为了压安装包体积把全尺寸素材踢出 bundle，
// 改走运行时远程下载，源站不可达时主页封面点了没反应。
// 下面的断言直接对着「运行时真的能拿到的资源索引」和「磁盘上的素材目录」，
// 任何一条内置背景缺素材，测试立刻红。

import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';
import { backgroundMedia, DEFAULT_BACKGROUND_FILE_NAME } from '../background-media';
import {
  buildBackgroundAssetIndex,
  fullSizeBackgroundAssets,
  previewBackgroundAssets,
  resolveBackgroundAssetUrl,
} from '../background-media-assets';

// vitest 的 import.meta.url 不是 file: 协议（fileURLToPath 会抛 "The URL must be of scheme file"），
// 所以这里按「vitest 进程的工作目录 = 仓库根」来定位素材目录。
const projectRoot = process.cwd();
const sourceBackgroundsDir = path.join(projectRoot, 'src', 'assets', 'media', 'source-backgrounds');

const builtinFileNames = backgroundMedia
  .map(item => item.fileName)
  .filter(fileName => fileName !== DEFAULT_BACKGROUND_FILE_NAME);

describe('background media assets', () => {
  it('indexes glob modules by file name', () => {
    const index = buildBackgroundAssetIndex({
      '/repo/src/assets/media/source-backgrounds/a.jpg': '/assets/a.jpg',
      'C:\\repo\\src\\assets\\media\\source-backgrounds\\b.mp4': '/assets/b.mp4',
    });

    expect(index).toEqual({
      'a.jpg': '/assets/a.jpg',
      'b.mp4': '/assets/b.mp4',
    });
  });

  it('ships a full-size asset for every built-in background', () => {
    for (const fileName of builtinFileNames) {
      expect(
        resolveBackgroundAssetUrl(fullSizeBackgroundAssets, fileName),
        `built-in background "${fileName}" has no bundled full-size asset`,
      ).toBeTruthy();
    }

    expect(Object.keys(fullSizeBackgroundAssets).length).toBe(builtinFileNames.length);
  });

  it('ships a preview asset for every built-in background', () => {
    for (const fileName of builtinFileNames) {
      const baseName = fileName.replace(/\.[^.]+$/, '');

      expect(
        resolveBackgroundAssetUrl(previewBackgroundAssets, baseName),
        `built-in background "${fileName}" has no bundled preview asset`,
      ).toBeTruthy();
    }
  });

  it('matches the asset directory on disk exactly', () => {
    const filesOnDisk = fs
      .readdirSync(sourceBackgroundsDir, { withFileTypes: true })
      .filter(entry => entry.isFile())
      .map(entry => entry.name)
      .sort();
    const expected = [...builtinFileNames].sort();

    expect(filesOnDisk).toEqual(expected);
  });

  it('keeps every bundled asset non-empty', () => {
    for (const fileName of builtinFileNames) {
      const assetPath = path.join(sourceBackgroundsDir, fileName);

      expect(fs.statSync(assetPath).size, `"${fileName}" is empty`).toBeGreaterThan(0);
    }
  });
});
