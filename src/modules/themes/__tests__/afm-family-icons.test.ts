// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-31（AFM r15 追加需求）：右上角图标 + 地址栏旁图标按主题配色区分。
//
// 这份用例守三件事：
//   1. **每套 AFM 主题都必须定义全部族色 token**（之前只有跃动活力接了线，
//      冷杉那 8 个还是跃动活力的紫色复制品且无人读取 —— 纯死 token）。
//   2. **族色必须可读**：对各自主题的标题栏底色与页面底色都 ≥ 3:1（WCAG 实算，不靠肉眼）。
//   3. **主题之间必须有区别**：暖纸=低饱和红（h 5..24）、冷杉=冷色（h 180..235）、
//      跃动活力不得有族色落在暖纸那个红区间里。
//
// 另外守住「右上角 / 地址栏旁这两处图标在 4 套主题下都真的接了线」。

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { AFM_THEMES } from '@/modules/themes/afm-themes';

const REPO_ROOT = process.cwd();

function readRepoFile(...segments: string[]): string {
  return readFileSync(join(REPO_ROOT, ...segments), 'utf8');
}

const AFM_THEMES_CSS = readRepoFile('src', 'styles', 'afm-themes.css');

const FAMILY_TOKENS = [
  'nav',
  'location',
  'refresh',
  'view',
  'thumbnail',
  'split',
  'tree',
  'info',
  'more',
  'extension',
  'search',
  'create',
  'filter',
  'share',
  'status',
] as const;

/** 右上角 + 地址栏旁必须分色的图标 class（接线范围）。 */
const TOP_RIGHT_SELECTORS = [
  '.navigator-toolbar-actions__icon--split',
  '.navigator-toolbar-actions__icon--tree',
  '.navigator-toolbar-actions__icon--info',
  '.navigator-toolbar-actions__icon--compare',
  '.navigator-toolbar-actions__icon--more',
  '.lan-share-toolbar-button__icon',
  '.status-center-toolbar-button__icon',
  '.extensions-toolbar-actions__icon--extension',
  '.settings-search-toolbar-button__icon--search',
] as const;

const ADDRESS_BAR_SELECTORS = [
  '.file-browser-toolbar-nav-buttons__icon.file-browser-toolbar-nav-buttons__icon--nav',
  '.file-browser-toolbar-nav-buttons__icon.file-browser-toolbar-nav-buttons__icon--location',
  '.file-browser-toolbar-nav-buttons__icon.file-browser-toolbar-nav-buttons__icon--refresh',
  '.file-browser-toolbar-nav-buttons__icon.file-browser-toolbar-nav-buttons__icon--view',
  '.file-browser-toolbar-nav-buttons__icon.file-browser-toolbar-nav-buttons__icon--thumbnail',
  '.file-browser-toolbar-nav-buttons__icon.file-browser-toolbar-nav-buttons__icon--more',
  '.file-browser-toolbar-create-button__icon--create',
  '.file-browser-toolbar-filter__icon--filter',
  '.file-browser-toolbar-swap-panes-button__icon--split',
  '.file-browser-toolbar-sync-browsing-button__icon--split',
] as const;

const BARE_HSL_PATTERN = /^\d+(?:\.\d+)?\s+\d+(?:\.\d+)?%\s+\d+(?:\.\d+)?%$/;

interface Hsl {
  h: number;
  s: number;
  l: number;
}

interface Rgb {
  r: number;
  g: number;
  b: number;
}

function parseHslTriplet(value: string): Hsl {
  const [hue, saturation, lightness] = value.trim().split(/\s+/);

  return {
    h: Number.parseFloat(hue),
    s: Number.parseFloat(saturation) / 100,
    l: Number.parseFloat(lightness) / 100,
  };
}

