// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION: 守卫「更新安装不再因应用未完全关闭而失败」这条修复的结构性前提。
//
// 背景（2026-10-29 真机复现）：
//   安装 3.6.5 时，安装向导调起 **已发布的旧卸载器**（3.6.4/3.6.5 的 uninstall.exe）。
//   旧卸载器的 PREUNINSTALL 钩子只「发起」杀进程，不等待进程退出、也不校验 exe 是否解锁；
//   约 2.4 秒后旧卸载器模板里的 CheckIfAppIsRunning 用 Restart Manager 仍然查到
//   $INSTDIR\alpha-file-manager.exe 被占用 -> 弹「正在运行！点击确定以终止运行」
//   -> 用户点确定 -> RmShutdown 失败 -> Abort（退出码非 0、无 POSTUNINSTALL）
//   -> setup 判卸载失败 -> 整个更新安装失败。
//
// 旧卸载器是**已经在用户机器上的二进制**，改不了。唯一还能在「调用旧卸载器之前」动手的
// 地方就是 setup 自己用的 NSIS 模板 —— 所以本仓库 vendor 了 Tauri 的 installer.nsi，
// 并在 ExecWait '$R1' $0 之前插入一个钩子。
//
// 因此下面这些断言是这条修复的**前提**，不是风格检查：
//   1. 补丁必须在旧卸载器调用之前（位置错 = 修复失效，而且不会报错）；
//   2. 补丁必须是可选的（hooks 缺失时退回上游行为，模板仍要能编译）；
//   3. vendor 模板必须仍然是**未渲染的模板**（含 handlebars 占位符），
//      渲染过的模板交给 bundler 会直接编译失败；
//   4. @tauri-apps/cli 必须精确 pin —— vendor 模板与 CLI 内嵌模板必须同版本，
//      CLI 一升级，vendor 模板就会与上游漂移（CLI 的 npm 包不带模板文件，无法自动比对）。
//
// 另外守卫 kill-app-tree.ps1 的能力边界：等待真正退出 + 独占打开校验，
// 缺了「独占校验」就等于回到旧行为（杀完就返回，句柄还没释放）。
//

import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

// vitest 的 import.meta.url 不是 file:// URL（ERR_INVALID_URL_SCHEME），
// 所以从 cwd 往上找包含 installer 目录的仓库根，而不是用 import.meta 推导。
function findInstallerDir(): string {
  let dir = process.cwd();

  for (let i = 0; i < 10; i += 1) {
    const candidate = path.join(dir, 'src-tauri', 'installer');

    if (fs.existsSync(path.join(candidate, 'kill-app-tree.ps1'))) {
      return candidate;
    }

    const parent = path.dirname(dir);

    if (parent === dir) break;
    dir = parent;
  }

  throw new Error('找不到 src-tauri/installer 目录（cwd=' + process.cwd() + '）');
}

function countOccurrences(haystack: string, needle: string): number {
  if (!needle) return 0;

  return haystack.split(needle).length - 1;
}

function readCargoPackageVersion(cargoToml: string): string | undefined {
  let inside = false;

  for (const line of cargoToml.split(/\r?\n/)) {
    if (/^\s*\[package\]\s*$/.test(line)) {
      inside = true;
      continue;
    }

    if (inside && /^\s*\[/.test(line)) break;

    if (inside) {
      const match = /^\s*version\s*=\s*"([^"]+)"/.exec(line);

      if (match) return match[1];
    }
  }

  return undefined;
}

