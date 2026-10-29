// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION: 安装器「早期守卫」（2026-10-29）。
//
// 事故（用户原话）：「每次切回 MiniMax code 我们这个会话界面，都会弹出 AFM 的安装界面。」
//
// 根因链（已实锤）：
//   1. 会话消息里带着安装包附件（`<media type="file" src="...setup.exe" />`）；
//   2. 宿主的预览组件在**每次渲染**时对这个附件 autoOpen；
//   3. .exe 没有内置预览器 → 走 `shell.openPath` → **直接执行 setup.exe**；
//   4. NSIS 向导弹出。「切回会话界面」= 重新渲染 = 重新执行。
//
// 修复落点：安装包改不了宿主，但可以让「重复运行自己」变得无害 ——
// 在 `.onInit`（唯一能在向导页出现之前拿到控制权的位置）判断版本：
// 同版本或更旧 ⇒ 静默退出，一个窗口都不弹。
//
// 这组断言守四件事：
//   1. **接线**：守卫必须真的在 `.onInit` 里、且在向导出现之前调用 ——
//      位置错 = 守卫失效，而且不会报错；
//   2. **静默**：守卫路径里不许出现 MessageBox（弹窗场景就是「用户没要求装」，
//      任何模态框都只是把一种弹窗换成另一种弹窗）；
//   3. **判定**：版本比较的判定表必须符合预期（升级放行 / 同版本静默 /
//      更旧静默 / 判不出来放行）；
//   4. **判别力**：把判定改坏之后，这套用例必须转红 —— 一套永远绿的
//      验收脚本比没有验收脚本更糟，它会让人以为这条路有覆盖。
//

import { describe, expect, it } from 'vitest';
import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

// 本测试文件就在 src-tauri/installer/__tests__/ 里，安装器目录是它的上一级。
// 直接以文件自身位置定位，不依赖 cwd —— 门禁有时是从仓库根、有时是从
// 工作区根拉起 vitest，依赖 cwd 的写法会在其中一种下找不到目录。
const INSTALLER_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

const HOOKS_PATH = path.join(INSTALLER_DIR, 'hooks.nsh');
const TEMPLATE_PATH = path.join(INSTALLER_DIR, 'installer.nsi');
const GUARD_PATH = path.join(INSTALLER_DIR, 'early-guard.ps1');

const isWindows = process.platform === 'win32';

// 每条判定用例都要拉起一次 PowerShell。守卫脚本只读注册表，不枚举进程，
// 单次通常 1~2 秒；八条一起跑给足预算。
const GUARD_TIMEOUT_MS = 180_000;

const VERDICT_FILE_NAME = 'alpha-early-guard.verdict';

function readFile(file: string): string {
  return fs.readFileSync(file, 'utf8');
}

