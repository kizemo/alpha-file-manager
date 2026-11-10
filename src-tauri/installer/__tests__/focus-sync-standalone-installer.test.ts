// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

//
// FORK-MODIFICATION: 守卫 Focus Sync 扩展独立安装器与新的侧车生命周期对齐。
//
// 背景（2026-11-10）：
//   侧车（focus-sync-sidecar.exe）已经改成 **Alpha File Manager 主进程的子进程** ——
//   主进程启动时 spawn、退出时 kill，命令行只有 `--port 37421`，不带 `--service`。
//   主安装器（src-tauri/installer/hooks.nsh）在 3.13.5 里已经彻底删掉了 legacy 的
//   `KizemoFocusSync` 计划任务注册，只保留幂等清理。
//
//   但扩展的**独立安装器**（extensions/kizemo.focus-sync/installer/installer.nsi）
//   当时还在 STAGE 6.5 注册那个计划任务（AtLogOn + `--service`）。一旦有人用独立
//   安装包装扩展，legacy 任务就会被重新注册 —— 侧车的生命周期又被拖回"和应用无关"
//   的旧模式，还会和主进程拉起的侧车抢 37421 端口。
//
// 这条门禁守的是**静态源码事实**，不依赖真机：
//   1. 独立安装器不再打包/调用 register-scheduled-task.ps1；
//   2. 独立安装器里不再出现 `--service` 与 `schtasks /Create`；
//   3. 独立安装器改为调用幂等清理脚本 remove-legacy-focus-task.ps1；
//   4. 结束侧车进程的脚本在部署二进制**之前**执行（否则 exe 被占用，覆盖会失败）；
//   5. 版本号三处（installer.nsi / build.ps1 / package.json）必须一致，
//      register.ps1 不许再硬编码版本号；
//   6. 进程名必须同时覆盖改名后的 alpha-file-manager 与旧名 sigma-file-manager。
//

import { describe, expect, it } from 'vitest';
import fs from 'node:fs';
import path from 'node:path';

// vitest 的 import.meta.url 不是 file:// URL（ERR_INVALID_URL_SCHEME），
// 所以从 cwd 往上找包含扩展安装器目录的仓库根，而不是用 import.meta 推导。
function findRepoRoot(): string {
  let dir = process.cwd();

  for (let i = 0; i < 10; i += 1) {
    const candidate = path.join(dir, 'extensions', 'kizemo.focus-sync', 'installer', 'installer.nsi');

    if (fs.existsSync(candidate)) {
      return dir;
    }

    const parent = path.dirname(dir);

    if (parent === dir) break;
    dir = parent;
  }

  throw new Error('从 cwd 向上找不到 extensions/kizemo.focus-sync/installer');
}

const REPO_ROOT = findRepoRoot();
const EXT_INSTALLER_DIR = path.join(REPO_ROOT, 'extensions', 'kizemo.focus-sync', 'installer');

function read(relativePath: string): string {
  return fs.readFileSync(path.join(EXT_INSTALLER_DIR, relativePath), 'utf8');
}

const NSI = read('installer.nsi');
const BUILD_PS1 = read('build.ps1');
const REGISTER_PS1 = read('register.ps1');
const KILL_PS1 = read('kill-sigma.ps1');
const REOPEN_PS1 = read('reopen-sigma.ps1');
const STOP_SIDECAR_PS1 = read('stop-sidecar-processes.ps1');
const PACKAGE_JSON = JSON.parse(
  fs.readFileSync(path.join(REPO_ROOT, 'extensions', 'kizemo.focus-sync', 'package.json'), 'utf8'),
) as { version?: string };

/** 取 NSIS 里某个 !define 的字面量值。 */
function nsisDefine(name: string): string {
  const match = NSI.match(new RegExp(`^!define\\s+${name}\\s+"([^"]*)"`, 'm'));

  if (!match) throw new Error(`installer.nsi 里找不到 !define ${name}`);

  return match[1];
}

