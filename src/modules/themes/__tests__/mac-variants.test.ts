// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION 2026-10-29（AFM 3.6.5）：内置 macOS Finder 风格主题变体的单测。

import { describe, expect, it } from 'vitest';
import {
  getMacVariantTheme,
  isMacVariantThemeId,
  MAC_VARIANT_THEMES,
  MAC_VARIANT_THEME_IDS,
} from '@/modules/themes/mac-variants';
import {
  findThemeOption,
  getAvailableThemeOptions,
  parseThemeId,
} from '@/modules/themes/registry';
import type { BuiltinThemeId } from '@/types/user-settings';

/**
 * 全仓有大量 `hsl(var(--token) / N%)` 的「二次加 alpha」写法。
 * 这些 token 自己不能再带 alpha，否则 `hsl(240 30% 99% / 78% / 80%)`
 * 是非法值，声明会被浏览器整体丢弃。
 */
const TOKENS_RE_ALPHA_ISED = [
  '--background',
  '--background-2',
  '--background-3',
  '--window-toolbar',
  '--icon',
  '--foreground',
  '--muted',
  '--muted-foreground',
  '--popover-foreground',
  '--card',
  '--card-foreground',
  '--border',
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

/** 深浅两套都必须覆盖的颜色 token（漏一个就会从 :root 继承浅色值）。 */
const REQUIRED_COLOR_TOKENS = [
  ...TOKENS_RE_ALPHA_ISED,
  '--popover',
  '--input',
] as const;

describe('macOS Finder theme variants', () => {
  it('ships six variants: three families times light and dark', () => {
    expect(MAC_VARIANT_THEME_IDS).toHaveLength(6);
    expect(new Set(MAC_VARIANT_THEME_IDS).size).toBe(6);
    expect(MAC_VARIANT_THEMES).toHaveLength(6);
  });

  it('keeps every variant id in sync with the BuiltinThemeId union', () => {
    const builtinIds: BuiltinThemeId[] = [
      'light',
      'dark',
      'system',
      'mac-classic-light',
      'mac-classic-dark',
      'mac-modern-light',
      'mac-modern-dark',
      'mac-glass-light',
      'mac-glass-dark',
    ];

    for (const id of MAC_VARIANT_THEME_IDS) {
      expect(builtinIds).toContain(id);
      expect(isMacVariantThemeId(id)).toBe(true);
    }
  });

  it('parses every variant id as a builtin theme', () => {
    for (const id of MAC_VARIANT_THEME_IDS) {
      expect(parseThemeId(id)).toEqual({
        source: 'builtin',
        builtinThemeId: id,
      });
    }
  });

  it('exposes every variant through the theme registry', () => {
    const availableIds = getAvailableThemeOptions({}).map(option => option.id);

    for (const id of MAC_VARIANT_THEME_IDS) {
      expect(availableIds).toContain(id);

      const option = findThemeOption(id, {});

      expect(option?.source).toBe('builtin');
      expect(['light', 'dark']).toContain(option?.baseTheme);
      expect(Object.keys(option?.variables ?? {}).length).toBeGreaterThan(0);
    }
  });

  it('pairs each family with one light and one dark variant', () => {
    const lightIds = MAC_VARIANT_THEME_IDS.filter(id => id.endsWith('-light'));
    const darkIds = MAC_VARIANT_THEME_IDS.filter(id => id.endsWith('-dark'));

    expect(lightIds).toHaveLength(3);
    expect(darkIds).toHaveLength(3);

    for (const id of MAC_VARIANT_THEME_IDS) {
      const theme = MAC_VARIANT_THEMES.find(item => item.id === id);

      expect(theme).toBeDefined();
      expect(['light', 'dark']).toContain(theme?.baseTheme);
    }

    for (const id of lightIds) {
      expect(getMacVariantTheme(id)?.baseTheme).toBe('light');
    }

    for (const id of darkIds) {
      expect(getMacVariantTheme(id)?.baseTheme).toBe('dark');
    }
  });

  it('defines every color token so nothing leaks from the light base', () => {
    for (const theme of MAC_VARIANT_THEMES) {
      for (const token of REQUIRED_COLOR_TOKENS) {
        expect(theme.variables[token]).toBeDefined();
      }
    }
  });

  it('never puts alpha into tokens the codebase re-alpha-izes', () => {
    for (const theme of MAC_VARIANT_THEMES) {
      for (const token of TOKENS_RE_ALPHA_ISED) {
        const value = theme.variables[token];

        expect(value).toBeDefined();
        expect(value).not.toMatch(/\//);
      }
    }
  });

  it('only lets --popover and --input carry alpha', () => {
    for (const theme of MAC_VARIANT_THEMES) {
      const alphaTokens = Object.entries(theme.variables)
        .filter(([token, value]) => REQUIRED_COLOR_TOKENS.includes(token as typeof REQUIRED_COLOR_TOKENS[number])
          && value.includes('/'))
        .map(([token]) => token);

      for (const token of alphaTokens) {
        expect(['--popover', '--input']).toContain(token);
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

    for (const theme of MAC_VARIANT_THEMES) {
      for (const token of sizeTokens) {
        expect(theme.variables[token as `--${string}`]).toBeUndefined();
      }
    }
  });
});