describe('安装器早期守卫：接线', () => {
  it('守卫脚本存在且保持纯 ASCII（PowerShell 5.1 按 ANSI 代码页读无 BOM 的 .ps1）', () => {
    const source = readFile(GUARD_PATH);

    expect(
      source,
      'early-guard.ps1 出现了非 ASCII 字符。它被原样解包到 $PLUGINSDIR 后，'
      + 'PowerShell 5.1 读无 BOM 的 .ps1 会按系统 ANSI 代码页解码，中文注释会变成乱码。',
    ).toMatch(/^[\x00-\x7F]*$/);
  });

  it('模板在 .onInit 里调用守卫，且排在 SetContext 与向导之前', () => {
    const template = readFile(TEMPLATE_PATH);

    const onInitAt = template.indexOf('Function .onInit');
    const insertAt = template.indexOf('!insertmacro NSIS_HOOK_EARLYINIT');
    const setContextAt = template.indexOf('!insertmacro SetContext');
    const installSectionAt = template.indexOf('Section Install');

    expect(onInitAt, '模板里找不到 Function .onInit').toBeGreaterThan(-1);
    expect(insertAt, '模板里找不到 !insertmacro NSIS_HOOK_EARLYINIT').toBeGreaterThan(-1);

    // 守卫必须落在 .onInit 里 —— 那是唯一能在向导页出现之前拿到控制权的位置。
    expect(
      insertAt,
      '守卫必须写在 .onInit 里。PREINSTALL 太晚了：向导已经弹出来了。',
    ).toBeGreaterThan(onInitAt);

    // 排在 SetContext 之前：守卫要同时看 HKLM 和 HKCU，不依赖 SHCTX。
    expect(
      insertAt,
      '守卫必须排在 SetContext 之前 —— 守卫自己读两个 hive，不该依赖 SHCTX。',
    ).toBeLessThan(setContextAt);

    // 必须落在 .onInit 的函数体内部。
    const onInitEnd = template.indexOf('FunctionEnd', onInitAt);

    expect(onInitEnd).toBeGreaterThan(onInitAt);
    expect(insertAt).toBeLessThan(onInitEnd);

    // .onInit 之后才是真正干活的 Section —— 守卫必然早于它。
    expect(onInitAt).toBeLessThan(installSectionAt);

    // 模板必须把版本号与注册表子键喂给守卫（hooks.nsh 在 !include 位置
    // 早于 !define VERSION，拿不到这两个编译期常量）。
    expect(template).toContain('StrCpy $AlphaGuardInstallerVersion "${VERSION}"');
    expect(template).toContain('StrCpy $AlphaGuardUninstKey "${UNINSTKEY}"');

    // hooks 缺失时模板仍要能编译。
    expect(template).toContain('!ifdef AFM_HOOK_EARLYINIT');
  });

  it('hooks.nsh 提供钩子定义、宏与守卫函数', () => {
    const hooks = readFile(HOOKS_PATH);

    expect(hooks).toContain('!define AFM_HOOK_EARLYINIT');
    expect(hooks).toContain('!macro NSIS_HOOK_EARLYINIT');
    expect(hooks).toContain('Function AlphaEarlyInstallGuard');

    // 变量声明必须在 hooks.nsh 里：模板要用 StrCpy 给它们赋值。
    expect(hooks).toContain('Var AlphaGuardInstallerVersion');
    expect(hooks).toContain('Var AlphaGuardUninstKey');

    // 命令行捕获函数也必须在 hooks.nsh 里（模板与卸载器共用这一份文件）。
    expect(hooks).toContain('Function AlphaCaptureCommandLine');
  });

  it('守卫用 File 指令内嵌脚本，路径指向真实源文件', () => {
    const hooks = readFile(HOOKS_PATH);

    expect(
      hooks,
      '内嵌块必须用 File 指令从 ${__FILEDIR__} 取源文件并解包到 $PLUGINSDIR，'
      + '这样安装时跑的就是仓库里那一份，不存在两份文本漂移。',
    ).toMatch(
      /File\s+\/oname=\$PLUGINSDIR\\early-guard\.ps1\s+"\$\{__FILEDIR__\}\\early-guard\.ps1"/,
    );

    expect(fs.existsSync(GUARD_PATH)).toBe(true);
  });

  it('判定文件路径：NSIS 传给脚本的与 NSIS 自己检查的必须是同一个', () => {
    const hooks = readFile(HOOKS_PATH);

    expect(
      hooks,
      'nsExec 必须把判定文件路径传给守卫脚本的 -VerdictFile 参数。',
    ).toContain(`-VerdictFile "$PLUGINSDIR\\${VERDICT_FILE_NAME}"`);

    expect(
      hooks,
      'IfFileExists 检查的必须是同一个判定文件路径。'
      + '两处写成不同的字面量时，守卫判了「不要弹界面」、NSIS 却看不到标记文件，'
      + '向导照样弹出来，而且没有任何报错。',
    ).toContain(`IfFileExists "$PLUGINSDIR\\${VERDICT_FILE_NAME}"`);
  });

  it('守卫必须静默退出：路径里不许出现 MessageBox', () => {
    const hooks = readFile(HOOKS_PATH);

    const start = hooks.indexOf('Function AlphaEarlyInstallGuard');
    const end = hooks.indexOf('FunctionEnd', start);

    expect(start).toBeGreaterThan(-1);
    expect(end).toBeGreaterThan(start);

    const guard = hooks.slice(start, end);

    expect(
      guard,
      '守卫里出现 MessageBox 就等于把一种弹窗换成另一种弹窗 —— '
      + '触发场景是「用户没要求装」，任何模态框都会继续打扰用户。',
    ).not.toContain('MessageBox');

    // 静默退出的实现是 Quit（直接结束安装器，不跑任何 Section）。
    expect(guard).toContain('Quit');
  });

  it('/FORCE 是人工重装 / 降级的唯一入口，必须真的能被发现', () => {
    const hooks = readFile(HOOKS_PATH);
    const guard = readFile(GUARD_PATH);

    // 识别逻辑放在守卫脚本里（PowerShell 有正则，也能被单元测试覆盖）。
    // NSIS 侧的 StrCmp 通配匹配在这台机器上实测不成立（8 种形态一次都没
    // 匹配上），所以那个旧函数不许再回来 —— 它一旦回来，/FORCE 就静默失效。
    expect(hooks).toContain('Function AlphaCaptureCommandLine');
    expect(hooks).not.toContain('Function AlphaDetectForceFlag');

    // NSIS 这一侧只负责把原始命令行原样交出去。
    expect(hooks).toContain('FileWrite $9 "$CMDLINE"');
    expect(hooks).toContain('-CommandLineFile "$PLUGINSDIR\\afm-commandline.txt"');

    // 守卫脚本要认这个命令行文件，否则 /FORCE 传下去也没用。
    expect(guard).toMatch(/\[string\]\$CommandLineFile\s*=\s*''/);
    expect(guard).toContain('/FORCE');
  });

  it('守卫失败时必须放行（守卫坏了不能让产品装不上）', () => {
    const guard = readFile(GUARD_PATH);

    // 「判定文件不存在 = 放行」这条不变量：脚本只在判定为「不要弹界面」
    // 时才写判定文件。
    const verdictWrites = guard.match(/Set-Content\s+-LiteralPath\s+\$VerdictFile/g) ?? [];

    expect(
      verdictWrites.length,
      '判定文件只能在「判定为不要弹界面」的分支里写。写多了就没有「放行」这条路了。',
    ).toBe(1);

    expect(guard).toContain("if ($decision -eq 'quit')");
  });
});

