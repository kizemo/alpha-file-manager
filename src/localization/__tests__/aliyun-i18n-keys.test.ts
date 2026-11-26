// SPDX-License-Identifier: GPL-3.0-or-later
// GNU General Public License v3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// 阿里云盘的 **i18n 覆盖**门禁。
//
// `messages-key-alignment.test.ts` 保证 17 个语言包的 key 集合一致，
// `messages-used-keys-exist.test.ts` 保证代码里引用的 key 在 en.json 里存在。
// 这两条合起来**仍然漏掉两种情况**：
//   ① 阿里云盘这批 key 有一部分是**动态**取用的
//      （`t(\`settings.remote.aliyun.status.${status}\`)`），静态扫描看不见；
//      而 `status` 的取值来自后端 —— 少一个 `stale`，界面就会把 key 原文
//      直接显示给用户。
//   ② 有人往语言包里塞了 `aliyun*` 的键却**没登记进契约清单**，
//      key 对齐门禁只看集合是否一致，看不出「多出来的键是不是有人忘了登记」。
//
// 所以这里把阿里云盘的 27 个键**逐一列出来**（来源：`.superpowers/sdd/b26-aliyun-contract.md` §4.5
// 加上批次 29 内嵌登录窗的实现记录 `.superpowers/sdd/b29-webview-impl.md`），
// 对 17 个语言包逐个核对：必须存在、必须是非空字符串、
// 并且**语言包里出现的 aliyun 键集合与本清单完全相等**（不多不少）。

import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { describe, expect, it } from 'vitest';

const messagesDirectory = path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '..',
  'messages',
);

/**
 * 阿里云盘这批 key 的**完整**清单。少列一个 = 少一条界面文案。
 *
 * 构成：
 *   1. 现有 quark 键集逐键替换（quark → aliyun、quark_pan → aliyun_pan、
 *      quarkLoginRequired → aliyunLoginRequired、settings.remote.netdisk.quark →
 *      settings.remote.netdisk.aliyun）—— 19 个。
 *   2. 阿里云盘**自己**的界面文案 —— 8 个（`field.*` 2 个 + 二维码面板补强 2 个
 *      + 内嵌登录窗 4 个）。合计 27 个。
 */
const ALIYUN_LEAF_KEYS = [
  'settings.remote.protocol.aliyun_pan',
  'settings.remote.error.aliyunLoginRequired',
  'settings.remote.aliyun.status.loading',
  'settings.remote.aliyun.status.missing',
  'settings.remote.aliyun.status.ready',
  'settings.remote.aliyun.status.stale',
  'settings.remote.aliyun.status.stale_soon',
  'settings.remote.aliyun.action.deleteSession',
  'settings.remote.aliyun.embedded.title',
  'settings.remote.aliyun.embedded.start',
  'settings.remote.aliyun.embedded.cancel',
  'settings.remote.aliyun.embedded.hint',
  'settings.remote.aliyun.embedded.status.waiting',
  'settings.remote.aliyun.embedded.status.ready',
  'settings.remote.aliyun.embedded.status.closed',
  'settings.remote.aliyun.embedded.status.timeout',
  'settings.remote.aliyun.embedded.status.error',
  'settings.remote.aliyun.embedded.saved',
  'settings.remote.netdisk.aliyun',
  // 2026-11-20 阿里云盘专属字段文案（扫码登录的「粘贴 access_token」兜底）。
  'settings.remote.aliyun.field.pasteToken',
  'settings.remote.aliyun.field.pasteTokenHint',
  // 2026-11-20 二维码面板补的两键：手动刷新 + 「手机已扫码」中间态。
  'settings.remote.aliyun.embedded.refresh',
  'settings.remote.aliyun.embedded.status.scanned',
  // 2026-11-20 内嵌 webview 登录窗（主路径）的四键；二维码面板降级为备用。
  'settings.remote.aliyun.embedded.windowStart',
  'settings.remote.aliyun.embedded.windowCancel',
  'settings.remote.aliyun.embedded.windowHint',
  'settings.remote.aliyun.embedded.qrFallback',
];

/**
 * 阿里云盘**自己**的键（没有 quark 来源键，不属于逐键替换的产物）。
 *
 * 前两对是粘贴 token 兜底；`refresh` / `status.scanned` 来自二维码面板的补强；
 * 最后四个来自内嵌登录窗 —— 夸克那边没有对应的界面，所以只能自己登记。
 */
const ALIYUN_ONLY_LEAF_KEYS = [
  'settings.remote.aliyun.field.pasteToken',
  'settings.remote.aliyun.field.pasteTokenHint',
  'settings.remote.aliyun.embedded.refresh',
  'settings.remote.aliyun.embedded.status.scanned',
  'settings.remote.aliyun.embedded.windowStart',
  'settings.remote.aliyun.embedded.windowCancel',
  'settings.remote.aliyun.embedded.windowHint',
  'settings.remote.aliyun.embedded.qrFallback',
];

/**
 * 产品名键：品牌原文在多数语言里就写作 `Aliyun Drive`，
 * 不参与「有没有翻译」的判定。
 */