/** 找出某个片段在文本里第一次出现的下标；找不到返回 -1。 */
function indexOf(haystack: string, needle: string): number {
  return haystack.indexOf(needle);
}

/**
 * 去掉 NSIS 注释行（以 ';' 开头）。
 * 头部注释里会**提到** legacy 行为（例如 "--service"）作为说明，
 * 那不是执行代码；真正要禁止的是"还在执行它"。
 */
const NSI_BODY = NSI.split(/\r?\n/)
  .filter((line) => !line.trimStart().startsWith(';'))
  .join('\n');

describe('Focus Sync 独立安装器：不再注册 legacy 计划任务', () => {
  it('installer.nsi 不再引用 register-scheduled-task.ps1', () => {
    expect(NSI).not.toContain('register-scheduled-task.ps1');
  });

  it('installer.nsi 不再以 --service 模式拉起侧车', () => {
    expect(NSI_BODY).not.toContain('--service');
  });

  it('installer.nsi 不再创建计划任务', () => {
    expect(NSI_BODY).not.toMatch(/schtasks\s+\/Create/i);
    expect(NSI_BODY).not.toMatch(/Register-ScheduledTask/i);
  });

  it('installer.nsi 改为调用幂等清理脚本 remove-legacy-focus-task.ps1', () => {
    expect(NSI).toContain('remove-legacy-focus-task.ps1');
    // 必须用 -File 形态：内联 powershell -Command 在安装器上下文不可靠
    // （3.13.5 真机实测：同一行命令在普通提权 shell 里能跑通，安装器里静默失效）。
    expect(NSI).toMatch(/powershell[^\r\n]*-File\s+"\$INSTDIR\\bin\\remove-legacy-focus-task\.ps1"/);
  });

  it('installer.nsi 会结束仍在运行的侧车进程', () => {
    expect(NSI).toContain('stop-sidecar-processes.ps1');
    expect(NSI).toMatch(/powershell[^\r\n]*-File\s+"\$INSTDIR\\bin\\stop-sidecar-processes\.ps1"/);
  });

  it('结束侧车进程发生在部署侧车二进制之前', () => {
    // 必须定位「执行」那一行，不能匹配 `File "stop-sidecar-processes.ps1"`。
    // 后者是 Stage 0 把脚本拷进 $INSTDIR\bin 的拷贝行，永远排在部署之前，
    // 用它做判据会让「先杀掉再覆盖」这条顺序断言恒为绿（3.13.5 复核发现）。
    const stopInvoke = NSI.match(
      /powershell[^\r\n]*-File\s+"\$INSTDIR\\bin\\stop-sidecar-processes\.ps1"/
    );
    const deployAt = indexOf(NSI, 'File "..\\bin\\focus-sync-sidecar.exe"');

    expect(stopInvoke, 'installer.nsi 里找不到 stop-sidecar-processes.ps1 的 -File 执行行').not.toBeNull();
    expect(deployAt).toBeGreaterThan(-1);
    // 运行中的映像会锁住自己的 exe，不先结束它，覆盖二进制会失败。
    expect(stopInvoke!.index).toBeLessThan(deployAt);
  });

  it('installer.nsi 的进程检测覆盖改名后的 alpha-file-manager', () => {
    expect(NSI).toContain('alpha-file-manager');
    expect(NSI).toContain('sigma-file-manager');
  });
});

