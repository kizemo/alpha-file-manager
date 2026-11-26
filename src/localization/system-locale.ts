// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { AppLocale } from './data';

/**
 * Fallback used when the system language has no matching language pack.
 * English on purpose: it is the one pack every install can be assumed to
 * have, and it is the locale the app already defaulted to.
 */
export const FALLBACK_APP_LOCALE: AppLocale = 'en';

/**
 * Exact matches against a normalized (lowercase, `-`-separated) system locale.
 *
 * Chinese is deliberately matched by full tag instead of by primary language:
 * the app ships a **simplified** Chinese pack under the internal locale code
 * `ch`, so routing `zh-TW` / `zh-HK` / `zh-Hant` to it would hand those users
 * a mixed simplified/traditional UI. Those tags are intentionally absent here
 * and get caught by the `zh` special case in {@link resolveAppLocaleFromSystem}.
 */
const EXACT_SYSTEM_LOCALES: Record<string, AppLocale> = {
  // Simplified Chinese only. Traditional variants intentionally absent.
  'zh': 'ch',
  'zh-cn': 'ch',
  'zh-sg': 'ch',
  'zh-hans': 'ch',

  // Everything else is matched by primary language below.
  'en': 'en',
  'ru': 'ru',
  'es': 'es',
  'de': 'de',
  'fr': 'fr',
  'tr': 'tr',
  'ja': 'ja',
  'fa': 'fa',
  'vi': 'vi',
  'it': 'it',
  'pt': 'pt',
  'sl': 'sl',
  'hi': 'hi',
  'ur': 'ur',
  'he': 'he',
  'hy': 'hy',
};

/**
 * Region subtags that identify a traditional-Chinese locale on their own.
 * `hant` is handled separately because it can appear in any position
 * (`zh-hant`, `zh-hant-tw`).
 */
const TRADITIONAL_CHINESE_REGIONS = new Set(['tw', 'hk', 'mo']);

/**
 * Map a system locale (BCP-47 tag such as `zh-CN`, `pt-BR`, `ko-KR`) onto one
 * of the app's own locale codes.
 *
 * Resolution order:
 *   1. exact match on the whole normalized tag
 *   2. `zh-*` that is not traditional → simplified pack
 *   3. `zh-*` that is traditional → fallback
 *   4. match on the primary language subtag (`en-us` → `en`)
 *   5. fallback
 */
export function resolveAppLocaleFromSystem(
  systemLocale: string | null | undefined,
): AppLocale {
  const normalized = (systemLocale ?? '')
    .trim()
    .toLowerCase()
    .replace(/_/g, '-');

  if (!normalized) {
    return FALLBACK_APP_LOCALE;
  }

  const exact = EXACT_SYSTEM_LOCALES[normalized];

  if (exact) {
    return exact;
  }

  // Drop empty subtags so malformed input like `zh-` degrades to bare `zh`
  // rather than indexing the map with an empty string.
  const subtags = normalized.split('-').filter(Boolean);
  const primaryLanguage = subtags[0] ?? '';

  if (primaryLanguage === 'zh') {
    // The simplified tags (`zh`, `zh-cn`, `zh-sg`, `zh-hans`) were already
    // returned by the exact match above. Anything left is either explicitly
    // traditional (`zh-hant`, `zh-hant-tw`) or carries a traditional region
    // (`zh-tw`, `zh-hk`, `zh-mo`). Route all of them to the fallback rather
    // than to the simplified pack.
    const isTraditional = subtags.includes('hant')
      || TRADITIONAL_CHINESE_REGIONS.has(subtags[1] ?? '');

    return isTraditional ? FALLBACK_APP_LOCALE : 'ch';
  }

  return EXACT_SYSTEM_LOCALES[primaryLanguage] ?? FALLBACK_APP_LOCALE;
}
