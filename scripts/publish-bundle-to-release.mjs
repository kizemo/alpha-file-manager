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

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const bundleDir = path.join(repoRoot, 'src-tauri', 'target', 'release', 'bundle');
const releaseDir = path.join(repoRoot, 'release');

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
    const target = path.join(releaseDir, platformEntry.name);
    fs.rmSync(target, { force: true });
    fs.cpSync(platformPath, target);
    published.push(platformEntry.name);
    continue;
  }

  for (const artifact of fs.readdirSync(platformPath, { withFileTypes: true })) {
    const source = path.join(platformPath, artifact.name);
    const target = path.join(releaseDir, artifact.name);

    // Only ever replace an entry carrying the same name. Anything else already
    // in release/ (README, checksum manifests, installers from another source)
    // is left alone.
    fs.rmSync(target, { recursive: true, force: true });
    fs.cpSync(source, target, { recursive: true });

    published.push(artifact.name);
  }
}

if (published.length === 0) {
  console.log('[publish-bundle] bundle/ 下没有可发布的产物。');
}

console.log(`[publish-bundle] 已发布到 ${path.relative(repoRoot, releaseDir)}/`);

for (const name of published) {
  console.log(`  - ${name}`);
}
