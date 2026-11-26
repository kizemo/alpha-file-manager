// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘的 **Tauri 命令契约**门禁。
//
// `remote-command-contract.test.ts` 已经管住了「Rust 参数名 ↔ JS 传出的键」，
// 但它管不到另外两件事，而这两件事在真机上都是**静默失败**：
//
//   1. **命令没注册到 `generate_handler!`** —— 函数写好了，`#[tauri::command]`
//      也加了，唯独漏了注册那一行。前端调用 `remote_aliyun_login_start`
//      在真机上会得到「command not found」，而单测里 `invoke` 是 mock 的，全绿。
//   2. **后端有命令、前端没有调用点** —— 前端少调一个命令，功能就是不存在；
//      反过来前端调了一个后端没有的命令，真机上同样是「command not found」。
//
// 所以这里**直接读源码**：不跑 Rust、不起应用，源码一变测试立刻红。
//
// 内嵌 webview 登录（批次 29）之后，命令一共十一条，并分成两类调用方：
// 前端 `invoke` 的十条，和**只被注入脚本调用**的 `remote_aliyun_login_observed`。
// 后者必须单列，否则它要么被误报成没人用的僵尸命令，要么从清单里消失，
// 变成一条没有任何契约保护、能随意改签名的后端接口。

import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

// 本文件在 `src/modules/remote/utils/__tests__/` 下，往上五层才是仓库根。
const REPO_ROOT = join(__dirname, '..', '..', '..', '..', '..');
const SRC_ROOT = join(REPO_ROOT, 'src');
const RUST_SRC = join(REPO_ROOT, 'src-tauri', 'src');
const LIB_RS = join(RUST_SRC, 'lib.rs');
/** 注入脚本 + 建窗代码的真源，钩子专用命令的调用方在这里。 */
const ALIYUN_LOGIN_RS = join(RUST_SRC, 'aliyun_login.rs');
/** 命令清单的**文档真源**：phase-1 契约 + 内嵌 webview 登录的实现记录。 */
const CONTRACT_DOCS = [
  join(REPO_ROOT, '.superpowers', 'sdd', 'b26-aliyun-contract.md'),
  join(REPO_ROOT, '.superpowers', 'sdd', 'b29-webview-impl.md'),
];

/**
 * 阿里云盘的全部命令契约。
 *
 * 十一条，少一条功能不存在，多一条就是「定义了却没人调」的僵尸命令。
 */
const EXPECTED_COMMANDS = [
  'remote_aliyun_qr_start',
  'remote_aliyun_qr_poll',
  'remote_aliyun_save_session',
  'remote_aliyun_login_start',
  'remote_aliyun_login_observed',
  'remote_aliyun_login_close',
  'remote_aliyun_login_pending',
  'remote_aliyun_login_claim',
  'remote_aliyun_login_cancel',
  'remote_aliyun_auth_status',
  'remote_aliyun_delete_session',
];

/** 只由注入钩子调用、前端不直接 invoke 的命令。 */
const HOOK_ONLY_COMMANDS = ['remote_aliyun_login_observed'];

/** 前端应当**自己**invoke 的那部分（= 全清单扣掉钩子专用）。 */
const FRONTEND_INVOKED_COMMANDS = EXPECTED_COMMANDS.filter(
  name => !HOOK_ONLY_COMMANDS.includes(name),
);

/** 从 Rust 源码里抽出所有 `#[tauri::command]` 的命令名。 */
function parseTauriCommands(source: string): string[] {
  const pattern = /#\[tauri::command\][\s\S]{0,120}?pub (?:async )?fn (\w+)/g;
  const names: string[] = [];
  let match = pattern.exec(source);

  while (match !== null) {
    names.push(match[1]);
    match = pattern.exec(source);
  }

  return names;
}

/** 递归列出目录下的全部文件。 */
function listFiles(dir: string): string[] {
  const out: string[] = [];

  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const full = join(dir, entry.name);

    if (entry.isDirectory()) {
      out.push(...listFiles(full));
    }
    else {
      out.push(full);
    }
  }

  return out;
}

const rustFiles = existsSync(RUST_SRC)
  ? listFiles(RUST_SRC).filter(path => path.endsWith('.rs'))
  : [];

/** Rust 侧定义的全部 `remote_aliyun_*` 命令（十一条都在 `aliyun_login.rs`）。 */
const rustAliyunCommands = rustFiles
  .flatMap(path => parseTauriCommands(readFileSync(path, 'utf8')))
  .filter(name => name.startsWith('remote_aliyun_'));

