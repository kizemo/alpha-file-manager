// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import type {
  ExtensionMarketSource,
  ExtensionRegistry,
  ExtensionRegistryCache,
  ExtensionRegistryEntryWithSource,
} from '@/types/extension';

/** 官方市场源（内置常量，不落盘）的固定 id。 */
export const OFFICIAL_MARKET_SOURCE_ID = 'official';

/** 第三方内置市场源（上游社区市场）的固定 id。同样不落盘、不可删。 */
export const COMMUNITY_MARKET_SOURCE_ID = 'community';

export type MarketSourceStatus = 'idle' | 'ok' | 'stale' | 'error';

/** 单个市场源的拉取状态，供市场页与设置页展示。 */
export type RegistrySourceState = {
  sourceId: string;
  status: MarketSourceStatus;
  error?: string;
  fetchedAt?: number;
  entryCount: number;
};

/** 单个源的拉取结果。`stale` = 拉取失败但有上次缓存，`error` = 拉取失败且无缓存。 */
export type MarketSourceFetchOutcome
  = | {
    status: 'ok';
    registry: ExtensionRegistry;
    fetchedAt: number;
  }
  | {
    status: 'stale';
    registry: ExtensionRegistry;
    fetchedAt: number;
    error: string;
  }
  | {
    status: 'error';
    error: string;
  };

export type MergedMarketRegistry = {
  schemaVersion: string;
  entries: ExtensionRegistryEntryWithSource[];
  conflicts: Array<{
    id: string;
    keptSourceId: string;
    droppedSourceId: string;
  }>;
  failedSourceIds: string[];
};

const BLOCKED_HOSTNAMES = new Set([
  'localhost',
  'localhost.localdomain',
  'ip6-localhost',
  'ip6-loopback',
]);

/**
 * 宿主拉取走 Rust 侧 `PublicOnlyResolver`：回环 / 内网 / 链路本地地址在 connect
 * 阶段就被拒绝。因此自建源必须是公网 https，前端要在添加时就拦掉本地地址，
 * 否则用户会看到「添加成功但永远拉不到」的源。
 */
export function isPublicHttpsUrl(rawUrl: string): boolean {
  let parsedUrl: URL;

  try {
    parsedUrl = new URL(rawUrl.trim());
  }
  catch {
    return false;
  }

  if (parsedUrl.protocol !== 'https:') {
    return false;
  }

  const hostname = parsedUrl.hostname.toLowerCase().replace(/^\[/, '').replace(/\]$/, '');

  if (!hostname) {
    return false;
  }

  if (BLOCKED_HOSTNAMES.has(hostname)) {
    return false;
  }

  if (hostname.endsWith('.localhost') || hostname.endsWith('.local') || hostname.endsWith('.internal')) {
    return false;
  }

  if (hostname === '::1' || hostname === '0.0.0.0') {
    return false;
  }

  // IPv6 链路本地 / 唯一本地地址
  if (hostname.includes(':')) {
    if (/^fe80:/i.test(hostname) || /^(fc|fd)/i.test(hostname)) {
      return false;
    }
  }

  const ipv4Match = hostname.match(/^(\d{1,3})\.(\d{1,3})\.(\d{1,3})\.(\d{1,3})$/);

  if (ipv4Match) {
    const firstOctet = Number(ipv4Match[1]);
    const secondOctet = Number(ipv4Match[2]);

    const isPrivate = firstOctet === 10
      || firstOctet === 127
      || firstOctet === 0
      || (firstOctet === 192 && secondOctet === 168)
      || (firstOctet === 172 && secondOctet >= 16 && secondOctet <= 31)
      || (firstOctet === 169 && secondOctet === 254);

    if (isPrivate) {
      return false;
    }
  }
  else if (!hostname.includes(':') && !hostname.includes('.')) {
    // 单标签主机名（intranet / nas 之类）通常指向内网
    return false;
  }

  return true;
}

/** 规范化清单地址；不是公网 https 时返回 null。 */
export function normalizeMarketSourceUrl(rawUrl: string): string | null {
  const trimmedUrl = rawUrl.trim();

  if (!trimmedUrl || !isPublicHttpsUrl(trimmedUrl)) {
    return null;
  }

  try {
    const parsedUrl = new URL(trimmedUrl);
    parsedUrl.hash = '';
    return parsedUrl.toString();
  }
  catch {
    return null;
  }
}

function hashMarketSourceUrl(url: string): string {
  let hash = 0x811C9DC5;

  for (let index = 0; index < url.length; index += 1) {
    hash ^= url.charCodeAt(index);
    hash = Math.imul(hash, 0x01000193) >>> 0;
  }

  return hash.toString(36);
}

