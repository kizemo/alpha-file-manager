// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
//
// 验证 publish-bundle-to-release.mjs 的「不覆盖已发布资产」行为。
//
// 为什么需要它：
//   release/ 同时是 GitHub Releases 的本地镜像和本地构建产物的落地目录。
//   旧实现是 rmSync + cpSync 的无条件覆盖，于是「给已发布版本重新构建一次」
//   就会把本地构建写进已发布版本的文件名里 —— 本地包冒充成官方发布物。
//   2026-11-02 核对发现 49 个本地安装包里有 12 个处于这种状态。
//
// 判定方式：在临时沙箱里造三种情形，各跑一次真实脚本，
//   1. 目标不存在        → 应当照常落到 release/ 顶层
//   2. 目标存在且内容相同 → 应当什么都不做
//   3. 目标存在且内容不同 → 应当改写 local-builds/，并保留原文件不动
// 外加一条负对照：把旧行为（无条件覆盖）在同一沙箱里跑一遍，
// 证明第 3 条断言确实会咬 —— 否则这个检查等于没有检查。
//
// 用法：node scripts/verify-publish-bundle-quarantine.mjs

import { spawnSync } from 'node:child_process';
import crypto from 'node:crypto';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const repoRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const scriptUnderTest = path.join(repoRoot, 'scripts', 'publish-bundle-to-release.mjs');

function sha256(file) {
  return crypto.createHash('sha256').update(fs.readFileSync(file)).digest('hex');
}

function makeSandbox() {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'afm-publish-quarantine-'));
  const bundleNsis = path.join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis');
  const releaseDir = path.join(root, 'release');
  fs.mkdirSync(bundleNsis, { recursive: true });
  fs.mkdirSync(releaseDir, { recursive: true });
  fs.mkdirSync(path.join(root, 'scripts'), { recursive: true });
  fs.copyFileSync(scriptUnderTest, path.join(root, 'scripts', 'publish-bundle-to-release.mjs'));
  return { root, bundleNsis, releaseDir };
}

function writeFile(file, content) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, content);
}

function runScript(sandbox) {
  const result = spawnSync(
    process.execPath,
    [path.join(sandbox.root, 'scripts', 'publish-bundle-to-release.mjs')],
    { encoding: 'utf8' },
  );
  if (result.status !== 0) {
    throw new Error(`脚本退出码 ${result.status}\n${result.stdout ?? ''}\n${result.stderr ?? ''}`);
  }
  return result.stdout ?? '';
}

const failures = [];
function check(label, condition, detail = '') {
  if (condition) {
    console.log(`  PASS ${label}`);
  }
  else {
    console.log(`  FAIL ${label}${detail ? ` — ${detail}` : ''}`);
    failures.push(label);
  }
}

// ---------------------------------------------------------------- 沙箱场景
const sandbox = makeSandbox();
const sameName = 'Alpha File Manager_9.9.9_x64-setup.exe';
const diffName = 'Alpha File Manager_9.9.8_x64-setup.exe';
const newName = 'Alpha File Manager_9.9.7_x64-setup.exe';

const sameBundleBytes = Buffer.from('bundle-and-release-are-identical');
const sameReleaseBytes = Buffer.from('bundle-and-release-are-identical');
const diffBundleBytes = Buffer.from('fresh-local-build-bytes');
const diffReleaseBytes = Buffer.from('published-asset-bytes');
const newBundleBytes = Buffer.from('never-published-build-bytes');

writeFile(path.join(sandbox.bundleNsis, sameName), sameBundleBytes);
writeFile(path.join(sandbox.bundleNsis, diffName), diffBundleBytes);
writeFile(path.join(sandbox.bundleNsis, newName), newBundleBytes);
writeFile(path.join(sandbox.releaseDir, sameName), sameReleaseBytes);
writeFile(path.join(sandbox.releaseDir, diffName), diffReleaseBytes);

const publishedBefore = sha256(path.join(sandbox.releaseDir, diffName));
const stdout = runScript(sandbox);

console.log('场景 1/2/3：');
check('目标不存在时落到 release/ 顶层', fs.existsSync(path.join(sandbox.releaseDir, newName)));
check(
  '目标已存在且内容相同时原文件不变',
  sha256(path.join(sandbox.releaseDir, sameName)) === sha256(path.join(sandbox.bundleNsis, sameName)),
);
check(
  '目标已存在且内容不同时原文件不被覆盖',
  sha256(path.join(sandbox.releaseDir, diffName)) === publishedBefore,
  `期望 ${publishedBefore}，实际 ${sha256(path.join(sandbox.releaseDir, diffName))}`,
);
check(
  '内容不同的产物改写入 local-builds/',
  fs.existsSync(path.join(sandbox.releaseDir, 'local-builds', diffName)),
);
if (fs.existsSync(path.join(sandbox.releaseDir, 'local-builds', diffName))) {
  check(
    'local-builds/ 里的内容等于构建产物',
    sha256(path.join(sandbox.releaseDir, 'local-builds', diffName)) === sha256(path.join(sandbox.bundleNsis, diffName)),
  );
}
check('脚本对冲突给出了告警', stdout.includes('local-builds'), stdout.trim());

// ---------------------------------------------------------------- 负对照
// 把旧行为（rmSync + cpSync 无条件覆盖）在同一沙箱里跑一遍，
// 证明「原文件不被覆盖」这条断言确实会咬 —— 否则这个检查等于没有检查。
const legacySandbox = makeSandbox();
writeFile(path.join(legacySandbox.bundleNsis, diffName), diffBundleBytes);
writeFile(path.join(legacySandbox.releaseDir, diffName), diffReleaseBytes);
const legacyBefore = sha256(path.join(legacySandbox.releaseDir, diffName));
fs.rmSync(path.join(legacySandbox.releaseDir, diffName), { recursive: true, force: true });
fs.cpSync(path.join(legacySandbox.bundleNsis, diffName), path.join(legacySandbox.releaseDir, diffName));
check(
  '负对照：旧行为确实会覆盖原文件（断言有牙）',
  sha256(path.join(legacySandbox.releaseDir, diffName)) !== legacyBefore,
);

fs.rmSync(sandbox.root, { recursive: true, force: true });
fs.rmSync(legacySandbox.root, { recursive: true, force: true });

if (failures.length > 0) {
  console.error(`\n[verify-publish-quarantine] 失败 ${failures.length} 项：${failures.join('、')}\n`);
  process.exit(1);
}
console.log('\n[verify-publish-quarantine] 全部通过\n');
