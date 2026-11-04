// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type { ShortcutKeys } from '@/types/user-settings';
import {
  buildShortcutActionId,
  isShortcutActionId,
  parseShortcutActionId,
} from '@/core/actions/shortcut-actions';

/**
 * 持久化的一段绑定：单槽是一个键位组合，多槽是按槽位排列的数组（空槽为 null）。
 */
export type ShortcutBindingValue = ShortcutKeys | Array<ShortcutKeys | null>;

/** 绑定来自哪种持久化格式。 */
export type ShortcutBindingFormat = 'action-id' | 'legacy';

export type ShortcutBindingLookup = {
  entry: ShortcutBindingValue | undefined;
  format: ShortcutBindingFormat | null;
};

export function isShortcutKeys(value: unknown): value is ShortcutKeys {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    return false;
  }

  const candidate = value as Record<string, unknown>;

  if (typeof candidate.key !== 'string') {
    return false;
  }

  return ['ctrl', 'alt', 'shift', 'meta'].every(
    modifier => candidate[modifier] === undefined || typeof candidate[modifier] === 'boolean',
  );
}

export function isShortcutBindingValue(value: unknown): value is ShortcutBindingValue {
  if (Array.isArray(value)) {
    return value.every(item => item === null || isShortcutKeys(item));
  }

  return isShortcutKeys(value);
}

/**
 * 取出一个绑定里指定槽位的键位；该槽位没有绑定时返回 null。
 */
export function pickShortcutBindingSlot(
  entry: ShortcutBindingValue | undefined,
  bindingSlot: number,
): ShortcutKeys | null {
  if (entry === undefined) {
    return null;
  }

  if (!Array.isArray(entry)) {
    return bindingSlot === 0 ? entry : null;
  }

  return entry[bindingSlot] ?? null;
}

/**
 * 按「动作注册表 id 优先、旧格式兜底」的顺序查找一段绑定。
 *
 * 新格式命中时旧格式整条作废（同一个快捷键的槽位是一体的，不能按槽混用），
 * 否则用户刚解绑的槽位会被旧数据复活。
 */
export function lookupShortcutBinding(
  bindings: Record<string, unknown> | undefined,
  actionId: string,
  legacyShortcutId: string,
): ShortcutBindingLookup {
  if (!bindings) {
    return {
      entry: undefined,
      format: null,
    };
  }

  const actionEntry = bindings[actionId];

  if (isShortcutBindingValue(actionEntry)) {
    return {
      entry: actionEntry,
      format: 'action-id',
    };
  }

  const legacyEntry = bindings[legacyShortcutId];

  if (isShortcutBindingValue(legacyEntry)) {
    return {
      entry: legacyEntry,
      format: 'legacy',
    };
  }

  return {
    entry: undefined,
    format: null,
  };
}

export type ShortcutBindingMigration = {
  /** 迁移后的绑定表，键统一是动作注册表 id。 */
  bindings: Record<string, ShortcutBindingValue>;
  /** 每个动作 id 的绑定原本来自哪种格式。 */
  formats: Record<string, ShortcutBindingFormat>;
  /** 旧格式里存在、但当前没有对应快捷键的键：原样保留，不丢用户数据。 */
  unmappedLegacyKeys: string[];
  /** 值不是合法绑定的键：视为未绑定，回落到默认键位。 */
  invalidKeys: string[];
};

/**
 * 把旧格式（键是 `ShortcutId`）的绑定表映射成以动作注册表 id 为键的新表。
 *
 * 只做内存映射，不改写用户文件；写盘由 store 在用户真正改动绑定时完成。
 */
export function migrateShortcutBindings(
  rawBindings: Record<string, unknown> | undefined,
  isKnownShortcutId: (shortcutId: string) => boolean,
): ShortcutBindingMigration {
  const bindings: Record<string, ShortcutBindingValue> = {};
  const formats: Record<string, ShortcutBindingFormat> = {};
  const unmappedLegacyKeys: string[] = [];
  const invalidKeys: string[] = [];

  if (!rawBindings) {
    return {
      bindings,
      formats,
      unmappedLegacyKeys,
      invalidKeys,
    };
  }

  const entries = Object.entries(rawBindings);

  // 第一遍：新格式优先占位
  for (const [key, value] of entries) {
    if (!isShortcutActionId(key)) continue;

    if (parseShortcutActionId(key) === null) {
      invalidKeys.push(key);
      continue;
    }

    if (isShortcutBindingValue(value)) {
      bindings[key] = value;
      formats[key] = 'action-id';
    }
    else {
      invalidKeys.push(key);
    }
  }

  // 第二遍：旧格式只填补新格式没覆盖到的快捷键
  for (const [key, value] of entries) {
    if (isShortcutActionId(key)) continue;

    if (!isKnownShortcutId(key)) {
      unmappedLegacyKeys.push(key);
      continue;
    }

    if (!isShortcutBindingValue(value)) {
      invalidKeys.push(key);
      continue;
    }

    const actionId = buildShortcutActionId(key);

    if (bindings[actionId] === undefined) {
      bindings[actionId] = value;
      formats[actionId] = 'legacy';
    }
  }

  return {
    bindings,
    formats,
    unmappedLegacyKeys,
    invalidKeys,
  };
}
