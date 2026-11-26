// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  EXTENSION_DEFAULT_LOCALE,
  EXTENSION_LOCALE_DIR,
  extractExtensionDescription,
  getExtensionLocaleCandidates,
  getExtensionLocaleFileNames,
  isExtensionLocalizationNeeded,
  parseExtensionLocaleMessages,
} from '@/modules/extensions/utils/extension-locale';

describe('getExtensionLocaleCandidates', () => {
  it('maps the AFM chinese locale code to BCP-47 first', () => {
    expect(getExtensionLocaleCandidates('ch')).toEqual(['zh-CN', 'ch']);
  });

  it('maps the regional AFM locale codes to BCP-47 first', () => {
    expect(getExtensionLocaleCandidates('he')).toEqual(['he-IL', 'he']);
    expect(getExtensionLocaleCandidates('hy')).toEqual(['hy-AM', 'hy']);
    expect(getExtensionLocaleCandidates('hi')).toEqual(['hi-IN', 'hi']);
    expect(getExtensionLocaleCandidates('ur')).toEqual(['ur-PK', 'ur']);
  });

  it('keeps identical codes once', () => {
    expect(getExtensionLocaleCandidates('en')).toEqual(['en']);
    expect(getExtensionLocaleCandidates('de')).toEqual(['de']);
    expect(getExtensionLocaleCandidates('ja')).toEqual(['ja']);
  });

  it('returns nothing for empty or missing locales', () => {
    expect(getExtensionLocaleCandidates('')).toEqual([]);
    expect(getExtensionLocaleCandidates('   ')).toEqual([]);
    expect(getExtensionLocaleCandidates(null)).toEqual([]);
    expect(getExtensionLocaleCandidates(undefined)).toEqual([]);
  });

  it('passes unknown locale codes through unchanged', () => {
    expect(getExtensionLocaleCandidates('xx-YY')).toEqual(['xx-YY']);
  });
});

describe('getExtensionLocaleFileNames', () => {
  it('points at the locales directory with the BCP-47 name first', () => {
    expect(getExtensionLocaleFileNames('ch')).toEqual([
      `${EXTENSION_LOCALE_DIR}/zh-CN.json`,
      `${EXTENSION_LOCALE_DIR}/ch.json`,
    ]);
  });

  it('returns no candidates when the locale is empty', () => {
    expect(getExtensionLocaleFileNames('')).toEqual([]);
  });
});

describe('isExtensionLocalizationNeeded', () => {
  it('skips the default english locale', () => {
    expect(EXTENSION_DEFAULT_LOCALE).toBe('en');
    expect(isExtensionLocalizationNeeded('en')).toBe(false);
  });

  it('requests localization for every non-english locale', () => {
    expect(isExtensionLocalizationNeeded('ch')).toBe(true);
    expect(isExtensionLocalizationNeeded('de')).toBe(true);
    expect(isExtensionLocalizationNeeded('he')).toBe(true);
  });

  it('skips when no locale is set', () => {
    expect(isExtensionLocalizationNeeded('')).toBe(false);
    expect(isExtensionLocalizationNeeded(null)).toBe(false);
  });
});

describe('parseExtensionLocaleMessages', () => {
  it('parses a flat locale object', () => {
    expect(parseExtensionLocaleMessages('{"extensionDescription":"你好"}')).toEqual({
      extensionDescription: '你好',
    });
  });

  it('rejects invalid json, arrays and scalars', () => {
    expect(parseExtensionLocaleMessages('{')).toBeNull();
    expect(parseExtensionLocaleMessages('[]')).toBeNull();
    expect(parseExtensionLocaleMessages('"text"')).toBeNull();
    expect(parseExtensionLocaleMessages('42')).toBeNull();
    expect(parseExtensionLocaleMessages('null')).toBeNull();
    expect(parseExtensionLocaleMessages('')).toBeNull();
    expect(parseExtensionLocaleMessages(null)).toBeNull();
  });
});

describe('extractExtensionDescription', () => {
  it('prefers extensionDescription over description', () => {
    expect(extractExtensionDescription({
      extensionDescription: '中文说明',
      description: 'english description',
    })).toBe('中文说明');
  });

  it('falls back to description', () => {
    expect(extractExtensionDescription({ description: 'english description' })).toBe(
      'english description',
    );
  });

  it('ignores blank and non-string values', () => {
    expect(extractExtensionDescription({ extensionDescription: '   ' })).toBeNull();
    expect(extractExtensionDescription({ extensionDescription: 42 })).toBeNull();
    expect(extractExtensionDescription({ extensionDescription: null })).toBeNull();
    expect(extractExtensionDescription({})).toBeNull();
    expect(extractExtensionDescription(null)).toBeNull();
  });

  it('trims the resolved description', () => {
    expect(extractExtensionDescription({ extensionDescription: '  中文说明  ' })).toBe('中文说明');
  });
});