/** 自建源 id 由地址派生 ⇒ 同一个地址重复添加不会产生第二条源。 */
export function createCustomMarketSourceId(url: string): string {
  return `custom-${hashMarketSourceUrl(url)}`;
}

export function buildCustomMarketSource(name: string, url: string): ExtensionMarketSource {
  const normalizedUrl = normalizeMarketSourceUrl(url) ?? url.trim();

  return {
    id: createCustomMarketSourceId(normalizedUrl),
    name: name.trim(),
    url: normalizedUrl,
    kind: 'custom',
    enabled: true,
  };
}

/** 存储里的源记录可能来自旧版本或被手改，逐字段兜底。 */
export function normalizeMarketSourceRecord(record: unknown): ExtensionMarketSource | null {
  if (!record || typeof record !== 'object') {
    return null;
  }

  const candidate = record as Partial<ExtensionMarketSource>;

  if (typeof candidate.id !== 'string' || !candidate.id) {
    return null;
  }

  if (typeof candidate.url !== 'string' || !candidate.url) {
    return null;
  }

  // 落盘的只有自建源，正常不会出现 official / community；但旧数据或被手改的文件
  // 可能带着这些 kind，逐档兜底，未知值一律归为 custom。
  const kind: ExtensionMarketSource['kind'] = candidate.kind === 'official'
    ? 'official'
    : candidate.kind === 'community' ? 'community' : 'custom';

  return {
    id: candidate.id,
    name: typeof candidate.name === 'string' && candidate.name ? candidate.name : candidate.id,
    url: candidate.url,
    kind,
    enabled: candidate.enabled !== false,
    builtIn: candidate.builtIn === true,
  };
}

/**
 * 合并多个源的清单。
 *
 * - 源按传入顺序处理（官方源恒排第一）⇒ id 冲突时「先到先得」= 官方源优先。
 * - 冲突记录在 `conflicts` 里，由调用方写 `console.warn`。
 * - `failedSourceIds` 是「拉取失败且无缓存」的源：它们的条目不参与合并，
 *   且调用方必须跳过卸载判定（否则该源 404 会把别的源的扩展误删）。
 */
export function mergeMarketSourceRegistries(
  sources: ExtensionMarketSource[],
  outcomes: Map<string, MarketSourceFetchOutcome>,
): MergedMarketRegistry {
  const entries: ExtensionRegistryEntryWithSource[] = [];
  const conflicts: MergedMarketRegistry['conflicts'] = [];
  const failedSourceIds: string[] = [];
  const keptSourceIdByEntryId = new Map<string, string>();
  let schemaVersion = '';

  for (const source of sources) {
    const outcome = outcomes.get(source.id);

    if (!outcome || outcome.status === 'error') {
      failedSourceIds.push(source.id);
      continue;
    }

    if (!schemaVersion) {
      schemaVersion = outcome.registry.schemaVersion;
    }

    for (const entry of outcome.registry.extensions) {
      const keptSourceId = keptSourceIdByEntryId.get(entry.id);

      if (keptSourceId) {
        conflicts.push({
          id: entry.id,
          keptSourceId,
          droppedSourceId: source.id,
        });
        continue;
      }

      keptSourceIdByEntryId.set(entry.id, source.id);
      entries.push({
        ...entry,
        sourceId: source.id,
      });
    }
  }

  return {
    schemaVersion,
    entries,
    conflicts,
    failedSourceIds,
  };
}

/**
 * 卸载判定用的 id 全集。
 *
 * 除了本次合并到的条目，还要带上**已停用源**的缓存条目 ——
 * 「停用 ≠ 下架」：用户停用一个源只是不想再从它那儿取新东西，
 * 已经装好的扩展不能因此被卸掉。
 */
export function collectApprovedExtensionIds(
  entries: ExtensionRegistryEntryWithSource[],
  disabledSourceCaches: Array<ExtensionRegistryCache | undefined>,
): Set<string> {
  const approvedExtensionIds = new Set<string>();

  for (const entry of entries) {
    approvedExtensionIds.add(entry.id);
  }

  for (const cache of disabledSourceCaches) {
    if (!cache) {
      continue;
    }

    for (const entry of cache.data.extensions) {
      approvedExtensionIds.add(entry.id);
    }
  }

  return approvedExtensionIds;
}

/** 取各源里最新的拉取时间（用于 TTL 判定：整体新鲜度取决于最旧的那个源）。 */
export function resolveRegistryFetchedAt(outcomes: Map<string, MarketSourceFetchOutcome>): number | null {
  let oldestFetchedAt: number | null = null;

  for (const outcome of outcomes.values()) {
    if (outcome.status === 'error') {
      continue;
    }

    if (oldestFetchedAt === null || outcome.fetchedAt < oldestFetchedAt) {
      oldestFetchedAt = outcome.fetchedAt;
    }
  }

  return oldestFetchedAt;
}