function hslToRgb({ h, s, l }: Hsl): Rgb {
  const chroma = (1 - Math.abs(2 * l - 1)) * s;
  const hueSector = (((h % 360) + 360) % 360) / 60;
  const second = chroma * (1 - Math.abs((hueSector % 2) - 1));

  let r = 0;
  let g = 0;
  let b = 0;

  if (hueSector < 1) {
    r = chroma;
    g = second;
  }
  else if (hueSector < 2) {
    r = second;
    g = chroma;
  }
  else if (hueSector < 3) {
    g = chroma;
    b = second;
  }
  else if (hueSector < 4) {
    g = second;
    b = chroma;
  }
  else if (hueSector < 5) {
    r = second;
    b = chroma;
  }
  else {
    r = chroma;
    b = second;
  }

  const offset = l - chroma / 2;

  return {
    r: r + offset,
    g: g + offset,
    b: b + offset,
  };
}

function linearizeChannel(value: number): number {
  return value <= 0.03928
    ? value / 12.92
    : ((value + 0.055) / 1.055) ** 2.4;
}

function relativeLuminance(color: Rgb): number {
  return 0.2126 * linearizeChannel(color.r)
    + 0.7152 * linearizeChannel(color.g)
    + 0.0722 * linearizeChannel(color.b);
}

function contrastRatio(foregroundToken: string, backgroundToken: string): number {
  const foreground = relativeLuminance(hslToRgb(parseHslTriplet(foregroundToken)));
  const background = relativeLuminance(hslToRgb(parseHslTriplet(backgroundToken)));

  const lighter = Math.max(foreground, background);
  const darker = Math.min(foreground, background);

  return (lighter + 0.05) / (darker + 0.05);
}

