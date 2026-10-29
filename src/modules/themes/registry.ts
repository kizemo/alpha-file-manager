// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { InstalledExtensionData } from '@/types/extension';
import type { BuiltinThemeId, Theme } from '@/types/user-settings';
// FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题。
import {
  isAfmThemeId,
  AFM_THEMES,
} from '@/modules/themes/afm-themes';

export const BUILTIN_THEME_IDS = ['dark', 'system'] as const;
export const DEFAULT_THEME_ID: BuiltinThemeId = 'afm-warm-light';

// FORK-MODIFICATION 2026-10-29：原来这里是 `Exclude<BuiltinThemeId, 'system'>`，
// BuiltinThemeId 扩出 afm-* 变体 id 之后，那个排除会把变体 id 也算进来，
// 而扩展主题的 baseTheme 只允许 'light' / 'dark'，所以这里必须写死。
type ResolvedBuiltinThemeId = 'light' | 'dark';

type BuiltinThemeOption = {
  id: BuiltinThemeId;
  source: 'builtin';
  builtinThemeId: BuiltinThemeId;
  /**
   * FORK-MODIFICATION 2026-10-29：内置变体主题（AFM 品牌三套）的 CSS 变量覆盖。
   * 经典主题（dark / system）没有这个字段；`use-theme.ts` 用它区分两条应用路径。
   */
  baseTheme?: 'light' | 'dark';
  variables?: Record<`--${string}`, string>;
  /**
   * FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：写入 `<html data-afm-theme>` 的值。
   * 少量 token 表达不了的形状特征靠它做作用域覆盖（见 `src/styles/afm-themes.css`）。
   */
  cssThemeId?: string;
};

type ExtensionThemeOption = {
  id: Theme;
  source: 'extension';
  extensionId: string;
  themeId: string;
  title: string;
  description?: string;
  baseTheme: ResolvedBuiltinThemeId;
  variables: Record<`--${string}`, string>;
};

export type ThemeOption = BuiltinThemeOption | ExtensionThemeOption;

type ParsedBuiltinThemeId = {
  source: 'builtin';
  builtinThemeId: BuiltinThemeId;
};

type ParsedExtensionThemeId = {
  source: 'extension';
  extensionId: string;
  themeId: string;
};

type ParsedThemeId = ParsedBuiltinThemeId | ParsedExtensionThemeId;

const EXTENSION_THEME_PREFIX = 'extension:';
const EXTENSION_THEME_ID_PATTERN = /^[a-z0-9-]+$/;

const CLASSIC_BUILTIN_THEME_IDS: readonly string[] = BUILTIN_THEME_IDS;

export function createExtensionThemeId(extensionId: string, themeId: string): Theme {
  return `${EXTENSION_THEME_PREFIX}${extensionId}:${themeId}` as Theme;
}

export function parseThemeId(theme: string): ParsedThemeId | null {
  if (CLASSIC_BUILTIN_THEME_IDS.includes(theme)) {
    return {
      source: 'builtin',
      builtinThemeId: theme as BuiltinThemeId,
    };
  }

  // FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题同样走 builtin 通道。
  if (isAfmThemeId(theme)) {
    return {
      source: 'builtin',
      builtinThemeId: theme,
    };
  }

  if (!theme.startsWith(EXTENSION_THEME_PREFIX)) {
    return null;
  }

  const identifier = theme.slice(EXTENSION_THEME_PREFIX.length);
  const separatorIndex = identifier.indexOf(':');

  if (separatorIndex <= 0 || separatorIndex === identifier.length - 1) {
    return null;
  }

  const extensionId = identifier.slice(0, separatorIndex);
  const themeId = identifier.slice(separatorIndex + 1);

  if (!EXTENSION_THEME_ID_PATTERN.test(themeId)) {
    return null;
  }

  return {
    source: 'extension',
    extensionId,
    themeId,
  };
}

export function getBuiltinThemeOptions(): ThemeOption[] {
  return BUILTIN_THEME_IDS.map(themeId => ({
    id: themeId,
    source: 'builtin' as const,
    builtinThemeId: themeId,
  }));
}

/**
 * FORK-MODIFICATION 2026-10-29（AFM UI 重设计）：三套品牌重设计主题。
 * 与扩展主题同构（基础明暗 + CSS 变量覆盖），额外带 `cssThemeId`，
 * 供 `use-theme.ts` 写到 `<html data-afm-theme>` 上做形状特征的作用域覆盖。
 */
export function getAfmThemeOptions(): ThemeOption[] {
  return AFM_THEMES.map(theme => ({
    id: theme.id,
    source: 'builtin' as const,
    builtinThemeId: theme.id,
    baseTheme: theme.baseTheme,
    variables: theme.variables,
    cssThemeId: theme.cssThemeId,
  }));
}

export function getExtensionThemeOptions(
  installedExtensions: Record<string, InstalledExtensionData>,
): ThemeOption[] {
  const themeOptions: ThemeOption[] = [];

  for (const [extensionId, extension] of Object.entries(installedExtensions)) {
    if (!extension.enabled || extension.installPendingDependencies) {
      continue;
    }

    const contributedThemes = extension.manifest.contributes?.themes ?? [];

    for (const theme of contributedThemes) {
      themeOptions.push({
        id: createExtensionThemeId(extensionId, theme.id),
        source: 'extension',
        extensionId,
        themeId: theme.id,
        title: theme.title,
        description: theme.description,
        baseTheme: theme.baseTheme,
        variables: theme.variables,
      });
    }
  }

  return themeOptions.sort((left, right) => {
    const extensionCompare = left.source === 'extension' && right.source === 'extension'
      ? left.extensionId.localeCompare(right.extensionId)
      : 0;

    if (extensionCompare !== 0) {
      return extensionCompare;
    }

    if (left.source === 'extension' && right.source === 'extension') {
      return left.title.localeCompare(right.title);
    }

    return 0;
  });
}

export function getAvailableThemeOptions(
  installedExtensions: Record<string, InstalledExtensionData>,
): ThemeOption[] {
  return [
    ...getBuiltinThemeOptions(),
    ...getAfmThemeOptions(),
    ...getExtensionThemeOptions(installedExtensions),
  ];
}

export function findThemeOption(
  theme: string,
  installedExtensions: Record<string, InstalledExtensionData>,
): ThemeOption | undefined {
  const parsedTheme = parseThemeId(theme);

  if (!parsedTheme) {
    return undefined;
  }

  if (parsedTheme.source === 'builtin') {
    const builtinOption = getBuiltinThemeOptions()
      .find(option => option.id === parsedTheme.builtinThemeId);

    if (builtinOption) {
      return builtinOption;
    }

    return getAfmThemeOptions()
      .find(option => option.id === parsedTheme.builtinThemeId);
  }

  return getExtensionThemeOptions(installedExtensions).find((option) => {
    return option.source === 'extension'
      && option.extensionId === parsedTheme.extensionId
      && option.themeId === parsedTheme.themeId;
  });
}

export function normalizeThemeSelection(
  theme: string,
  installedExtensions: Record<string, InstalledExtensionData>,
): Theme {
  const selectedTheme = findThemeOption(theme, installedExtensions);
  return (selectedTheme?.id ?? DEFAULT_THEME_ID) as Theme;
}