/** 前端**实际 invoke** 的全部 `remote_aliyun_*` 命令。 */
const frontendCalledCommands = listFiles(SRC_ROOT)
  .filter(path => /\.(ts|vue)$/.test(path))
  .flatMap((path) => {
    const source = readFileSync(path, 'utf8');
    const pattern = /invoke(?:<[^>]*>)?\(\s*'(remote_aliyun_\w+)'/g;
    const names: string[] = [];
    let match = pattern.exec(source);

    while (match !== null) {
      names.push(match[1]);
      match = pattern.exec(source);
    }

    return names;
  });

/** 文档真源不在版本库里，干净检出时缺席就跳过，不假绿；在场就必须列全。 */
const contractDocsText = CONTRACT_DOCS
  .filter(path => existsSync(path))
  .map(path => readFileSync(path, 'utf8'))
  .join('\n');
const contractMissing = contractDocsText === ''
  ? []
  : EXPECTED_COMMANDS.filter(name => !contractDocsText.includes(name));

describe('阿里云盘的 Tauri 命令契约', () => {
  it('前端自己调用的阿里云盘命令恰好是这 10 条，不多不少', () => {
    const called = [...new Set(frontendCalledCommands)].sort();
    const expected = [...FRONTEND_INVOKED_COMMANDS].sort();

    expect(called, `前端实际调用的阿里云盘命令：${called.join(', ')}`).toEqual(expected);
  });

  it('钩子专用命令前端不得 invoke，且必须真的存在于注入脚本里', () => {
    const leaked = [...new Set(frontendCalledCommands)].filter(
      name => HOOK_ONLY_COMMANDS.includes(name),
    );

    expect(leaked, `前端不该直接 invoke 钩子专用命令：${leaked.join(', ')}`).toEqual([]);

    const injected = existsSync(ALIYUN_LOGIN_RS)
      ? readFileSync(ALIYUN_LOGIN_RS, 'utf8')
      : '';
    const absent = HOOK_ONLY_COMMANDS.filter(name => !injected.includes(name));

    // 只在清单里挂着、注入脚本里没人调 = 一条凭空登记的接口。
    expect(absent, `aliyun_login.rs 的注入脚本里没有调用：${absent.join(', ')}`).toEqual([]);
  });

  it('前端订阅内嵌登录窗的事件频道，但不碰窗口的 label', () => {
    const formSource = readFileSync(
      join(SRC_ROOT, 'modules', 'settings', 'ui', 'categories', 'remote', 'connection-form.vue'),
      'utf8',
    );

    // 内嵌 webview 方案（批次 29）反转了旧契约：登录进度由后端事件推给前端，
    // 前端不再自己轮询扫码态 —— 这正是「手机说成功、App 无变化」的成因。
    expect(formSource, 'connection-form.vue 没有订阅 afm://aliyun-login')
      .toContain('afm://aliyun-login');
    // label 的真源只有一个：Rust 建窗那一侧。前端拿不到，避免两处漂移。
    expect(formSource, 'connection-form.vue 不应硬编码登录窗的 label')
      .not.toContain('afm-aliyun-login');
  });

  it('契约文档里的命令清单与前端一致', () => {
    expect(contractMissing, `契约文档缺少：${contractMissing.join(', ')}`).toEqual([]);
  });

  describe.skipIf(rustAliyunCommands.length === 0)('Rust 侧（命令接线后自动启用）', () => {
    it('后端确实定义了全部 11 条命令', () => {
      const missing = EXPECTED_COMMANDS.filter(name => !rustAliyunCommands.includes(name));

      expect(missing, `后端缺少命令：${missing.join(', ')}`).toEqual([]);
    });

    it('全部 11 条命令都注册进了 generate_handler!', () => {
      const libSource = readFileSync(LIB_RS, 'utf8');
      const handlerMatch = /generate_handler!\[([\s\S]*?)\]/.exec(libSource);

      expect(handlerMatch, 'lib.rs 里找不到 generate_handler!').not.toBeNull();

      const registered = handlerMatch?.[1] ?? '';
      const missing = EXPECTED_COMMANDS.filter(name => !registered.includes(name));

      // 少注册一条 = 真机上 invoke 得到「command not found」，而单测全绿。
      expect(missing, `generate_handler! 里没有注册：${missing.join(', ')}`).toEqual([]);
    });

    it('没有多余的阿里云盘命令（后端多出来的接口会变成没人用的僵尸命令）', () => {
      const extra = rustAliyunCommands.filter(name => !EXPECTED_COMMANDS.includes(name));

      expect(extra, `多出来的阿里云盘命令：${extra.join(', ')}`).toEqual([]);
    });
  });
});
