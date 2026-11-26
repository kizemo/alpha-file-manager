// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-19（批次 23）：设备（手机 / MTP）相关文案的 **i18n 覆盖**门禁。
//
// `messages-key-alignment.test.ts` 保证 17 个语言包的 key 集合一致，
// `messages-used-keys-exist.test.ts` 保证代码里静态引用的 key 在 en.json 里存在。
// 这两条合起来**仍然漏掉一种情况**：`sync-i18n` 只补 key、**不翻译** ——
// 新加的 key 在 16 个语言包里会是一模一样的英文。
// 界面上就出现「中文界面里夹着一句英文」，
// 而所有自动门禁都是绿的。
//
// 所以这里把这批 key **逐一列出来**，对 17 个语言包逐个核对：
// 必须存在、必须是非空字符串、并且中英文不得一字不差。

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const messagesDirectory = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  'messages',
);

/** 批次 23 新增的 key 的**完整**清单。少列一个 = 少一条界面文案。 */
const SHELL_DEVICE_LEAF_KEYS = [
  'fileBrowser.devices',
  'fileBrowser.device',
  'fileBrowser.deviceOpenFailed',
  'fileBrowser.deviceOpenFailedDescription',
  'fileBrowser.openInSystemExplorer',
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

describe('设备相关文案的语言包覆盖', () => {
  it('语言包数量没变（17 个）', () => {
    expect(locales).toHaveLength(17);
    expect(locales).toContain('en');
    expect(locales).toContain('ch');
  });

  it.each(locales)('%s 语言的 5 个设备键都在且非空', (locale) => {
    const messages = readLocale(locale);
    const problems: string[] = [];

    for (const keyPath of SHELL_DEVICE_LEAF_KEYS) {
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

    const identical = SHELL_DEVICE_LEAF_KEYS
      .filter(keyPath => readLeaf(ch, keyPath) === readLeaf(en, keyPath));

    expect(identical, `这些键中英文一字不差，多半是没翻译：\n  ${identical.join('\n  ')}`).toEqual([]);
  });

  it('中文用的是简体中文表述', () => {
    const ch = readLocale('ch');

    // 「设备」在简体中文里就是这两个字；繁体写作「裝置」。
    expect(readLeaf(ch, 'fileBrowser.devices')).toBe('设备');
    expect(readLeaf(ch, 'fileBrowser.device')).toBe('设备');
  });
});
