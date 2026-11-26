// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { intlLocaleMapping } from '@/localization/data';

/**
 * 扩展包内的语言包目录与默认语言。
 *
 * 约定：扩展作者把语言包放在 `locales/<BCP-47>.json`，例如 `locales/zh-CN.json`。
 * 这与 AFM 自身的 locale code（`ch`、`he`、`hy`…）不是同一套编码，必须通过
 * `intlLocaleMapping` 做映射，否则拼出来的文件名永远不存在。
 */
export const EXTENSION_LOCALE_DIR = 'locales';

export const EXTENSION_DEFAULT_LOCALE = 'en';

const INTL_LOCALE_BY_APP_LOCALE: Record<string, string> = intlLocaleMapping;

/**
 * 把 AFM 的 locale code 展开成扩展语言包可用的 BCP-47 候选列表。
 *
 * 顺序：BCP-47 形态（`zh-CN`）优先，再退回 AFM 原始 code（`ch`），去重。
 * AFM 实际的 17 个语言包里，两者相同的条目只会留下一项。
 */
export function getExtensionLocaleCandidates(appLocale: string | null | undefined): string[] {
  const raw = typeof appLocale === 'string' ? appLocale.trim() : '';

  if (!raw) {
    return [];
  }

  const intl = INTL_LOCALE_BY_APP_LOCALE[raw];
  const candidates: string[] = [];

  for (const candidate of [intl, raw]) {
    if (candidate && !candidates.includes(candidate)) {
      candidates.push(candidate);
    }
  }

  return candidates;
}

/** 候选语言包相对扩展根目录的路径，例如 `locales/zh-CN.json`。 */
export function getExtensionLocaleFileNames(appLocale: string | null | undefined): string[] {
  return getExtensionLocaleCandidates(appLocale).map(code => `${EXTENSION_LOCALE_DIR}/${code}.json`);
}

/**
 * 当前界面语言是否需要额外取一次扩展语言包。
 *
 * `en` 直接短路：市场清单里的 `description` 本来就是英文，再请求一次没有意义。
 */
export function isExtensionLocalizationNeeded(appLocale: string | null | undefined): boolean {
  return getExtensionLocaleCandidates(appLocale).some(
    code => code.toLowerCase() !== EXTENSION_DEFAULT_LOCALE,
  );
}

/** 宽进严出地解析语言包；非法 JSON、数组、标量一律视为无效。 */
export function parseExtensionLocaleMessages(
  text: string | null | undefined,
): Record<string, unknown> | null {
  if (typeof text !== 'string' || text.trim() === '') {
    return null;
  }

  try {
    const parsed: unknown = JSON.parse(text);

    if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
      return null;
    }

    return parsed as Record<string, unknown>;
  }
  catch {
    return null;
  }
}

/**
 * 从语言包里取扩展说明文字。
 *
 * 扩展包实际用的是 `extensionDescription`（与 `extensionName` 成对），部分扩展可能
 * 只写 `description`，两者都认，取第一个非空字符串。
 */
export function extractExtensionDescription(
  messages: Record<string, unknown> | null | undefined,
): string | null {
  if (!messages) {
    return null;
  }

  for (const key of ['extensionDescription', 'description']) {
    const value = messages[key];

    if (typeof value === 'string' && value.trim() !== '') {
      return value.trim();
    }
  }

  return null;
}
