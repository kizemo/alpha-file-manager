// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-28：守卫方向反转。
//
// 原守卫（background-source-import-guard.ts）禁止打包期引入
// src/assets/media/source-backgrounds/ 下的素材，理由是上游要压安装包体积。
// 现在用户要求「所有默认背景都下载到本地并打包进安装包」，这条约束正好反过来：
// 每一条内置背景都必须有本地素材随包分发，否则主页封面只能回落到远程下载，
// 而远程源不可达 ⇒ 点了没反应。
//
// 所以守卫改成校验「素材目录」与「内置背景清单」严格一一对应：
// 少了 → 构建期报错（这条背景会在运行期变成死的远程下载）；
// 多了 → 构建期报错（白占安装包体积）。

import fs from 'node:fs';
import path from 'node:path';
import type { Plugin } from 'vite';

export const SOURCE_BACKGROUNDS_DIR_SEGMENTS = [
  'src',
  'assets',
  'media',
  'source-backgrounds',
] as const;

export const DEFAULT_BACKGROUND_DIR_SEGMENTS = [
  'src',
  'assets',
  'media',
  'default-background',
] as const;

export function listBackgroundAssetFileNames(assetDir: string): string[] {
  if (!fs.existsSync(assetDir)) {
    return [];
  }

  return fs
    .readdirSync(assetDir, { withFileTypes: true })
    .filter(entry => entry.isFile())
    .map(entry => entry.name)
    .sort();
}

export function assertBackgroundAssetsMatchMedia(
  mediaFileNames: string[],
  assetFileNames: string[],
  defaultFileName: string,
): void {
  const expected = mediaFileNames.filter(name => name !== defaultFileName).sort();
  const actual = [...assetFileNames].sort();
  const missing = expected.filter(name => !actual.includes(name));
  const orphaned = actual.filter(name => !expected.includes(name));

  if (missing.length === 0 && orphaned.length === 0) {
    return;
  }

  const details: string[] = [];

  if (missing.length > 0) {
    details.push(`missing from the bundle: ${missing.join(', ')}`);
  }

  if (orphaned.length > 0) {
    details.push(`not referenced by any built-in background: ${orphaned.join(', ')}`);
  }

  throw new Error(
    `Built-in background assets do not match the built-in background list (${details.join('; ')}). `
    + 'Every built-in background must ship inside the app bundle.',
  );
}

export function assertDefaultBackgroundAssetExists(
  defaultFileName: string,
  defaultAssetDir: string,
): void {
  if (!defaultFileName) {
    throw new Error('The default background file name must not be empty.');
  }

  const defaultAssetPath = path.join(defaultAssetDir, defaultFileName);

  if (!fs.existsSync(defaultAssetPath)) {
    throw new Error(
      `The default background asset is missing from the bundle: ${defaultAssetPath}`,
    );
  }
}

export type BackgroundSourceBundleGuardOptions = {
  mediaFileNames: string[];
  defaultFileName: string;
};

export function createBackgroundSourceBundleGuard(
  options: BackgroundSourceBundleGuardOptions,
): Plugin {
  return {
    name: 'background-source-bundle-guard',
    configResolved(config) {
      const assetDir = path.join(config.root, ...SOURCE_BACKGROUNDS_DIR_SEGMENTS);
      const defaultAssetDir = path.join(config.root, ...DEFAULT_BACKGROUND_DIR_SEGMENTS);

      assertBackgroundAssetsMatchMedia(
        options.mediaFileNames,
        listBackgroundAssetFileNames(assetDir),
        options.defaultFileName,
      );
      assertDefaultBackgroundAssetExists(options.defaultFileName, defaultAssetDir);
    },
  };
}