describe.skipIf(!isWindows)('安装器早期守卫：判定与判别力', () => {
  const registryKeyName = `AFM-Early-Guard-Test-${process.pid}`;
  const registrySubKey = `Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\${registryKeyName}`;

  function writeInstalledVersion(version: string | null): void {
    // 用 reg.exe 而不是 PowerShell：参数少一层引号，行为确定。
    // 写在 HKCU 下，不需要管理员权限。
    const keyPath = `HKCU\\${registrySubKey}`;

    if (version === null) {
      spawnSync('reg', ['delete', keyPath, '/f'], { encoding: 'utf8' });

      return;
    }

    spawnSync('reg', ['add', keyPath, '/v', 'DisplayVersion', '/t', 'REG_SZ', '/d', version, '/f'], {
      encoding: 'utf8',
    });
  }

  function runGuard(options: {
    installerVersion: string;
    installedVersion: string | null;
    force?: boolean;
    scriptPath?: string;
    commandLine?: string | null;
  }): { status: number | null; verdict: string | null; output: string } {
    const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'afm-early-guard-'));
    const verdictPath = path.join(scratch, VERDICT_FILE_NAME);
    const logPath = path.join(scratch, 'guard.log');
    const scriptPath = options.scriptPath ?? GUARD_PATH;
    const commandLinePath = path.join(scratch, 'commandline.txt');

    writeInstalledVersion(options.installedVersion);

    // commandLine 为 null 时**不写这个文件** —— 用来验「文件不存在」这条分支。
    if (options.commandLine !== null && options.commandLine !== undefined) {
      fs.writeFileSync(commandLinePath, options.commandLine, 'utf8');
    }

    const args = [
      '-NoProfile',
      '-ExecutionPolicy',
      'Bypass',
      '-File',
      scriptPath,
      '-InstallerVersion',
      options.installerVersion,
      '-UninstKey',
      registrySubKey,
      '-Force',
      options.force ? '1' : '0',
      '-CommandLineFile',
      commandLinePath,
      '-VerdictFile',
      verdictPath,
      '-LogFileOverride',
      logPath,
    ];

    const result = spawnSync('powershell', args, { encoding: 'utf8', timeout: 120000 });

    const verdict = fs.existsSync(verdictPath)
      ? fs.readFileSync(verdictPath, 'utf8').trim()
      : null;

    return {
      status: result.status,
      verdict,
      output: `${result.stdout ?? ''}${result.stderr ?? ''}`,
    };
  }

  it('判定表全部符合预期', () => {
    const cases: {
      name: string;
      installer: string;
      installed: string | null;
      force?: boolean;
      expectQuit: boolean;
      expectReason: string | null;
    }[] = [
      { name: '没装过 -> 放行', installer: '3.6.6', installed: null, expectQuit: false, expectReason: null },
      { name: '版本更新 -> 放行', installer: '3.6.7', installed: '3.6.6', expectQuit: false, expectReason: null },
      { name: '同版本 -> 静默退出', installer: '3.6.6', installed: '3.6.6', expectQuit: true, expectReason: 'same-version' },
      { name: '版本更旧 -> 静默退出', installer: '3.6.5', installed: '3.6.6', expectQuit: true, expectReason: 'older-version' },
      { name: '版本读不出来 -> 放行', installer: '3.6.6', installed: 'not-a-version', expectQuit: false, expectReason: null },
      { name: '/FORCE -> 放行', installer: '3.6.6', installed: '3.6.6', force: true, expectQuit: false, expectReason: null },
      {
        name: '已装预发布版 -> 正式版放行',
        installer: '3.6.6',
        installed: '3.6.6-beta.1',
        expectQuit: false,
        expectReason: null,
      },
      {
        name: '预发布版比正式版旧 -> 静默退出',
        installer: '3.6.6-beta.1',
        installed: '3.6.6',
        expectQuit: true,
        expectReason: 'older-version',
      },
    ];

    for (const item of cases) {
      const result = runGuard({
        installerVersion: item.installer,
        installedVersion: item.installed,
        force: item.force,
      });

      expect(
        result.status,
        `用例「${item.name}」的退出码不对（期望 ${item.expectQuit ? 10 : 0}）：\n${result.output}`,
      ).toBe(item.expectQuit ? 10 : 0);

      if (item.expectQuit) {
        expect(
          result.verdict,
          `用例「${item.name}」应该留下判定文件，说明「不要弹界面」：\n${result.output}`,
        ).toBe(item.expectReason);
      } else {
        expect(
          result.verdict,
          `用例「${item.name}」不该留下判定文件 —— 留下就意味着守卫误杀了这次安装：\n${result.output}`,
        ).toBeNull();
      }
    }

    writeInstalledVersion(null);
  }, GUARD_TIMEOUT_MS);

  it('命令行里的 /FORCE 才算逃生口：整词匹配，/XFORCE 不算', () => {
    const cases: {
      name: string;
      commandLine: string | null;
      expectQuit: boolean;
    }[] = [
      { name: 'setup.exe /FORCE -> 放行', commandLine: '"C:\\setup.exe" /FORCE', expectQuit: false },
      { name: 'setup.exe /force -> 放行', commandLine: '"C:\\setup.exe" /force', expectQuit: false },
      { name: 'setup.exe /FORCE /S -> 放行', commandLine: '"C:\\setup.exe" /FORCE /S', expectQuit: false },
      { name: 'setup.exe /S /FORCE -> 放行', commandLine: '"C:\\setup.exe" /S /FORCE', expectQuit: false },
      { name: 'setup.exe /XFORCE -> 仍然静默退出', commandLine: '"C:\\setup.exe" /XFORCE', expectQuit: true },
      { name: '命令行里没有 /FORCE -> 仍然静默退出', commandLine: '"C:\\setup.exe"', expectQuit: true },
      { name: '没有命令行文件 -> 仍然静默退出', commandLine: null, expectQuit: true },
    ];

    for (const item of cases) {
      const result = runGuard({
        installerVersion: '3.6.6',
        installedVersion: '3.6.6',
        commandLine: item.commandLine,
      });

      expect(
        result.status,
        `用例「${item.name}」的退出码不对（期望 ${item.expectQuit ? 10 : 0}）：\n${result.output}`,
      ).toBe(item.expectQuit ? 10 : 0);

      if (item.expectQuit) {
        expect(
          result.verdict,
          `用例「${item.name}」应该留下判定文件，说明「不要弹界面」：\n${result.output}`,
        ).toBe('same-version');
      } else {
        expect(
          result.verdict,
          `用例「${item.name}」不该留下判定文件 —— 留下就意味着守卫误杀了这次安装：\n${result.output}`,
        ).toBeNull();
      }
    }

    writeInstalledVersion(null);
  }, GUARD_TIMEOUT_MS);

  it('判别力：把判定改坏之后，用例必须转红', () => {
    const scratch = fs.mkdtempSync(path.join(os.tmpdir(), 'afm-early-guard-negative-'));
    const brokenPath = path.join(scratch, 'early-guard.ps1');

    // 故意让「同版本」这条判据不再静默退出 —— 于是同版本会继续弹向导。
    // 「同版本 -> 静默退出」那条用例必须失败。
    //
    // 注意不能只改 elseif 的条件：改成永远不成立时，同版本会掉进
    // 「更旧」那个 else 分支，结果照样是退出（守卫根本没被关掉）。
    // 所以必须改「同版本这一支的决定本身」。
    const source = readFile(GUARD_PATH).replace(/\r\n/g, '\n');

    const broken = source.replace(
      "    $decision = 'quit'\n    $reason = 'same-version'",
      "    $decision = 'proceed'\n    $reason = 'same-version'",
    );

    expect(
      broken,
      '注入点没找到 —— 守卫脚本的同版本判定被改过了，判别力测试必须跟着更新。',
    ).not.toBe(source);

    fs.writeFileSync(brokenPath, broken, 'utf8');

    const result = runGuard({
      installerVersion: '3.6.6',
      installedVersion: '3.6.6',
      scriptPath: brokenPath,
    });

    expect(
      result.status,
      `把守卫判定改坏之后，同版本居然还是退出了（exit=${result.status}）：\n${result.output}\n`
      + '这说明判定用例没有判别力 —— 它证明不了守卫真的会拦住重复弹窗。',
    ).toBe(0);

    expect(result.verdict).toBeNull();

    writeInstalledVersion(null);
  }, GUARD_TIMEOUT_MS);
});