describe('AFM 主题族色（右上角 + 地址栏旁图标）', () => {
  it('defines every family colour for every AFM theme', () => {
    for (const theme of AFM_THEMES) {
      for (const family of FAMILY_TOKENS) {
        const token = `--family-${family}` as `--family-${string}`;
        const value = theme.variables[token];

        expect(value, `${theme.id} 缺少 ${token}`).toBeDefined();
        expect(value, `${theme.id} 的 ${token} 不是裸 HSL 三元组：${value}`)
          .toMatch(BARE_HSL_PATTERN);
      }
    }
  });

  it('keeps every family colour legible against the theme surfaces', () => {
    for (const theme of AFM_THEMES) {
      for (const family of FAMILY_TOKENS) {
        const token = `--family-${family}` as `--family-${string}`;
        const value = theme.variables[token];

        const toolbarRatio = contrastRatio(value, theme.variables['--window-toolbar']);
        const backgroundRatio = contrastRatio(value, theme.variables['--background']);

        expect(
          toolbarRatio,
          `${theme.id} 的 ${token} 对标题栏底色只有 ${toolbarRatio.toFixed(2)}:1`,
        ).toBeGreaterThanOrEqual(3);
        expect(
          backgroundRatio,
          `${theme.id} 的 ${token} 对页面底色只有 ${backgroundRatio.toFixed(2)}:1`,
        ).toBeGreaterThanOrEqual(3);
      }
    }
  });

  it('keeps the warm-paper share and status icons in the low-saturation red band', () => {
    for (const theme of AFM_THEMES.filter(item => item.id.startsWith('afm-warm'))) {
      for (const family of ['share', 'status'] as const) {
        const { h, s } = parseHslTriplet(theme.variables[`--family-${family}`]);

        expect(h, `${theme.id} 的 --family-${family} 色相 ${h} 不在 5..24`).toBeGreaterThanOrEqual(5);
        expect(h, `${theme.id} 的 --family-${family} 色相 ${h} 不在 5..24`).toBeLessThanOrEqual(24);
        expect(s * 100, `${theme.id} 的 --family-${family} 饱和度不在 26..42%`)
          .toBeGreaterThanOrEqual(26);
        expect(s * 100, `${theme.id} 的 --family-${family} 饱和度不在 26..42%`)
          .toBeLessThanOrEqual(42);
      }
    }
  });

  it('keeps the cool theme on a cold hue ramp', () => {
    const cool = AFM_THEMES.find(theme => theme.id === 'afm-cool-light');

    expect(cool).toBeDefined();

    for (const family of FAMILY_TOKENS) {
      const { h } = parseHslTriplet(cool!.variables[`--family-${family}`]);

      expect(h, `afm-cool-light 的 --family-${family} 色相 ${h} 不在冷色区间 180..235`)
        .toBeGreaterThanOrEqual(180);
      expect(h, `afm-cool-light 的 --family-${family} 色相 ${h} 不在冷色区间 180..235`)
        .toBeLessThanOrEqual(235);
    }
  });

  it('never lets the vivid theme fall into the warm-paper red band', () => {
    const vivid = AFM_THEMES.find(theme => theme.id === 'afm-vivid-light');

    expect(vivid).toBeDefined();

    for (const family of FAMILY_TOKENS) {
      const { h, s } = parseHslTriplet(vivid!.variables[`--family-${family}`]);

      const inRedBand = h >= 5 && h <= 24 && s * 100 >= 26;

      expect(inRedBand, `afm-vivid-light 的 --family-${family} 落进了暖纸的红区间`).toBe(false);
    }
  });

  it('gives every theme its own toolbar action colour', () => {
    const values = AFM_THEMES.map(theme => theme.variables['--toolbar-action-foreground']);

    expect(new Set(values).size).toBe(AFM_THEMES.length);

    const hues = AFM_THEMES.map(theme => parseHslTriplet(theme.variables['--toolbar-action-foreground']).h);
    const warm = hues.filter(hue => hue >= 5 && hue <= 24);
    const violet = hues.filter(hue => hue >= 240 && hue <= 270);
    const cool = hues.filter(hue => hue >= 185 && hue <= 230);

    expect(warm.length, '暖纸两个主题的右上角按钮色应当是低饱和红').toBe(2);
    expect(violet.length, '跃动活力的右上角按钮色应当是紫色').toBe(1);
    expect(cool.length, '冷杉的右上角按钮色应当是钢蓝').toBe(1);
  });

  it('wires the top-right and address-bar icons in all four themes', () => {
    for (const theme of AFM_THEMES) {
      const scope = `html[data-afm-theme='${theme.cssThemeId}']`;

      for (const selector of [...TOP_RIGHT_SELECTORS, ...ADDRESS_BAR_SELECTORS]) {
        expect(
          AFM_THEMES_CSS,
          `${theme.id} 没有接线 ${selector}`,
        ).toContain(`${scope} ${selector}`);
      }
    }
  });

  // FORK-MODIFICATION 2026-10-31（r16 追加需求）：暖纸主题的「信息面板」「目录树」两个图标
  // 必须是低饱和红。但通用激活态规则 `html[...] .navigator-toolbar-actions__button--active
  // .navigator-toolbar-actions__icon`（特异性 0,3,1）会压过族色规则（0,2,1），
  // 而目录树按钮默认就带 `--active` ⇒ 图标会被染成暖纸主题的 --primary（品牌钢蓝）。
  // 所以这两个图标必须显式声明「激活态仍回族色」，并用本用例守住这条规则不被删。
  it('keeps the tree and info icons on their family colour even while active', () => {
    const pairs = [
      ['.navigator-toolbar-actions__icon--tree', '--family-tree'],
      ['.navigator-toolbar-actions__icon--info', '--family-info'],
    ] as const;

    for (const theme of AFM_THEMES) {
      const scope = `html[data-afm-theme='${theme.cssThemeId}']`;

      for (const [icon, token] of pairs) {
        const selector = `${scope} .navigator-toolbar-actions__button--active ${icon}`;
        // CSS 里四个主题是**并列选择器**写在同一个规则块里的，
        // 所以不能找 `${selector} {` —— 只有并列块里最后一个主题后面才跟 `{`。
        // 改成：先定位该选择器，再往后找它所在规则块的 `{` 与 `}`。
        const index = AFM_THEMES_CSS.indexOf(selector);

        expect(index, `${theme.id} 缺少「激活态仍回族色」的规则：${selector}`).toBeGreaterThan(-1);

        const blockStart = AFM_THEMES_CSS.indexOf('{', index);
        const block = AFM_THEMES_CSS.slice(blockStart, AFM_THEMES_CSS.indexOf('}', blockStart));

        expect(block, `${selector} 没有回 ${token}`).toContain(`color: hsl(var(${token}))`);
        expect(block, `${selector} 没有回 ${token}`).toContain(`stroke: hsl(var(${token}))`);
      }
    }
  });
});
