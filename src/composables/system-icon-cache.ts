// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { invoke } from '@tauri-apps/api/core';
import { platform } from '@tauri-apps/plugin-os';
import type { DirEntry } from '@/types/dir-entry';

const WINDOWS_UNIQUE_ICON_EXTENSIONS = new Set([
  'exe',
  'dll',
  'ico',
  'lnk',
  'scr',
  'cpl',
  'msi',
  'appx',
  'msix',
]);

const LINUX_UNIQUE_ICON_EXTENSIONS = new Set([
  'desktop',
  'appimage',
]);

const MACOS_UNIQUE_ICON_EXTENSIONS = new Set([
  'icns',
]);

export const NAVIGATOR_ICON_PREFETCH_SIZES = [18, 24, 48] as const;
const SYSTEM_ICON_PREFETCH_CONCURRENCY = 6;

// 系统图标缓存的容量上限。缓存值是 base64 PNG data URL（体积大），
// 原本只有写入、没有任何淘汰逻辑，浏览含大量 exe/dll 的目录时内存会单调增长。
const SYSTEM_ICON_CACHE_MAX_ENTRIES = 512;

type SystemIconCacheKeyClass = 'extension' | 'path' | 'directory';

// 淘汰优先级：ext: 类命中率高、条数天然有界，优先保留；
// path: 类（exe/dll 等）最容易无限增长，最先被淘汰；dir: 类次之。
const SYSTEM_ICON_CACHE_EVICTION_ORDER: readonly SystemIconCacheKeyClass[] = [
  'path',
  'directory',
  'extension',
];

const systemIconCache = new Map<string, string | null>();
const systemIconInFlight = new Map<string, Promise<string | null>>();

export interface SystemIconCacheStats {
  total: number;
  extension: number;
  path: number;
  directory: number;
  limit: number;
}

export interface SystemIconRequest {
  path: string;
  isDir: boolean;
  extension: string | null;
  size: number;
}

function isSystemIconRequestPath(path: string): boolean {
  const trimmed = path.trim();

  if (!trimmed || trimmed.includes('://')) {
    return false;
  }

  const lower = trimmed.toLowerCase();

  if (trimmed.startsWith('::{') || lower.startsWith('shell:')) {
    return false;
  }

  return true;
}

function getUniqueIconExtensions(): Set<string> {
  const currentPlatform = platform();

  if (currentPlatform === 'linux') {
    return LINUX_UNIQUE_ICON_EXTENSIONS;
  }

  if (currentPlatform === 'macos') {
    return MACOS_UNIQUE_ICON_EXTENSIONS;
  }

  return WINDOWS_UNIQUE_ICON_EXTENSIONS;
}

function isExtensionCacheKey(cacheKey: string): boolean {
  return cacheKey.startsWith('ext:');
}

function getSystemIconCacheKeyClass(cacheKey: string): SystemIconCacheKeyClass {
  if (isExtensionCacheKey(cacheKey)) {
    return 'extension';
  }

  if (cacheKey.startsWith('path:')) {
    return 'path';
  }

  return 'directory';
}

// Map 保持插入序，删除后重新插入即可把该条目标记为「最近使用」。
function touchSystemIconCacheEntry(cacheKey: string, icon: string | null): void {
  systemIconCache.delete(cacheKey);
  systemIconCache.set(cacheKey, icon);
}

function setSystemIconCacheEntry(cacheKey: string, icon: string | null): void {
  touchSystemIconCacheEntry(cacheKey, icon);
  enforceSystemIconCacheLimit();
}

function enforceSystemIconCacheLimit(): void {
  while (systemIconCache.size > SYSTEM_ICON_CACHE_MAX_ENTRIES) {
    if (!evictLeastRecentlyUsedSystemIconEntry()) {
      return;
    }
  }
}

// 按淘汰优先级找出最久未使用的条目并删除：先 path:，再 dir:，最后 ext:。
// 同一类别内部按插入序遍历，第一个即最久未使用的那一条。
function evictLeastRecentlyUsedSystemIconEntry(): boolean {
  for (const keyClass of SYSTEM_ICON_CACHE_EVICTION_ORDER) {
    for (const cacheKey of systemIconCache.keys()) {
      if (getSystemIconCacheKeyClass(cacheKey) === keyClass) {
        systemIconCache.delete(cacheKey);
        return true;
      }
    }
  }

  return false;
}

