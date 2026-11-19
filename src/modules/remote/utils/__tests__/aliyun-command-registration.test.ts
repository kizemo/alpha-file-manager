// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘的 **Tauri 命令契约**门禁。
//
// `remote-command-contract.test.ts` 已经管住了「Rust 参数名 ↔ JS 传出的键」，
// 但它管不到另外两件事，而这两件事在真机上都是**静默失败**：
//
//   1. **命令没注册到 `generate_handler!`** —— 函数写好了，`#[tauri::command]`
//      也加了，唯独漏了注册那一行。前端 `invoke('remote_aliyun_login_start')`
//      在真机上会得到「command not found」，而单测里 `invoke` 是 mock 的，全绿。
//   2. **后端有命令、前端没有调用点** —— 前端少调一个命令，功能就是不存在；
//      反过来前端调了一个后端没有的命令，真机上同样是「command not found」。
//
// 所以这里**直接读源码**：不跑 Rust、不起应用，源码一变测试立刻红。
//
// ⚠️ Rust 侧那几条命令目前**尚未接线**（`src-tauri/src/remote/` 里只有
// `aliyun_api.rs` / `aliyun_auth.rs` / `aliyun_provider.rs` 三个纯逻辑文件，
// 一个 `#[tauri::command]` 都没有）。所以 Rust 侧的两组断言用
// `describe.skipIf` 挂起：**后端一旦定义出第一条 `remote_aliyun_*` 命令，
// 这两组断言立刻自动生效** —— 不用改测试，也不会一直挂空。

import { describe, expect, it } from 'vitest';
import { existsSync, readFileSync, readdirSync } from 'node:fs';
import { join } from 'node:path';

// 本文件在 `src/modules/remote/utils/__tests__/` 下，往上五层才是仓库根。
const REPO_ROOT = join(__dirname, '..', '..', '..', '..', '..');
const SRC_ROOT = join(REPO_ROOT, 'src');
const RUST_SRC = join(REPO_ROOT, 'src-tauri', 'src');
const LIB_RS = join(RUST_SRC, 'lib.rs');
const CONTRACT_DOC = join(REPO_ROOT, '.superpowers', 'sdd', 'b26-aliyun-contract.md');

/**
 * 阿里云盘前端实际调用的全部命令。
 *
 * 六条，少一条功能不存在，多一条就是「定义了却没人调」的僵尸命令。
 */
const EXPECTED_COMMANDS = [
  'remote_aliyun_login_start',
  'remote_aliyun_login_pending',
  'remote_aliyun_login_claim',
  'remote_aliyun_login_cancel',
  'remote_aliyun_auth_status',
  'remote_aliyun_delete_session',
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

/** Rust 侧定义的全部 `remote_aliyun_*` 命令（目前为空）。 */
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

/** 契约文档里的命令清单（文档不在版本库里，干净检出里没有）。 */
const contractDoc = existsSync(CONTRACT_DOC) ? readFileSync(CONTRACT_DOC, 'utf8') : null;
const contractMissing = contractDoc === null
  ? []
  : EXPECTED_COMMANDS.filter(name => !contractDoc.includes(name));

describe('阿里云盘的 Tauri 命令契约', () => {
  it('前端调用的阿里云盘命令恰好是这 6 条，不多不少', () => {
    const called = [...new Set(frontendCalledCommands)].sort();
    const expected = [...EXPECTED_COMMANDS].sort();

    expect(called, `前端实际调用的阿里云盘命令：${called.join(', ')}`).toEqual(expected);
  });

  it('前端只用阿里云盘自己的事件频道', () => {
    const formSource = readFileSync(
      join(SRC_ROOT, 'modules', 'settings', 'ui', 'categories', 'remote', 'connection-form.vue'),
      'utf8',
    );

    expect(formSource).toContain('afm://aliyun-login');
    // 前端不碰授权窗的 label —— 那是后端建窗时的事。
    expect(formSource).not.toContain('afm-aliyun-login');
  });

  it('契约文档里的命令清单与前端一致', () => {
    expect(contractMissing, `契约文档缺少：${contractMissing.join(', ')}`).toEqual([]);
  });

  describe.skipIf(rustAliyunCommands.length === 0)('Rust 侧（命令接线后自动启用）', () => {
    it('后端确实定义了全部 6 条命令', () => {
      const missing = EXPECTED_COMMANDS.filter(name => !rustAliyunCommands.includes(name));

      expect(missing, `后端缺少命令：${missing.join(', ')}`).toEqual([]);
    });

    it('全部 6 条命令都注册进了 generate_handler!', () => {
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
