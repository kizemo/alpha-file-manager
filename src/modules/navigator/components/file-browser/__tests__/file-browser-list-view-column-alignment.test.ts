// SPDX-License-Identifier: GPL-3.0-or-later
// GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-11-24（用户指出）：列表视图「可用空间」列的文字贴在格子顶部，
// 和其它列的文字对不齐。
//
// 根因：`.file-browser-list-view__entry` 是 grid 容器，行里的每个列单元格都是它的子项，
// 子项默认被拉伸到整行高度。其它列之所以是垂直居中的，是因为它们各自有
// `display: flex` + `align-items: center`；而「可用空间」列**整列都没有任何 CSS 规则**，
// 于是它是一个普通 span，文字就顶在格子上沿。
//
// 这条用例把「每一列的单元格都必须有垂直居中规则」锁死：以后再加一列却忘了加居中，
// 这里就会红。表头那一侧同理（表头格与表头文字都要居中）。
//
// 断言方式是**源码文本级**的：vitest 默认不注入 SFC 的样式（`css: false`），
// jsdom 里拿不到 `getComputedStyle` 的真实层叠结果，写渲染断言只会得到假绿。
// 所以这里直接读 `.vue` 的 `<style>` 文本并解析规则 —— 与本仓
// `recycle-bin-delete-contract.test.ts` / `afm-themes.test.ts` 的契约风格一致。

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { FILE_BROWSER_LIST_REORDERABLE_COLUMN_IDS } from '../utils/file-browser-list-columns';

interface StyleRule {
  selectors: string[];
  declarations: Map<string, string>;
}

/** 读同目录（file-browser 组件目录）下的一个文件。 */
function readComponentFile(fileName: string): string {
  return readFileSync(join(__dirname, '..', fileName), 'utf8');
}

/** 取出文件里所有 `<style>` 块的正文（scoped 与非 scoped 都要）。 */
function extractStyleBlocks(source: string): string {
  const blocks: string[] = [];
  const pattern = /<style[^>]*>([\s\S]*?)<\/style>/g;
  let match = pattern.exec(source);

  while (match !== null) {
    blocks.push(match[1]);
    match = pattern.exec(source);
  }

  return blocks.join('\n');
}

/** 解析一条规则体内的声明。 */
function parseDeclarations(body: string): Map<string, string> {
  const declarations = new Map<string, string>();

  for (const chunk of body.split(';')) {
    const separatorIndex = chunk.indexOf(':');

    if (separatorIndex === -1) {
      continue;
    }

    const property = chunk.slice(0, separatorIndex).trim().toLowerCase();
    const value = chunk.slice(separatorIndex + 1).trim();

    if (!property) {
      continue;
    }

    declarations.set(property, value);
  }

  return declarations;
}

/**
 * 极简 CSS 规则解析：取出「选择器 { 声明 }」的平铺规则。
 * `@keyframes` / `@media` 这类带嵌套花括号的整块会被递归展开（内部规则照样算数），
 * 这样就不用给测试引入 postcss 之类的额外依赖。
 */
function parseStyleRules(css: string): StyleRule[] {
  const withoutComments = css.replace(/\/\*[\s\S]*?\*\//g, '');
  const rules: StyleRule[] = [];
  let index = 0;

  while (index < withoutComments.length) {
    const openIndex = withoutComments.indexOf('{', index);

    if (openIndex === -1) {
      break;
    }

    const selector = withoutComments.slice(index, openIndex).trim();
    let depth = 1;
    let cursor = openIndex + 1;

    while (cursor < withoutComments.length && depth > 0) {
      const char = withoutComments[cursor];

      if (char === '{') {
        depth += 1;
      }

      if (char === '}') {
        depth -= 1;
      }

      cursor += 1;
    }

    const body = withoutComments.slice(openIndex + 1, cursor - 1);

    if (selector.startsWith('@')) {
      rules.push(...parseStyleRules(body));
    }
    else {
      rules.push({
        selectors: selector.split(',').map(part => part.trim()).filter(Boolean),
        declarations: parseDeclarations(body),
      });
    }

    index = cursor;
  }

  return rules;
}

/** 找出所有**选择器正好是**这个 class 的规则（排除后代组合选择器）。 */
function findRulesForClass(rules: StyleRule[], className: string): StyleRule[] {
  const selector = `.${className}`;

  return rules.filter(rule => rule.selectors.includes(selector));
}

/** 这条规则是否声明了「flex 容器 + 交叉轴居中」。 */
function isVerticallyCentered(rule: StyleRule): boolean {
  return rule.declarations.get('display') === 'flex'
    && rule.declarations.get('align-items') === 'center';
}

const LIST_VIEW_RULES = parseStyleRules(extractStyleBlocks(
  readComponentFile('file-browser-list-view.vue'),
));

const HEADER_CELL_RULES = parseStyleRules(extractStyleBlocks(
  readComponentFile('file-browser-list-header-cell.vue'),
));

const HEADER_RULES = parseStyleRules(extractStyleBlocks(
  readComponentFile('file-browser-list-header.vue'),
));

/**
 * 名称列的单元格不走 `__entry-${columnId}` 那一套
 * （它是在 list-view 模板里单独渲染的 div），所以单独点名。
 */
const NAME_COLUMN_CELL_CLASS = 'file-browser-list-view__entry-name';

const COLUMN_CELL_CLASSES = [
  NAME_COLUMN_CELL_CLASS,
  ...FILE_BROWSER_LIST_REORDERABLE_COLUMN_IDS.map(columnId => `file-browser-list-view__entry-${columnId}`),
];

describe('列表视图列单元格的垂直对齐', () => {
  it('样式解析器确实读到了规则（防止解析失败导致下面的断言假绿）', () => {
    expect(LIST_VIEW_RULES.length).toBeGreaterThan(20);
    expect(HEADER_CELL_RULES.length).toBeGreaterThan(0);
    expect(HEADER_RULES.length).toBeGreaterThan(0);
  });

  it('每一列的单元格都有垂直居中的规则', () => {
    const notCentered = COLUMN_CELL_CLASSES.filter((className) => {
      const rules = findRulesForClass(LIST_VIEW_RULES, className);

      return !rules.some(isVerticallyCentered);
    });

    expect(notCentered).toEqual([]);
  });

  it('「可用空间」列和其它列共享同一套排版（字号 / 颜色 / 省略号）', () => {
    // 这一列曾经既没有居中规则，也没进「其它列共享」的那条规则组，
    // 于是它的字号和颜色跟旁边的列对不上。
    const sharedRule = findRulesForClass(
      LIST_VIEW_RULES,
      'file-browser-list-view__entry-availableSpace',
    ).find(rule => rule.declarations.has('font-size') && rule.declarations.has('text-overflow'));

    expect(sharedRule).toBeDefined();
    expect(sharedRule?.declarations.get('font-size')).toBe('calc(12px + var(--font-size-offset, 0px))');
    expect(sharedRule?.declarations.get('color')).toBe('hsl(var(--muted-foreground))');
    expect(sharedRule?.declarations.get('text-overflow')).toBe('ellipsis');
  });

  it('表头格与表头文字都是垂直居中的', () => {
    const headerCellCentered = findRulesForClass(HEADER_CELL_RULES, 'file-browser-list-view__header-cell')
      .some(isVerticallyCentered);
    const headerItemCentered = findRulesForClass(HEADER_RULES, 'file-browser-list-view__header-item')
      .some(isVerticallyCentered);

    expect(headerCellCentered).toBe(true);
    expect(headerItemCentered).toBe(true);
  });
});
