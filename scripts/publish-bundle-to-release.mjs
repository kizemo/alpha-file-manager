// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * Publish the installers produced by `tauri build` into the repository-level
 * `release/` folder.
 *
 * Why a copy instead of moving Cargo's target directory:
 * `tauri build` always writes bundles to `<cargo-target-dir>/release/bundle/`
 * and offers no `--out-dir` option, so the only way to relocate that output is
 * the `CARGO_TARGET_DIR` environment variable — which would drag ~17 GB of
 * `debug/`, `incremental/` and fingerprint data into the repository root and
 * break any tooling that assumes the standard `src-tauri/target` layout.
 * Copying just the finished installers keeps the build chain untouched.
 *
 * The per-platform folders under `bundle/` (nsis/, msi/, deb/, appimage/, ...)
 * exist to satisfy the bundler, not the person looking for the artifact, so
 * this flattens one level: `bundle/nsis/App_1.0_setup.exe` lands at
 * `release/App_1.0_setup.exe` rather than `release/nsis/App_1.0_setup.exe`.
 *
 * This restores the layout the project shipped upstream with: `release/` held
 * the installers directly, and `.gitignore` still names them explicitly.
 */

import crypto from 'node:crypto';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bundleDir = path.join(repoRoot, 'src-tauri', 'target', 'release', 'bundle');
const releaseDir = path.join(repoRoot, 'release');
const localBuildDir = path.join(releaseDir, 'local-builds');

/** 两个文件是否逐字节一致（先比大小，再比 sha256）。 */
function sameContent(a, b) {
  const statA = fs.statSync(a);
  const statB = fs.statSync(b);
  if (!statA.isFile() || !statB.isFile()) return false;
  if (statA.size !== statB.size) return false;
  const hash = file => crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
  return hash(a) === hash(b);
}

/**
 * 把构建产物落到 release/，但**绝不覆盖内容不同的同名文件**。
 *
 * release/ 同时承担两个角色：GitHub Releases 的本地镜像 + 本地构建产物的落地目录。
 * 过去这里是 `rmSync` + `cpSync` 的无条件覆盖，于是「给已发布版本重新构建一次」
 * 就把本地构建写进了已发布版本的文件名里 —— 一个本地包就此冒充成官方发布物。
 * 2026-11-02 核对发现 49 个本地安装包里有 12 个处于这种状态
 * （3.8.0 / 3.9.0 / 3.9.1 等），它们的字节与线上已发布资产不同。
 *
 * 现在的规则：
 *   - 目标不存在            → 照常落地到 release/ 顶层
 *   - 目标存在且内容相同    → 什么都不做（同一次构建的重复执行）
 *   - 目标存在但内容不同    → 它多半是已发布资产的副本，改写到 local-builds/ 并告警
 */
function placeArtifact(source, name) {
  const target = path.join(releaseDir, name);

  if (!fs.existsSync(target)) {
    fs.cpSync(source, target, { recursive: true });
    return name;
  }

  if (sameContent(source, target)) {
    return name;
  }

  fs.mkdirSync(localBuildDir, { recursive: true });
  const quarantined = path.join(localBuildDir, name);
  fs.rmSync(quarantined, { recursive: true, force: true });
  fs.cpSync(source, quarantined, { recursive: true });
  console.warn(`[publish-bundle] ${name} 已存在且内容不同（多半是已发布资产的副本），`);
  console.warn(`[publish-bundle]   改写入 ${path.relative(repoRoot, quarantined)}，不覆盖原文件。`);
  return path.join('local-builds', name);
}

if (!fs.existsSync(bundleDir)) {
  console.log(`[publish-bundle] 未找到 ${path.relative(repoRoot, bundleDir)}，跳过。`);
  process.exit(0);
}

fs.mkdirSync(releaseDir, { recursive: true });

const published = [];

for (const platformEntry of fs.readdirSync(bundleDir, { withFileTypes: true })) {
  const platformPath = path.join(bundleDir, platformEntry.name);

  // A file sitting directly under bundle/ is already flat; keep it as-is.
  if (!platformEntry.isDirectory()) {
    published.push(placeArtifact(platformPath, platformEntry.name));
    continue;
  }

  for (const artifact of fs.readdirSync(platformPath, { withFileTypes: true })) {
    published.push(placeArtifact(path.join(platformPath, artifact.name), artifact.name));
  }
}

if (published.length === 0) {
  console.log('[publish-bundle] bundle/ 下没有可发布的产物。');
}

console.log(`[publish-bundle] 已发布到 ${path.relative(repoRoot, releaseDir)}/`);

for (const name of published) {
  console.log(`  - ${name}`);
}