describe('NSIS 模板补丁（更新安装不再被「应用未完全关闭」打断）', () => {
  const installerDir = findInstallerDir();
  const repoRoot = path.resolve(installerDir, '..', '..');
  const templatePath = path.join(installerDir, 'installer.nsi');
  const hooksPath = path.join(installerDir, 'hooks.nsh');
  const template = fs.readFileSync(templatePath, 'utf8');
  const templateLines = template.split(/\r?\n/);
  const hooks = fs.readFileSync(hooksPath, 'utf8');

  it('vendor 模板存在，且仍是未渲染的 handlebars 模板', () => {
    expect(fs.existsSync(templatePath)).toBe(true);
    expect(template.length).toBeGreaterThan(1000);

    // 这三个占位符是 Tauri bundler 渲染时替换的。渲染过的模板会被 bundler
    // 当成普通 .nsi 再编译一次，直接失败 —— 所以这里必须仍是占位符形态。
    expect(template).toContain('!include "{{installer_hooks}}"');
    expect(template).toContain('!define PRODUCTNAME "{{product_name}}"');
    expect(template).toContain('!define MAINBINARYNAME "{{main_binary_name}}"');

    // 上游的关键结构必须还在，否则说明 vendor 时截断错了。
    expect(template).toContain('reinst_uninstall:');
    expect(template).toContain('CheckIfAppIsRunning');
  });

  it('补丁插在旧卸载器 ExecWait 之前，且落在 NSIS 分支内', () => {
    const patchIdx = templateLines.findIndex((l) => l.trim() === '!ifdef AFM_HOOK_PRELEGACY_UNINSTALL');
    const insertIdx = templateLines.findIndex((l) => l.trim() === '!insertmacro NSIS_HOOK_PRELEGACY_UNINSTALL');

    expect(patchIdx).toBeGreaterThan(-1);
    expect(insertIdx).toBe(patchIdx + 1);

    const reinstIdx = templateLines.findIndex((l) => l.trim() === 'reinst_uninstall:');
    expect(reinstIdx).toBeGreaterThan(-1);
    expect(patchIdx).toBeGreaterThan(reinstIdx);

    // 模板里有两处 ExecWait '$R1' $0：Wix 分支一处、NSIS 分支一处。
    const execWaitIdx = templateLines
      .map((l, i) => ({ l, i }))
      .filter(({ l }) => l.trim() === "ExecWait '$R1' $0")
      .map(({ i }) => i);

    expect(execWaitIdx.length).toBe(2);

    // 补丁必须夹在两者之间：Wix 分支不受影响，NSIS 分支在补丁之后才调旧卸载器。
    expect(patchIdx).toBeGreaterThan(execWaitIdx[0]);
    expect(patchIdx).toBeLessThan(execWaitIdx[1]);
  });

  it('补丁在模板里只出现一次，且由 hooks 侧的宏实现（hooks 缺失时可退回上游行为）', () => {
    expect(countOccurrences(template, '!ifdef AFM_HOOK_PRELEGACY_UNINSTALL')).toBe(1);
    expect(countOccurrences(template, '!insertmacro NSIS_HOOK_PRELEGACY_UNINSTALL')).toBe(1);

    // 宏与 define 都放在 hooks.nsh 里：模板因此不依赖 hooks 也能编译。
    expect(hooks).toContain('!define AFM_HOOK_PRELEGACY_UNINSTALL');
    expect(hooks).toContain('!macro NSIS_HOOK_PRELEGACY_UNINSTALL');

    const macroStart = hooks.indexOf('!macro NSIS_HOOK_PRELEGACY_UNINSTALL');
    const macroEnd = hooks.indexOf('!macroend', macroStart);
    const macro = hooks.slice(macroStart, macroEnd === -1 ? macroStart + 2000 : macroEnd);

    expect(macro).toContain('Call AlphaPrepareLegacyUninstall');
  });

  it('旧卸载器前的钩子会带 -InstDir / -DisableScheduledTask 调用杀进程脚本', () => {
    const fnStart = hooks.indexOf('Function AlphaPrepareLegacyUninstall');
    expect(fnStart).toBeGreaterThan(-1);

    const fn = hooks.slice(fnStart, fnStart + 2500);

    expect(fn).toContain('-File "$PLUGINSDIR\\kill-app-tree.ps1"');
    expect(fn).toContain('-InstDir "$AlphaLegacyInstDir"');
    expect(fn).toContain('-DisableScheduledTask');

    // 安装侧（PREINSTALL）与卸载侧（PREUNINSTALL）共用同一套杀进程逻辑，
    // 且都必须把目标安装目录告诉脚本，否则独占校验无从下手。
    expect(hooks).toContain('-InstDir "$AlphaKillInstDir"');
    expect(hooks).toContain('Var AlphaKillInstDir');
    expect(hooks).toContain('Var AlphaLegacyInstDir');
  });

  it('tauri.conf.json 指向 vendor 模板与 hooks', () => {
    const conf = JSON.parse(fs.readFileSync(path.join(repoRoot, 'src-tauri', 'tauri.conf.json'), 'utf8'));

    expect(conf.bundle.windows.nsis.template).toBe('installer/installer.nsi');
    expect(conf.bundle.windows.nsis.installerHooks).toBe('installer/hooks.nsh');
  });

  it('@tauri-apps/cli 精确 pin 到 2.12.0（vendor 模板必须与 CLI 内嵌模板同版本）', () => {
    const pkg = JSON.parse(fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'));
    const cli = pkg.devDependencies['@tauri-apps/cli'];

    // 必须是精确版本，不能是 ^ / ~ 范围：CLI 一升级，vendor 模板就与上游漂移，
    // 而 CLI 的 npm 包不附带 .nsi 模板文件，没有自动比对手段。
    expect(cli).toBe('2.12.0');
    expect(cli).not.toMatch(/^[\^~]/);
  });

  it('package.json 与 Cargo.toml 的版本号一致', () => {
    const pkg = JSON.parse(fs.readFileSync(path.join(repoRoot, 'package.json'), 'utf8'));
    const cargo = fs.readFileSync(path.join(repoRoot, 'src-tauri', 'Cargo.toml'), 'utf8');

    expect(readCargoPackageVersion(cargo)).toBe(pkg.version);
  });
});

describe('kill-app-tree.ps1 的「杀干净」能力', () => {
  const installerDir = findInstallerDir();
  const ps1 = fs.readFileSync(path.join(installerDir, 'kill-app-tree.ps1'), 'utf8');

  it('仍是纯 ASCII（PowerShell 5.1 按 ANSI 代码页读无 BOM 的 .ps1）', () => {
    expect(ps1).not.toMatch(/[^\x00-\x7F]/);
  });

  it('提供等待退出与独占打开校验所需的参数与实现', () => {
    expect(ps1).toContain('[string]$InstDir');
    expect(ps1).toContain('[switch]$DisableScheduledTask');
    expect(ps1).toContain('[int]$WaitTimeoutMs');
    expect(ps1).toContain('[int]$VerifyTimeoutMs');

    expect(ps1).toContain('function Test-ExclusiveOpen');
    expect(ps1).toContain('function Wait-PidsGone');

    // 仍然被占用时必须用独立的退出码报出来（3），不能和「有进程幸存」(1) 混在一起。
    expect(ps1).toMatch(/exit 3/);
    expect(ps1).toContain('STILL LOCKED');
  });
});
