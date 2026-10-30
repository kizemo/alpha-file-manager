// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题的单测。
//
// 这份用例守的是三组约束：
//   1. **技术约束** —— 裸 HSL 三元组、颜色 token 全覆盖、
//      尺寸 token 一律不动、id 与 `BuiltinThemeId` 同步。
//   2. 用户本轮下达的**框架约束**（C1/C3/C4）—— 图标轨仍是 42px、
//      形状特征层不得出现任何布局属性、圆角阶梯不得推出负值。
//   3. 设计稿的**硬要求**（R2/R3）—— 每套主题都要承载品牌主色、明暗都要成立。
//
// 可读性用 WCAG 相对亮度实算，不靠肉眼。

import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import {
  AFM_THEME_IDS,
  AFM_THEMES,
  getAfmTheme,
  isAfmThemeId,
} from '@/modules/themes/afm-themes';
import {
  findThemeOption,
  getAvailableThemeOptions,
  parseThemeId,
} from '@/modules/themes/registry';
import type { BuiltinThemeId } from '@/types/user-settings';

const REPO_ROOT = process.cwd();

function readRepoFile(...segments: string[]): string {
  return readFileSync(join(REPO_ROOT, ...segments), 'utf8');
}

const AFM_THEMES_CSS = readRepoFile('src', 'styles', 'afm-themes.css');
const INDEX_CSS = readRepoFile('src', 'styles', 'index.css');
const VARS_CSS = readRepoFile('src', 'styles', 'vars.css');
const USER_SETTINGS_TS = readRepoFile('src', 'types', 'user-settings.ts');

/** 基础色板：AFM 主题必须覆盖的必备 token 集合。 */
const BASE_COLOR_TOKENS = [
  '--background',
  '--background-2',
  '--background-3',
  '--window-toolbar',
  '--icon',
  '--foreground',
  '--muted',
  '--muted-foreground',
  '--popover',
  '--popover-foreground',
  '--card',
  '--card-foreground',
  '--border',
  '--input',
  '--primary',
  '--primary-foreground',
  '--secondary',
  '--secondary-foreground',
  '--destructive',
  '--destructive-foreground',
  '--ring',
  '--success',
  '--dangerous',
  '--warning',
] as const;

/** 四套重设计主题新增的品牌 / 点缀 / 图标轨 / 层次 token。 */
const AFM_EXTRA_COLOR_TOKENS = [
  '--brand',
  '--brand-foreground',
  '--accent',
  '--accent-foreground',
  '--accent-ink',
  '--accent-muted',
  '--nav-rail-background',
  '--nav-rail-foreground',
  '--nav-rail-muted-foreground',
  '--brand-ink',
  '--surface-sidebar',
  '--surface-panel',
  '--surface-stripe',
  '--state-hover',
  '--state-selected',
  '--state-active',
  '--border-subtle',
  '--tab-surface-active',
  '--tab-indicator',
  '--scrollbar-track',
  '--scrollbar-thumb',
  '--scrollbar-thumb-hover',
] as const;

const REQUIRED_COLOR_TOKENS = [
  ...BASE_COLOR_TOKENS,
  ...AFM_EXTRA_COLOR_TOKENS,
] as const;

/** 唯一允许自带 alpha 的两个 token（全仓没有对它们做二次加 alpha）。 */
const ALPHA_ALLOWED_TOKENS = ['--popover', '--input'] as const;

/** 裸 HSL 三元组：`H S% L%`，hue 可带小数（基础 token 里有 `220.9 39.3% 11%`）。 */
const BARE_HSL_PATTERN = /^\d+(?:\.\d+)?\s+\d+(?:\.\d+)?%\s+\d+(?:\.\d+)?%$/;

// ---------------------------------------------------------------------------
// WCAG 相对亮度与对比度
// ---------------------------------------------------------------------------

function parseHslTriplet(value: string): {
  h: number;
  s: number;
  l: number;
} {
  const [hue, saturation, lightness] = value.trim().split(/\s+/);

  return {
    h: Number.parseFloat(hue),
    s: Number.parseFloat(saturation) / 100,
    l: Number.parseFloat(lightness) / 100,
  };
}