async function runWithConcurrencyLimit<T>(
  items: T[],
  concurrency: number,
  worker: (item: T) => Promise<unknown>,
): Promise<void> {
  let nextItemIndex = 0;
  const workerCount = Math.min(concurrency, items.length);

  await Promise.all(Array.from({ length: workerCount }, async () => {
    while (nextItemIndex < items.length) {
      const itemIndex = nextItemIndex;
      nextItemIndex += 1;
      const item = items[itemIndex];

      if (item === undefined) {
        return;
      }

      await worker(item);
    }
  }));
}

export function getSystemIconCacheKey(request: SystemIconRequest): string {
  const iconSize = Math.max(8, Math.min(256, Math.round(request.size)));

  if (request.isDir) {
    return `dir:${request.path.toLowerCase()}:${iconSize}`;
  }

  const normalizedExtension = request.extension?.toLowerCase() ?? '';

  if (normalizedExtension && getUniqueIconExtensions().has(normalizedExtension)) {
    return `path:${request.path.toLowerCase()}:${iconSize}`;
  }

  return `ext:${normalizedExtension}:${iconSize}`;
}

export async function fetchSystemIconCached(request: SystemIconRequest): Promise<string | null> {
  const iconSize = Math.max(8, Math.min(256, Math.round(request.size)));
  const requestCacheKey = getSystemIconCacheKey({
    ...request,
    size: iconSize,
  });

  if (!isSystemIconRequestPath(request.path)) {
    return null;
  }

  const cached = systemIconCache.get(requestCacheKey);

  if (cached !== undefined) {
    touchSystemIconCacheEntry(requestCacheKey, cached);
    return cached;
  }

  const existingRequest = systemIconInFlight.get(requestCacheKey);

  if (existingRequest) {
    return await existingRequest;
  }

  const requestPromise = invoke<string | null>('get_system_icon', {
    path: request.path,
    isDir: request.isDir,
    extension: request.extension,
    size: iconSize,
  })
    .then((result) => {
      if (result !== null || !isExtensionCacheKey(requestCacheKey)) {
        setSystemIconCacheEntry(requestCacheKey, result);
      }

      return result;
    })
    .catch(() => {
      if (!isExtensionCacheKey(requestCacheKey)) {
        setSystemIconCacheEntry(requestCacheKey, null);
      }

      return null;
    })
    .finally(() => {
      systemIconInFlight.delete(requestCacheKey);
    });

  systemIconInFlight.set(requestCacheKey, requestPromise);
  return await requestPromise;
}

export function prefetchSystemIconsForEntries(
  entries: DirEntry[],
  sizes: readonly number[] = NAVIGATOR_ICON_PREFETCH_SIZES,
): void {
  const pendingRequests = new Map<string, SystemIconRequest>();

  for (const entry of entries) {
    if (entry.drive_metadata) {
      continue;
    }

    for (const size of sizes) {
      const request: SystemIconRequest = {
        path: entry.path,
        isDir: entry.is_dir,
        extension: entry.ext,
        size,
      };
      const cacheKey = getSystemIconCacheKey(request);

      if (systemIconCache.has(cacheKey) || systemIconInFlight.has(cacheKey)) {
        continue;
      }

      pendingRequests.set(cacheKey, request);
    }
  }

  void runWithConcurrencyLimit(
    [...pendingRequests.values()],
    SYSTEM_ICON_PREFETCH_CONCURRENCY,
    request => fetchSystemIconCached(request),
  );
}

// 仅供测试使用，生产路径不得调用。
export function __getSystemIconCacheStatsForTests(): SystemIconCacheStats {
  const stats: SystemIconCacheStats = {
    total: systemIconCache.size,
    extension: 0,
    path: 0,
    directory: 0,
    limit: SYSTEM_ICON_CACHE_MAX_ENTRIES,
  };

  for (const cacheKey of systemIconCache.keys()) {
    stats[getSystemIconCacheKeyClass(cacheKey)] += 1;
  }

  return stats;
}

// 仅供测试使用，生产路径不得调用。
export function __resetSystemIconCacheForTests(): void {
  systemIconCache.clear();
  systemIconInFlight.clear();
}
