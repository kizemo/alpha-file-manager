// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 58（2026-11-25）——「导出设置 / 导入设置」的数据格式。
//
// 导出文件是一份 JSON，形状固定：
//
// {
//   format: 'alpha-file-manager-settings',
//   version: 1,
//   exportedAt: '2026-11-25T…',
//   settings: { 'navigator.enableBoxSelection': true, … },   ← 与盘上 user-settings.json 同形
//   favorites: [ … ],
//   favoriteCategories: [ … ],
//   tags: [ … ],
//   taggedItems: [ … ],
//   remoteConnections: [ … ]
// }
//
// 三个刻意的取舍：
//
// 1. **history 与 frequentItems 不导出。** 它们是「使用痕迹」不是「配置」，
//    换台机器时把访问历史一起搬过去既没意义又占地方。
// 2. **凭据不导出。** `credentialKey` 指向本机 DPAPI 密文，proxy 的 password
//    同理：跨机无效，而且属于敏感信息。导出时一律剔除；导入时若本机已有
//    同 id 的连接，则沿用本机凭据（见 mergeImportedRemoteConnections）。
// 3. **settings 用盘上的扁平点号键。** 导入端要按白名单过滤再写回，
//    扁平键与白名单的形态一致，不必在两端各写一套展开 / 折叠逻辑。

import type {
  FavoriteCategory,
  FavoriteItem,
  ItemTag,
  TaggedItem,
} from '@/types/user-stats';
import type { RemoteConnection } from '@/types/remote';

export const SETTINGS_TRANSFER_FORMAT = 'alpha-file-manager-settings';
export const SETTINGS_TRANSFER_VERSION = 1;

/** settings 里的 schema 版本键：它是存储格式的版本，不是用户设置。 */
export const SETTINGS_SCHEMA_VERSION_KEY = '__schemaVersion';

export interface SettingsTransferBundle {
  format: string;
  version: number;
  exportedAt: string;
  settings: Record<string, unknown>;
  favorites: FavoriteItem[];
  favoriteCategories: FavoriteCategory[];
  tags: ItemTag[];
  taggedItems: TaggedItem[];
  remoteConnections: RemoteConnection[];
}

/** 导出时的三个数据源（盘上三份 JSON 的解析结果）。 */
export interface SettingsTransferSource {
  settings?: unknown;
  stats?: unknown;
  remoteConnections?: unknown;
}

export type SettingsTransferParseFailure = 'format' | 'version' | 'structure';

export type SettingsTransferParseResult
  = | {
    ok: true;
    bundle: SettingsTransferBundle;
  }
  | {
    ok: false;
    reason: SettingsTransferParseFailure;
  };

export interface MergedRemoteConnections {
  connections: RemoteConnection[];
  /** 本机没有对应凭据、需要重新登录的连接 id。 */
  needsCredential: string[];
}