function hslToRgb(h: number, s: number, l: number): {
  r: number;
  g: number;
  b: number;
} {
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

function relativeLuminance(color: {
  r: number;
  g: number;
  b: number;
}): number {
  return 0.2126 * linearizeChannel(color.r)
    + 0.7152 * linearizeChannel(color.g)
    + 0.0722 * linearizeChannel(color.b);
}

function tokenToRgb(value: string): {
  r: number;
  g: number;
  b: number;
} {
  const { h, s, l } = parseHslTriplet(value);

  return hslToRgb(h, s, l);
}

function contrastRatio(foregroundToken: string, backgroundToken: string): number {
  const foreground = relativeLuminance(tokenToRgb(foregroundToken));
  const background = relativeLuminance(tokenToRgb(backgroundToken));

  const lighter = Math.max(foreground, background);
  const darker = Math.min(foreground, background);

  return (lighter + 0.05) / (darker + 0.05);
}

/**
 * 必须实算的配对。少了任何一条，就等于把某个真实场景的对比度交给运气。
 */
const CONTRAST_PAIRS = [
  {
    label: '正文 / 页面底色',
    foreground: '--foreground',
    background: '--background',
  },
  {
    label: '次级文字 / 页面底色',
    foreground: '--muted-foreground',
    background: '--background',
  },
  {
    label: '次级文字 / 次级底色',
    foreground: '--muted-foreground',
    background: '--background-2',
  },
  {
    label: '主按钮文字 / 主色',
    foreground: '--primary-foreground',
    background: '--primary',
  },
  {
    label: '点缀按钮文字 / 点缀色',
    foreground: '--accent-foreground',
    background: '--accent',
  },
  {
    label: '品牌文字 / 品牌色',
    foreground: '--brand-foreground',
    background: '--brand',
  },
  {
    label: '选中态文字 / 次级底色',
    foreground: '--accent-ink',
    background: '--background-2',
  },
  {
    label: '图标轨图标 / 图标轨底色',
    foreground: '--nav-rail-foreground',
    background: '--nav-rail-background',
  },
  {
    label: '图标轨盘符图标 / 图标轨底色',
    foreground: '--nav-rail-muted-foreground',
    background: '--nav-rail-background',
  },
] as const;

const MINIMUM_CONTRAST = 4.5;

/** C4：形状特征层只许碰颜色 / 形状 / 字体 / 阴影，出现布局属性就是改框架。 */
const FORBIDDEN_LAYOUT_PROPERTIES = [
  'width',
  'height',
  'min-width',
  'max-width',
  'min-height',
  'max-height',
  'margin',
  'margin-top',
  'margin-right',
  'margin-bottom',
  'margin-left',
  'padding',
  'padding-top',
  'padding-right',
  'padding-bottom',
  'padding-left',
  'top',
  'right',
  'bottom',
  'left',
  'inset',
  'gap',
  'row-gap',
  'column-gap',
  'flex',
  'flex-basis',
  'flex-grow',
  'flex-shrink',
  'order',
  'position',
  'display',
  'float',
  'clear',
  'transform',
  'translate',
  'zoom',
  'line-height',
  'grid-template',
  'grid-template-columns',
  'grid-template-rows',
  'grid-column',
  'grid-row',
];

describe('AFM brand redesign themes', () => {
  it('ships four themes', () => {
    expect(AFM_THEME_IDS).toHaveLength(4);
    expect(new Set(AFM_THEME_IDS).size).toBe(4);
    expect(AFM_THEMES).toHaveLength(4);
  });

  it('keeps every theme id in sync with the BuiltinThemeId union', () => {
    const declaredIds = Array.from(
      USER_SETTINGS_TS.matchAll(/'(afm-[a-z-]+)'/g),
      match => match[1],
    ).sort();

    expect(declaredIds).toEqual([...AFM_THEME_IDS].sort());

    const builtinIds: BuiltinThemeId[] = [
      'light',
      'dark',
      'system',
      'afm-warm-light',
      'afm-warm-dark',
      'afm-vivid-light',
      'afm-cool-light',
    ];

    for (const id of AFM_THEME_IDS) {
      expect(builtinIds).toContain(id);
      expect(isAfmThemeId(id)).toBe(true);
      expect(getAfmTheme(id)?.id).toBe(id);
    }
  });

  it('parses every theme id as a builtin theme', () => {
    for (const id of AFM_THEME_IDS) {
      expect(parseThemeId(id)).toEqual({
        source: 'builtin',
        builtinThemeId: id,
      });
    }
  });

  it('exposes every theme through the theme registry', () => {
    const availableIds = getAvailableThemeOptions({}).map(option => option.id);

    for (const id of AFM_THEME_IDS) {
      expect(availableIds).toContain(id);

      const option = findThemeOption(id, {});

      expect(option?.source).toBe('builtin');
      expect(['light', 'dark']).toContain(option?.baseTheme);
      expect(Object.keys(option?.variables ?? {}).length).toBeGreaterThan(0);

      if (!option || option.source !== 'builtin') {
        throw new Error(`主题 ${id} 没有出现在主题注册表里`);
      }

      // 形状特征层靠 `data-afm-theme` 生效，这里必须真的把 id 透出去。
      expect(option.cssThemeId).toBe(id);
    }
  });

  it('defines every color token so nothing leaks from the light base', () => {
    for (const theme of AFM_THEMES) {
      for (const token of REQUIRED_COLOR_TOKENS) {
        expect(theme.variables[token], `${theme.id} 缺少 ${token}`).toBeDefined();
      }
    }
  });

  it('keeps every color token a bare HSL triplet', () => {
    for (const theme of AFM_THEMES) {
      for (const token of REQUIRED_COLOR_TOKENS) {
        const value = theme.variables[token];

        expect(value, `${theme.id} 的 ${token} 未定义`).toBeDefined();
        expect(value, `${theme.id} 的 ${token} 不是裸 HSL 三元组：${value}`)
          .toMatch(BARE_HSL_PATTERN);
      }
    }
  });

  it('only lets --popover and --input carry alpha', () => {
    for (const theme of AFM_THEMES) {
      for (const token of REQUIRED_COLOR_TOKENS) {
        const value = theme.variables[token];

        if (!value.includes('/')) {
          continue;
        }

        expect(
          ALPHA_ALLOWED_TOKENS,
          `${theme.id} 的 ${token} 带了 alpha（${value}），会让 hsl(var(${token}) / N%) 整体失效`,
        ).toContain(token);
      }
    }
  });

  it('does not touch any size token', () => {
    const sizeTokens = [
      '--nav-sidebar-width',
      '--quick-access-panel-width',
      '--window-toolbar-height',
      '--action-toolbar-height',
      '--workspace-toolbar-height',
      '--navigator-list-view-entry-height',
      '--navigator-grid-view-dir-entry-height',
      '--navigator-grid-view-entry-height',
      '--navigator-grid-view-file-entry-height',
      '--tab-height',
      '--control-button-width',
    ];

    for (const theme of AFM_THEMES) {
      for (const token of sizeTokens) {
        expect(theme.variables[token as `--${string}`]).toBeUndefined();
      }
    }
  });

  it('keeps the nav rail at 42px', () => {
    expect(VARS_CSS).toMatch(/--nav-sidebar-width:\s*42px;/);
  });

  it('derives a valid radius ladder', () => {
    // --radius-sm 是 calc(var(--radius) - 4px)，--radius 小于 4px 会推出负值。
    for (const theme of AFM_THEMES) {
      const radius = theme.variables['--radius'];

      expect(radius).toBeDefined();

      const radiusInPx = Number.parseFloat(radius) * 16;

      expect(radiusInPx, `${theme.id} 的 --radius 会让 --radius-sm 变成负值`).toBeGreaterThanOrEqual(4);
    }
  });

  it('keeps every text-on-surface pair readable', () => {
    for (const theme of AFM_THEMES) {
      for (const pair of CONTRAST_PAIRS) {
        const foreground = theme.variables[pair.foreground];
        const background = theme.variables[pair.background];

        const ratio = contrastRatio(foreground, background);

        expect(
          ratio,
          `${theme.id}：${pair.label}（${pair.foreground} / ${pair.background}）只有 ${ratio.toFixed(2)}:1`,
        ).toBeGreaterThanOrEqual(MINIMUM_CONTRAST);
      }
    }
  });

  it('carries the brand colour in every theme', () => {
    for (const theme of AFM_THEMES) {
      expect(theme.variables['--brand']).toBeDefined();
      expect(theme.variables['--brand-foreground']).toBeDefined();

      // R2：品牌主色必须在样式表里真的被用上，不能只定义不落地。
      const scopedSelector = `html[data-afm-theme='${theme.cssThemeId}']`;

      expect(AFM_THEMES_CSS).toContain(scopedSelector);
      expect(AFM_THEMES_CSS).toContain('hsl(var(--brand))');
    }
  });

  it('offers a light and a dark warm-paper pair', () => {
    expect(getAfmTheme('afm-warm-light')?.baseTheme).toBe('light');
    expect(getAfmTheme('afm-warm-dark')?.baseTheme).toBe('dark');
    expect(getAfmTheme('afm-vivid-light')?.baseTheme).toBe('light');
  });

  it('scopes every shape rule to a theme id', () => {
    const selectors = AFM_THEMES_CSS
      .split('\n')
      .filter(line => line.trim().startsWith('html['));

    expect(selectors.length).toBeGreaterThan(0);

    for (const selector of selectors) {
      expect(selector).toMatch(/^html\[data-afm-theme='afm-[a-z-]+'\]/);
    }
  });

  it('never writes a layout property in the shape layer', () => {
    const declarations = AFM_THEMES_CSS
      .split('\n')
      .map(line => line.trim())
      .filter(line => line.length > 0 && !line.startsWith('/*') && !line.startsWith('*'));

    for (const declaration of declarations) {
      for (const property of FORBIDDEN_LAYOUT_PROPERTIES) {
        expect(
          declaration,
          `形状特征层出现了布局属性「${property}」：${declaration}`,
        ).not.toMatch(new RegExp(`^${property}\\s*:`));
      }
    }
  });

  it('imports the shape layer last so it can win on equal specificity', () => {
    const imports = INDEX_CSS
      .split('\n')
      .map(line => line.trim())
      .filter(line => line.startsWith('@import'));

    expect(imports[imports.length - 1]).toBe('@import "./afm-themes.css";');
  });
});
