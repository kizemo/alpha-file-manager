// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  FALLBACK_APP_LOCALE,
  resolveAppLocaleFromSystem,
} from '@/localization/system-locale';

describe('resolveAppLocaleFromSystem', () => {
  it('maps simplified Chinese system tags to the internal `ch` locale', () => {
    // The app's Chinese pack uses the internal code `ch`, NOT `zh`/`zh-CN`.
    // Getting this wrong yields an unresolvable locale at runtime.
    expect(resolveAppLocaleFromSystem('zh-CN')).toBe('ch');
    expect(resolveAppLocaleFromSystem('zh')).toBe('ch');
    expect(resolveAppLocaleFromSystem('zh-SG')).toBe('ch');
    expect(resolveAppLocaleFromSystem('zh-Hans')).toBe('ch');
  });

  it('falls back to English for traditional Chinese tags', () => {
    // These must NOT reach the simplified pack: a mixed simplified/traditional
    // UI is worse than falling back to English.
    expect(resolveAppLocaleFromSystem('zh-TW')).toBe('en');
    expect(resolveAppLocaleFromSystem('zh-HK')).toBe('en');
    expect(resolveAppLocaleFromSystem('zh-MO')).toBe('en');
    expect(resolveAppLocaleFromSystem('zh-Hant')).toBe('en');
    expect(resolveAppLocaleFromSystem('zh-Hant-TW')).toBe('en');
  });

  it('still maps simplified tags that carry an extra region subtag', () => {
    // `hant` in any position marks traditional; its absence does not.
    expect(resolveAppLocaleFromSystem('zh-Hans-CN')).toBe('ch');
    expect(resolveAppLocaleFromSystem('zh-CN-Hans')).toBe('ch');
  });

  it('is case insensitive and tolerates underscore separators', () => {
    expect(resolveAppLocaleFromSystem('ZH-cn')).toBe('ch');
    expect(resolveAppLocaleFromSystem('EN-us')).toBe('en');
    expect(resolveAppLocaleFromSystem('pt_BR')).toBe('pt');
  });

  it('matches other languages by primary language subtag', () => {
    expect(resolveAppLocaleFromSystem('en-GB')).toBe('en');
    expect(resolveAppLocaleFromSystem('pt-BR')).toBe('pt');
    expect(resolveAppLocaleFromSystem('de-AT')).toBe('de');
    expect(resolveAppLocaleFromSystem('ja-JP')).toBe('ja');
    expect(resolveAppLocaleFromSystem('hy-AM')).toBe('hy');
    expect(resolveAppLocaleFromSystem('hi-IN')).toBe('hi');
  });

  it('resolves right-to-left languages', () => {
    expect(resolveAppLocaleFromSystem('fa-IR')).toBe('fa');
    expect(resolveAppLocaleFromSystem('ur-PK')).toBe('ur');
    expect(resolveAppLocaleFromSystem('he-IL')).toBe('he');
  });

  it('falls back to English for languages without a language pack', () => {
    expect(resolveAppLocaleFromSystem('ko-KR')).toBe('en');
    expect(resolveAppLocaleFromSystem('nl-NL')).toBe('en');
    expect(resolveAppLocaleFromSystem('pl-PL')).toBe('en');
    expect(resolveAppLocaleFromSystem('uk-UA')).toBe('en');
  });

  it('falls back to English for empty and malformed input', () => {
    expect(resolveAppLocaleFromSystem('')).toBe(FALLBACK_APP_LOCALE);
    expect(resolveAppLocaleFromSystem('   ')).toBe(FALLBACK_APP_LOCALE);
    expect(resolveAppLocaleFromSystem('-')).toBe(FALLBACK_APP_LOCALE);
    expect(resolveAppLocaleFromSystem(null)).toBe(FALLBACK_APP_LOCALE);
    expect(resolveAppLocaleFromSystem(undefined)).toBe(FALLBACK_APP_LOCALE);
  });

  it('degrades a trailing-separator Chinese tag to bare zh rather than crashing', () => {
    // `zh-` normalizes to the same subtags as bare `zh`.
    expect(resolveAppLocaleFromSystem('zh-')).toBe('ch');
  });
});