function isPlainObject(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

/**
 * 归一化设置集合：去掉 schema 版本键与 undefined 值。
 *
 * 其余键原样保留 —— 合法性由导入端的**白名单**把关（`applyUserSettingsEntries`），
 * 导出端不做白名单判断：导出的内容应当忠实反映盘上状态，
 * 而不是「导出时就已经悄悄丢了几项」。
 */
export function normalizeSettingsForExport(value: unknown): Record<string, unknown> {
  if (!isPlainObject(value)) {
    return {};
  }

  const result: Record<string, unknown> = {};

  for (const [key, item] of Object.entries(value)) {
    if (!key || key === SETTINGS_SCHEMA_VERSION_KEY || item === undefined) {
      continue;
    }

    result[key] = item;
  }

  return result;
}

function normalizeFavoritesForExport(value: unknown): FavoriteItem[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const result: FavoriteItem[] = [];

  for (const raw of value) {
    if (!isPlainObject(raw)) {
      continue;
    }

    const path = typeof raw.path === 'string' ? raw.path : '';

    if (!path) {
      continue;
    }

    const item: FavoriteItem = {
      path,
      addedAt: typeof raw.addedAt === 'number' ? raw.addedAt : 0,
      isFile: raw.isFile === true,
      categoryId: typeof raw.categoryId === 'string' ? raw.categoryId : null,
    };

    if (typeof raw.alias === 'string') {
      item.alias = raw.alias;
    }

    result.push(item);
  }

  return result;
}

function normalizeFavoriteCategoriesForExport(value: unknown): FavoriteCategory[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const result: FavoriteCategory[] = [];

  for (const raw of value) {
    if (!isPlainObject(raw)) {
      continue;
    }

    if (typeof raw.id !== 'string' || typeof raw.name !== 'string') {
      continue;
    }

    result.push({
      id: raw.id,
      name: raw.name,
      createdAt: typeof raw.createdAt === 'number' ? raw.createdAt : 0,
    });
  }

  return result;
}

function normalizeTagsForExport(value: unknown): ItemTag[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const result: ItemTag[] = [];

  for (const raw of value) {
    if (!isPlainObject(raw)) {
      continue;
    }

    if (typeof raw.id !== 'string' || typeof raw.name !== 'string') {
      continue;
    }

    result.push({
      id: raw.id,
      name: raw.name,
      color: typeof raw.color === 'string' ? raw.color : '',
    });
  }

  return result;
}

function normalizeTaggedItemsForExport(value: unknown): TaggedItem[] {
  if (!Array.isArray(value)) {
    return [];
  }

  const result: TaggedItem[] = [];

  for (const raw of value) {
    if (!isPlainObject(raw)) {
      continue;
    }

    const path = typeof raw.path === 'string' ? raw.path : '';

    if (!path) {
      continue;
    }

    result.push({
      path,
      tagIds: Array.isArray(raw.tagIds)
        ? raw.tagIds.filter((tagId): tagId is string => typeof tagId === 'string')
        : [],
      addedAt: typeof raw.addedAt === 'number' ? raw.addedAt : 0,
      isFile: raw.isFile === true,
    });
  }

  return result;
}

/**
 * 导出前把一条远程连接脱敏。
 *
 * 去掉两样东西：`credentialKey`（本机 DPAPI 密文的键）与 proxy 的 `password`。
 * 两者跨机都无效，留着只会让导出的文件「看起来带着密码」，徒增泄露风险。
 */
export function sanitizeRemoteConnectionForExport(connection: RemoteConnection): RemoteConnection {
  const sanitizedProxy = { ...(connection.proxy as unknown as Record<string, unknown>) };

  delete sanitizedProxy.password;

  const sanitized: RemoteConnection = {
    ...connection,
    proxy: sanitizedProxy as unknown as RemoteConnection['proxy'],
  };

  // `credentialKey` 是可选字段，用 delete 去掉它 —— 解构省略写法
  // （`const { credentialKey: _c, ...rest } = ...`）会被 no-unused-vars 判红。
  delete sanitized.credentialKey;

  return sanitized;
}

/**
 * 把导入的连接与本机已有连接合并。
 *
 * 规则：**元数据用导入的，凭据用本机的**。
 * 同 id 的连接在本机已有一份凭据（DPAPI 密文），就沿用它 ——
 * 否则「导入设置」会把用户刚配好的密码冲掉；而本机没有的连接，
 * 凭据只能是空，界面据此提示需要重新登录。
 */
export function mergeImportedRemoteConnections(
  imported: RemoteConnection[],
  local: RemoteConnection[],
): MergedRemoteConnections {
  const localById = new Map(local.map(connection => [connection.id, connection]));
  const connections: RemoteConnection[] = [];
  const needsCredential: string[] = [];

  for (const connection of imported) {
    const localConnection = localById.get(connection.id);

    if (!localConnection) {
      connections.push(connection);
      needsCredential.push(connection.id);
      continue;
    }

    const merged: RemoteConnection = { ...connection };

    if (localConnection.credentialKey) {
      merged.credentialKey = localConnection.credentialKey;
    }

    const localProxy = localConnection.proxy as unknown as Record<string, unknown> | undefined;

    if (localProxy && typeof localProxy.password === 'string' && localProxy.password) {
      merged.proxy = {
        ...(merged.proxy as unknown as Record<string, unknown>),
        password: localProxy.password,
      } as unknown as RemoteConnection['proxy'];
    }

    connections.push(merged);
  }

  return {
    connections,
    needsCredential,
  };
}

/**
 * 构造导出包。
 *
 * `stats` 里只取 favorites / favoriteCategories / tags / taggedItems ——
 * history 与 frequentItems 在这一步就被丢掉，导出文件里根本不存在这两项。
 */
export function buildSettingsTransferBundle(
  source: SettingsTransferSource,
  exportedAt: string = new Date().toISOString(),
): SettingsTransferBundle {
  const stats = isPlainObject(source.stats) ? source.stats : {};
  const remote = isPlainObject(source.remoteConnections) ? source.remoteConnections : {};
  const rawConnections = Array.isArray(remote.connections) ? remote.connections : [];

  return {
    format: SETTINGS_TRANSFER_FORMAT,
    version: SETTINGS_TRANSFER_VERSION,
    exportedAt,
    settings: normalizeSettingsForExport(source.settings),
    favorites: normalizeFavoritesForExport(stats.favorites),
    favoriteCategories: normalizeFavoriteCategoriesForExport(stats.favoriteCategories),
    tags: normalizeTagsForExport(stats.tags),
    taggedItems: normalizeTaggedItemsForExport(stats.taggedItems),
    remoteConnections: rawConnections
      .filter(isPlainObject)
      .map(connection => sanitizeRemoteConnectionForExport(connection as unknown as RemoteConnection)),
  };
}

/**
 * 解析导入文件。
 *
 * 格式不对 / 版本更高一律拒绝 —— 宁可让用户知道「这份文件读不了」，
 * 也不要拿半懂的字段去覆盖现有配置。
 */
export function parseSettingsTransferBundle(raw: unknown): SettingsTransferParseResult {
  if (!isPlainObject(raw)) {
    return {
      ok: false,
      reason: 'structure',
    };
  }

  if (raw.format !== SETTINGS_TRANSFER_FORMAT) {
    return {
      ok: false,
      reason: 'format',
    };
  }

  if (typeof raw.version !== 'number' || raw.version > SETTINGS_TRANSFER_VERSION) {
    return {
      ok: false,
      reason: 'version',
    };
  }

  return {
    ok: true,
    bundle: {
      format: SETTINGS_TRANSFER_FORMAT,
      version: raw.version,
      exportedAt: typeof raw.exportedAt === 'string' ? raw.exportedAt : '',
      settings: normalizeSettingsForExport(raw.settings),
      favorites: normalizeFavoritesForExport(raw.favorites),
      favoriteCategories: normalizeFavoriteCategoriesForExport(raw.favoriteCategories),
      tags: normalizeTagsForExport(raw.tags),
      taggedItems: normalizeTaggedItemsForExport(raw.taggedItems),
      remoteConnections: Array.isArray(raw.remoteConnections)
        ? raw.remoteConnections
            .filter(isPlainObject)
            .map(connection => connection as unknown as RemoteConnection)
        : [],
    },
  };
}