const BRAND_KEYS = new Set([
  'settings.remote.protocol.aliyun_pan',
  'settings.remote.netdisk.aliyun',
]);

/**
 * 允许与英文**一字不差**的键。
 *
 * 扫码登录那两个键在 17 个语言包里都**逐字翻译过** ⇒ 这里没有例外项。
 */
const ALLOWED_IDENTICAL_TO_ENGLISH = new Set<string>();

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

/** 收集一棵 JSON 树里所有叶子（字符串）值的点分路径。 */
function collectLeafPaths(node: unknown, prefix = ''): string[] {
  if (node === null || typeof node !== 'object' || Array.isArray(node)) {
    return prefix === '' ? [] : [prefix];
  }

  const paths: string[] = [];

  for (const [key, value] of Object.entries(node as JsonObject)) {
    const next = prefix === '' ? key : `${prefix}.${key}`;

    paths.push(...collectLeafPaths(value, next));
  }

  return paths;
}

/**
 * quark 键 → aliyun 键的**逐键替换**规则：
 * `quark` → `aliyun`、`quark_pan` → `aliyun_pan`、`quarkLoginRequired` → `aliyunLoginRequired`。
 */
function mapQuarkKeyToAliyun(keyPath: string): string {
  return keyPath
    .split('.')
    .map((segment) => {
      if (segment === 'quark') {
        return 'aliyun';
      }

      if (segment === 'quark_pan') {
        return 'aliyun_pan';
      }

      if (segment === 'quarkLoginRequired') {
        return 'aliyunLoginRequired';
      }

      return segment;
    })
    .join('.');
}

const locales = fs
  .readdirSync(messagesDirectory)
  .filter(fileName => fileName.endsWith('.json'))
  .map(fileName => fileName.replace(/\.json$/, ''))
  .sort();

describe('阿里云盘的语言包覆盖', () => {
  it('语言包数量没变（17 个）', () => {
    expect(locales).toHaveLength(17);
    expect(locales).toContain('en');
    expect(locales).toContain('ch');
  });

  it('契约清单本身不重不漏', () => {
    expect(new Set(ALIYUN_LEAF_KEYS).size).toBe(ALIYUN_LEAF_KEYS.length);
  });

  it('登记清单 == quark 键集逐键替换 + 8 个专属键（漏了就红）', () => {
    const english = readLocale('en');
    const quarkPaths = collectLeafPaths(english).filter(keyPath => keyPath.includes('quark'));
    const derived = [
      ...quarkPaths.map(keyPath => mapQuarkKeyToAliyun(keyPath)),
      ...ALIYUN_ONLY_LEAF_KEYS,
    ].sort();

    const missing = derived.filter(keyPath => !ALIYUN_LEAF_KEYS.includes(keyPath));
    const extra = ALIYUN_LEAF_KEYS
      .filter(keyPath => !derived.includes(keyPath))
      .sort();

    expect(
      missing,
      `quark 键集逐键替换后还差这些阿里云键，补进 ALIYUN_LEAF_KEYS：\n  ${missing.join('\n  ')}`,
    ).toEqual([]);
    expect(
      extra,
      `这些阿里云键在 quark 键集里没有对应的来源键，别自造：\n  ${extra.join('\n  ')}`,
    ).toEqual([]);
  });

  it.each(locales)('%s 语言的 27 个阿里云键都在且非空', (locale) => {
    const messages = readLocale(locale);
    const problems: string[] = [];

    for (const keyPath of ALIYUN_LEAF_KEYS) {
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

  it.each(locales)('%s 语言里的 aliyun 键集合与登记清单完全相等', (locale) => {
    const messages = readLocale(locale);
    const found = collectLeafPaths(messages).filter(keyPath => keyPath.toLowerCase().includes('aliyun'));
    const expected = [...ALIYUN_LEAF_KEYS].sort();

    const missing = expected.filter(keyPath => !found.includes(keyPath));
    const unregistered = found.filter(keyPath => !ALIYUN_LEAF_KEYS.includes(keyPath)).sort();

    expect(missing, `${locale}.json 缺这些登记过的键：\n  ${missing.join('\n  ')}`).toEqual([]);
    expect(
      unregistered,
      `${locale}.json 里有未登记进契约清单的 aliyun 键：\n  ${unregistered.join('\n  ')}`,
    ).toEqual([]);
  });

  it.each(locales.filter(locale => locale !== 'en'))('%s 语言没有整段照抄英文', (locale) => {
    const messages = readLocale(locale);
    const english = readLocale('en');
    const problems: string[] = [];

    for (const keyPath of ALIYUN_LEAF_KEYS) {
      if (BRAND_KEYS.has(keyPath)) {
        continue;
      }

      const value = readLeaf(messages, keyPath);
      const englishValue = readLeaf(english, keyPath);

      if (value === englishValue && !ALLOWED_IDENTICAL_TO_ENGLISH.has(`${locale}:${keyPath}`)) {
        problems.push(keyPath);
      }
    }

    expect(
      problems,
      `${locale}.json 这些键与英文一字不差，多半是没翻译：\n  ${problems.join('\n  ')}`,
    ).toEqual([]);
  });
});
