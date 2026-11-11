// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 夸克网盘的 **i18n 覆盖**门禁。
//
// `messages-key-alignment.test.ts` 保证 17 个语言包的 key 集合一致，
// `messages-used-keys-exist.test.ts` 保证代码里引用的 key 在 en.json 里存在。
// 这两条合起来**仍然漏掉一种情况**：夸克这批 key 是**动态**取用的
// （`t(\`settings.remote.quark.status.${status}\`)`），静态扫描看不见它们；
// 而 `status` 的取值来自后端 —— 少一个 `stale`，界面就会把 key 原文
// 直接显示给用户。
//
// 所以这里把夸克的 24 个键**逐一列出来**，对 17 个语言包逐个核对：
// 必须存在、必须是非空字符串、并且中英文都要有（英文是基准语言，
// 它的值同时是其它语言的兜底）。

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const messagesDirectory = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  'messages',
);

/** 夸克这批 key 的**完整**清单。少列一个 = 少一条界面文案。 */
const QUARK_LEAF_KEYS = [
  'settings.remote.protocol.quark_pan',
  'settings.remote.error.quarkLoginRequired',
  'settings.remote.quark.status.loading',
  'settings.remote.quark.status.missing',
  'settings.remote.quark.status.ready',
  'settings.remote.quark.status.stale',
  'settings.remote.quark.status.stale_soon',
  'settings.remote.quark.action.deleteSession',
  'settings.remote.quark.embedded.title',
  'settings.remote.quark.embedded.start',
  'settings.remote.quark.embedded.cancel',
  'settings.remote.quark.embedded.hint',
  'settings.remote.quark.embedded.status.waiting',
  'settings.remote.quark.embedded.status.ready',
  'settings.remote.quark.embedded.status.closed',
  'settings.remote.quark.embedded.status.timeout',
  'settings.remote.quark.embedded.status.error',
  'settings.remote.quark.embedded.saved',
  'settings.remote.netdisk.provider',
  'settings.remote.netdisk.baidu',
  'settings.remote.netdisk.quark',
  // 2026-11-11「添加网盘」流程改造新增的三条文案。
  'settings.remote.netdisk.signedIn',
  'settings.remote.nav.netdiskDescription',
  'settings.remote.nav.remoteDescription',
];

type JsonObject = { [key: string]: unknown };

function readLocale(locale: string): JsonObject {
  const filePath = path.join(messagesDirectory, `${locale}.json`);

  return JSON.parse(fs.readFileSync(filePath, 'utf8')) as JsonObject;
}

function readLeaf(root: JsonObject, keyPath: string): unknown {
  let node: unknown = root;

  for (const part of keyPath.split('.')) {
    if (node === null || typeof node !== 'object') {
      return undefined;
    }

    node = (node as JsonObject)[part];
  }

  return node;
}

const locales = fs
  .readdirSync(messagesDirectory)
  .filter(fileName => fileName.endsWith('.json'))
  .map(fileName => fileName.replace(/\.json$/, ''))
  .sort();

describe('夸克网盘的语言包覆盖', () => {
  it('语言包数量没变（17 个）', () => {
    expect(locales).toHaveLength(17);
    expect(locales).toContain('en');
    expect(locales).toContain('ch');
  });

  it.each(locales)('%s 语言的 24 个夸克键都在且非空', (locale) => {
    const messages = readLocale(locale);
    const problems: string[] = [];

    for (const keyPath of QUARK_LEAF_KEYS) {
      const value = readLeaf(messages, keyPath);

      if (typeof value !== 'string') {
        problems.push(`${keyPath} 缺失`);
      }
      else if (value.trim() === '') {
        problems.push(`${keyPath} 是空字符串`);
      }
    }

    expect(problems, `${locale}.json 有问题：\n  ${problems.join('\n  ')}`).toEqual([]);
  });

  it('中文与英文的文案确实不同（没被 sync-i18n 刷成同一份）', () => {
    const ch = readLocale('ch');
    const en = readLocale('en');

    const identical = QUARK_LEAF_KEYS.filter(keyPath => readLeaf(ch, keyPath) === readLeaf(en, keyPath));

    // 协议名「夸克网盘」在英文里本来就写作 Quark Drive，不算「没翻译」。
    const allowedIdentical = ['settings.remote.netdisk.baidu'];

    const unexpected = identical.filter(keyPath => !allowedIdentical.includes(keyPath));

    expect(unexpected, `这些键中英文一字不差，多半是没翻译：\n  ${unexpected.join('\n  ')}`).toEqual([]);
  });
});
