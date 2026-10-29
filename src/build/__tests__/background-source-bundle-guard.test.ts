// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：守卫方向反转后的测试。
// 原测试锁的是「禁止把 source-backgrounds 打进包」，那条约束已被用户要求推翻；
// 现在锁的是相反的不变量：内置背景清单与素材目录必须严格一一对应。

import { describe, expect, it } from 'vitest';
import path from 'node:path';
import {
  assertBackgroundAssetsMatchMedia,
  assertDefaultBackgroundAssetExists,
  listBackgroundAssetFileNames,
} from '../background-source-bundle-guard';

const DEFAULT_FILE_NAME = 'Exile by Aleksey Hoffman.jpg';

// vitest 的 import.meta.url 不是 file: 协议，改用进程工作目录（= 仓库根）定位素材目录。
const projectRoot = process.cwd();
const sourceBackgroundsDir = path.join(projectRoot, 'src', 'assets', 'media', 'source-backgrounds');
const defaultBackgroundDir = path.join(projectRoot, 'src', 'assets', 'media', 'default-background');

describe('background source bundle guard', () => {
  it('passes when every built-in background has exactly one bundled asset', () => {
    expect(() => assertBackgroundAssetsMatchMedia(
      [DEFAULT_FILE_NAME, 'a.jpg', 'b.mp4'],
      ['a.jpg', 'b.mp4'],
      DEFAULT_FILE_NAME,
    )).not.toThrow();
  });

  it('fails when a built-in background has no local asset', () => {
    expect(() => assertBackgroundAssetsMatchMedia(
      [DEFAULT_FILE_NAME, 'a.jpg', 'b.mp4'],
      ['a.jpg'],
      DEFAULT_FILE_NAME,
    )).toThrow(/missing from the bundle: b\.mp4/i);
  });

  it('fails when an asset is not referenced by any built-in background', () => {
    expect(() => assertBackgroundAssetsMatchMedia(
      [DEFAULT_FILE_NAME, 'a.jpg'],
      ['a.jpg', 'orphan.jpg'],
      DEFAULT_FILE_NAME,
    )).toThrow(/not referenced by any built-in background: orphan\.jpg/i);
  });

  it('never treats the default background as a source-backgrounds asset', () => {
    expect(() => assertBackgroundAssetsMatchMedia(
      [DEFAULT_FILE_NAME],
      [],
      DEFAULT_FILE_NAME,
    )).not.toThrow();
  });

  it('lists only files from the asset directory', () => {
    const fileNames = listBackgroundAssetFileNames(sourceBackgroundsDir);

    expect(fileNames.length).toBeGreaterThan(0);
    expect(fileNames.every(name => name.includes('.'))).toBe(true);
  });

  it('fails when the default background asset is missing', () => {
    expect(() => assertDefaultBackgroundAssetExists(
      'no-such-file.jpg',
      defaultBackgroundDir,
    )).toThrow(/missing from the bundle/i);
  });
});
