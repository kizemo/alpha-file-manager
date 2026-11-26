// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 夸克网盘的 **Tauri 命令契约**门禁。
//
// `remote-command-contract.test.ts` 已经管住了「Rust 参数名 ↔ JS 传出的键」，
// 但它管不到另外两件事，而这两件事在真机上都是**静默失败**：
//
//   1. **命令没注册到 `generate_handler!`** —— 函数写好了，`#[tauri::command]`
//      也加了，唯独漏了注册那一行。前端 `invoke('remote_quark_login_start')`
//      在真机上会得到「command not found」，而单测里 `invoke` 是 mock 的，
//      全绿。
//   2. **后端有命令、前端没有调用点** —— 前端少调一个命令，功能就是不存在。
//
// 所以这里**直接读 Rust 源码**：不跑 Rust、不起应用，源码一变测试立刻红。

import { describe, expect, it } from 'vitest';
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

// 本文件在 `src/modules/remote/utils/__tests__/` 下，往上五层才是仓库根。
const REPO_ROOT = join(__dirname, '..', '..', '..', '..', '..');
const RUST_SRC = join(REPO_ROOT, 'src-tauri', 'src');
const LIB_RS = join(RUST_SRC, 'lib.rs');
const COMMANDS_RS = join(RUST_SRC, 'remote', 'commands.rs');
const QUARK_LOGIN_RS = join(RUST_SRC, 'quark_login.rs');
const STORE_TS = join(REPO_ROOT, 'src', 'stores', 'storage', 'remote-connections.ts');

/**
 * 夸克网盘对外暴露的全部命令。
 *
 * 三条会话命令在 `remote/commands.rs` 里，四条登录窗命令在 `quark_login.rs` 里
 * —— 拆成两个文件是因为登录窗要在**独立线程**里建 WebView2 窗口，
 * 不能塞进连接那套依赖里。
 */
const EXPECTED_COMMANDS = [
  'remote_quark_save_session',
  'remote_quark_auth_status',
  'remote_quark_delete_session',
  'remote_quark_login_start',
  'remote_quark_login_pending',
  'remote_quark_login_claim',
  'remote_quark_login_cancel',
];

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

describe('夸克网盘的 Tauri 命令契约', () => {
  it('后端确实定义了全部 7 条命令', () => {
    const defined = [
      ...parseTauriCommands(readFileSync(COMMANDS_RS, 'utf8')),
      ...parseTauriCommands(readFileSync(QUARK_LOGIN_RS, 'utf8')),
    ];

    const missing = EXPECTED_COMMANDS.filter(name => !defined.includes(name));

    expect(missing, `后端缺少命令：${missing.join(', ')}`).toEqual([]);
  });

  it('全部 7 条命令都注册进了 generate_handler!', () => {
    const libSource = readFileSync(LIB_RS, 'utf8');
    const handlerMatch = /generate_handler!\[([\s\S]*?)\]/.exec(libSource);

    expect(handlerMatch, 'lib.rs 里找不到 generate_handler!').not.toBeNull();

    const registered = handlerMatch?.[1] ?? '';
    const missing = EXPECTED_COMMANDS.filter(name => !registered.includes(name));

    // 少注册一条 = 真机上 invoke 得到「command not found」，而单测全绿。
    expect(missing, `generate_handler! 里没有注册：${missing.join(', ')}`).toEqual([]);
  });

  it('前端每条命令都有调用点', () => {
    const storeSource = readFileSync(STORE_TS, 'utf8');
    const missing = EXPECTED_COMMANDS.filter(name => !storeSource.includes(`'${name}'`));

    expect(missing, `前端没有调用：${missing.join(', ')}`).toEqual([]);
  });

  it('没有多余的夸克命令（后端多出来的接口会变成没人用的僵尸命令）', () => {
    const defined = [
      ...parseTauriCommands(readFileSync(COMMANDS_RS, 'utf8')),
      ...parseTauriCommands(readFileSync(QUARK_LOGIN_RS, 'utf8')),
    ].filter(name => name.startsWith('remote_quark_'));

    const extra = defined.filter(name => !EXPECTED_COMMANDS.includes(name));

    expect(extra, `多出来的夸克命令：${extra.join(', ')}`).toEqual([]);
  });
});