describe('Focus Sync 独立安装器：版本号只有一个来源', () => {
  it('installer.nsi 的版本与 package.json 一致', () => {
    expect(PACKAGE_JSON.version).toBeTruthy();
    expect(nsisDefine('PRODUCT_VERSION')).toBe(PACKAGE_JSON.version);
    expect(nsisDefine('PRODUCT_SIDECAR_VER')).toBe(PACKAGE_JSON.version);
  });

  it('installer.nsi 的输出文件名带同一个版本号', () => {
    expect(NSI).toContain(`kizemo.focus-sync-\${PRODUCT_VERSION}-setup.exe`);
  });

  it('build.ps1 的产物名带同一个版本号', () => {
    expect(BUILD_PS1).toContain(`kizemo.focus-sync-${PACKAGE_JSON.version}-setup.exe`);
    // 不能再出现旧版本号（历史上 header 写 0.2.0、脚本发 0.5.8 的那类漂移）
    expect(BUILD_PS1).not.toMatch(/kizemo\.focus-sync-0\.5\.[0-8]-setup\.exe/);
  });

  it('register.ps1 不再硬编码版本号，改为读已部署的 package.json', () => {
    // 大小写不敏感：register.ps1 里的变量叫 $extensionVersion，
    // 旧正则 /version\s*=\s*'0\.\d+\.\d+'/ 匹配不到 `Version = '0.5.9'`
    // ⇒ 硬编码版本号在门禁里完全隐形（3.13.5 复核实测 match=null）。
    expect(REGISTER_PS1).not.toMatch(/version\s*=\s*'0\.\d+\.\d+'/i);
    expect(REGISTER_PS1).toContain('package.json');
    expect(REGISTER_PS1).toContain('$extensionVersion');
  });

  it('register.ps1 的扩展描述与新生命周期一致', () => {
    // 只看 manifest 里的 description 字段本身：注释里会为了说明历史而**提到**
    // 旧文案（"Sidecar runs as a Windows Scheduled Task"），那是解释，不是执行。
    const descMatch = REGISTER_PS1.match(/description\s*=\s*"([^"]*)"/);

    expect(descMatch, 'register.ps1 里找不到 manifest 的 description 字段').not.toBeNull();
    expect(descMatch?.[1]).not.toContain('Scheduled Task');
    expect(descMatch?.[1]).toContain('child process of Alpha File Manager');
  });
});

describe('Focus Sync 独立安装器：进程名与品牌对齐', () => {
  it('kill-sigma.ps1 同时匹配新旧进程名', () => {
    expect(KILL_PS1).toContain('alpha-file-manager');
    expect(KILL_PS1).toContain('sigma-file-manager');
  });

  it('reopen-sigma.ps1 能找到改名后的安装位置', () => {
    expect(REOPEN_PS1).toContain('Alpha File Manager');
    expect(REOPEN_PS1).toContain('alpha-file-manager.exe');
  });

  it('installer.nsi 的用户可见文案使用 Alpha File Manager 品牌', () => {
    expect(NSI).toContain('Alpha File Manager');
    expect(NSI).not.toMatch(/Sigma File Manager/);
  });
});

describe('Focus Sync 独立安装器：helper 脚本保持纯 ASCII', () => {
  const helpers = [
    'register.ps1',
    'unregister.ps1',
    'kill-sigma.ps1',
    'reopen-sigma.ps1',
    'remove-legacy-focus-task.ps1',
    'stop-sidecar-processes.ps1',
    'register-scheduled-task.ps1',
    'unregister-scheduled-task.ps1',
  ];

  it.each(helpers)('%s 不含非 ASCII 字符', (name) => {
    const content = read(name);
    const nonAscii = content.match(/[^\x00-\x7F]/g);

    // PowerShell 5.1 按系统 ANSI 代码页读取无 BOM 的 .ps1，
    // 脚本里出现中文会被解析成乱码。需要中文时走命令行参数。
    expect(nonAscii, `${name} 含有非 ASCII 字符`).toBeNull();
  });

  it('stop-sidecar-processes.ps1 覆盖规范的嵌套部署路径', () => {
    // 规范位置：<appdata>\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe
    expect(STOP_SIDECAR_PS1).toContain('focus-sync-sidecar');
    expect(STOP_SIDECAR_PS1).toMatch(/bin\\focus-sync-sidecar/i);
  });
});
