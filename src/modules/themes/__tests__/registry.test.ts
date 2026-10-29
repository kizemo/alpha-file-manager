// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import type { InstalledExtensionData } from '@/types/extension';
import {
  createExtensionThemeId,
  DEFAULT_THEME_ID,
  getAvailableThemeOptions,
  normalizeThemeSelection,
  parseThemeId,
} from '@/modules/themes/registry';

function createInstalledExtensionData(
  overrides: Partial<InstalledExtensionData> = {},
): InstalledExtensionData {
  return {
    version: '1.0.0',
    enabled: true,
    autoUpdate: true,
    installedAt: 1,
    manifest: {
      id: 'test.palette',
      name: 'Test Palette',
      version: '1.0.0',
      repository: 'https://github.com/example/test-palette',
      license: 'MIT',
      extensionType: 'api',
      main: 'index.js',
      permissions: [],
      contributes: {
        themes: [
          {
            id: 'midnight',
            title: 'Midnight',
            baseTheme: 'dark',
            variables: {
              '--background': '230 20% 10%',
            },
          },
        ],
      },
      engines: {
        sigmaFileManager: '>=2.0.0',
      },
    },
    settings: {
      scopedDirectories: [],
      customSettings: {},
    },
    ...overrides,
  };
}

describe('theme registry', () => {
  it('includes built-in themes and enabled extension themes', () => {
    const availableThemes = getAvailableThemeOptions({
      'test.palette': createInstalledExtensionData(),
      'test.disabled': createInstalledExtensionData({
        enabled: false,
        manifest: {
          ...createInstalledExtensionData().manifest,
          id: 'test.disabled',
          contributes: {
            themes: [
              {
                id: 'sunrise',
                title: 'Sunrise',
                baseTheme: 'light',
                variables: {
                  '--background': '30 100% 98%',
                },
              },
            ],
          },
        },
      }),
    });

    // FORK-MODIFICATION 2026-10-30：六套 macOS Finder 风格主题已删除，
    // 内置 'light'（亮色·开发中）已从可选主题下架，'light' 仅保留在类型与 CSS 基座里
    // 供 ResolvedBuiltinThemeId / 扩展主题 baseTheme 使用。默认主题改为 afm-warm-light。
    expect(availableThemes.map(theme => theme.id)).toEqual([
      'dark',
      'system',
      'afm-warm-light',
      'afm-warm-dark',
      'afm-vivid-light',
      'extension:test.palette:midnight',
    ]);
  });

  it('normalizes missing extension themes back to the default theme', () => {
    expect(normalizeThemeSelection('extension:test.palette:missing', {})).toBe(DEFAULT_THEME_ID);
  });

  it('parses extension theme ids', () => {
    expect(parseThemeId(createExtensionThemeId('test.palette', 'midnight'))).toEqual({
      source: 'extension',
      extensionId: 'test.palette',
      themeId: 'midnight',
    });
  });
});
