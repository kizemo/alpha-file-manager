// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import type { DirEntry } from '@/types/dir-entry';

type PendingIconRequest = {
  resolveIcon: (icon: string | null) => void;
};

const {
  invokeMock,
  platformMock,
} = vi.hoisted(() => ({
  invokeMock: vi.fn<(command: string, payload: unknown) => Promise<string | null>>(),
  platformMock: vi.fn(() => 'windows'),
}));

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}));

vi.mock('@tauri-apps/plugin-os', () => ({
  platform: platformMock,
}));

function createEntry(overrides: Partial<DirEntry> = {}): DirEntry {
  return {
    name: 'readme.txt',
    ext: 'txt',
    path: 'C:/docs/readme.txt',
    size: 0,
    item_count: null,
    modified_time: 0,
    created_time: 0,
    accessed_time: 0,
    is_dir: false,
    is_file: true,
    is_hidden: false,
    is_symlink: false,
    mime: null,
    ...overrides,
  };
}

function createExeIconRequest(entryIndex: number, size = 32) {
  return {
    path: `C:/apps/app-${entryIndex}.exe`,
    isDir: false,
    extension: 'exe',
    size,
  };
}

describe('system icon cache', () => {
  beforeEach(() => {
    vi.resetModules();
    invokeMock.mockReset();
    platformMock.mockReturnValue('windows');
  });

  it('deduplicates generic file icons by extension', async () => {
    const { getSystemIconCacheKey } = await import('@/composables/system-icon-cache');
    const firstKey = getSystemIconCacheKey({
      path: 'C:/docs/readme.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });
    const secondKey = getSystemIconCacheKey({
      path: 'C:/docs/notes.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });

    expect(firstKey).toBe('ext:txt:32');
    expect(secondKey).toBe(firstKey);
  });

  it('uses platform-specific unique icon cache keys', async () => {
    const { getSystemIconCacheKey } = await import('@/composables/system-icon-cache');

    expect(getSystemIconCacheKey({
      path: 'C:/apps/launcher.desktop',
      isDir: false,
      extension: 'desktop',
      size: 32,
    })).toBe('ext:desktop:32');

    platformMock.mockReturnValue('linux');

    expect(getSystemIconCacheKey({
      path: '/usr/share/applications/launcher.desktop',
      isDir: false,
      extension: 'desktop',
      size: 32,
    })).toBe('path:/usr/share/applications/launcher.desktop:32');
  });

  it('does not cache generic extension misses for the full session', async () => {
    invokeMock.mockResolvedValue(null);

    const { fetchSystemIconCached } = await import('@/composables/system-icon-cache');

    await fetchSystemIconCached({
      path: 'C:/docs/readme.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });
    await fetchSystemIconCached({
      path: 'C:/docs/notes.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });

    expect(invokeMock).toHaveBeenCalledTimes(2);
  });

  it('prefetches unique cache keys only once per extension', async () => {
    invokeMock.mockResolvedValue('txt-icon');

    const {
      fetchSystemIconCached,
      prefetchSystemIconsForEntries,
    } = await import('@/composables/system-icon-cache');

    prefetchSystemIconsForEntries([
      createEntry({ path: 'C:/docs/readme.txt' }),
      createEntry({
        path: 'C:/docs/notes.txt',
        name: 'notes.txt',
      }),
    ], [32]);

    await fetchSystemIconCached({
      path: 'C:/docs/readme.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });
    await Promise.resolve();
    await new Promise(resolve => setTimeout(resolve, 0));

    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('limits concurrent prefetch requests', async () => {
    const pendingRequests: PendingIconRequest[] = [];
    let activeRequestCount = 0;
    let maxActiveRequestCount = 0;

    invokeMock.mockImplementation(() => {
      activeRequestCount += 1;
      maxActiveRequestCount = Math.max(maxActiveRequestCount, activeRequestCount);

      return new Promise((resolveIcon) => {
        pendingRequests.push({
          resolveIcon: (icon) => {
            activeRequestCount -= 1;
            resolveIcon(icon);
          },
        });
      });
    });

    const { prefetchSystemIconsForEntries } = await import('@/composables/system-icon-cache');
    const entries = Array.from({ length: 10 }, (_value, entryIndex) => createEntry({
      name: `Folder ${entryIndex}`,
      path: `C:/Projects/Folder-${entryIndex}`,
      is_dir: true,
      is_file: false,
      ext: null,
    }));

    prefetchSystemIconsForEntries(entries, [32]);
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledTimes(6);
    expect(maxActiveRequestCount).toBe(6);

    const firstPendingRequest = pendingRequests.shift();
    firstPendingRequest?.resolveIcon('folder-icon');
    await flushPromises();

    expect(invokeMock).toHaveBeenCalledTimes(7);
    expect(maxActiveRequestCount).toBe(6);

    await drainPendingRequests(pendingRequests, 10);

    expect(invokeMock).toHaveBeenCalledTimes(10);
    expect(activeRequestCount).toBe(0);
  });

  it('keeps the system icon cache within its capacity limit', async () => {
    invokeMock.mockResolvedValue('exe-icon');

    const {
      fetchSystemIconCached,
      __getSystemIconCacheStatsForTests,
    } = await import('@/composables/system-icon-cache');

    const limit = __getSystemIconCacheStatsForTests().limit;

    for (let entryIndex = 0; entryIndex < limit + 40; entryIndex += 1) {
      await fetchSystemIconCached(createExeIconRequest(entryIndex));
    }

    const stats = __getSystemIconCacheStatsForTests();

    expect(stats.total).toBeLessThanOrEqual(limit);
    expect(stats.path).toBe(limit);
  });

  it('evicts unique path entries before generic extension entries', async () => {
    invokeMock.mockResolvedValue('icon');

    const {
      fetchSystemIconCached,
      __getSystemIconCacheStatsForTests,
    } = await import('@/composables/system-icon-cache');

    await fetchSystemIconCached({
      path: 'C:/docs/readme.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });
    await fetchSystemIconCached({
      path: 'C:/docs/readme.md',
      isDir: false,
      extension: 'md',
      size: 32,
    });

    const limit = __getSystemIconCacheStatsForTests().limit;

    for (let entryIndex = 0; entryIndex < limit + 40; entryIndex += 1) {
      await fetchSystemIconCached(createExeIconRequest(entryIndex));
    }

    const stats = __getSystemIconCacheStatsForTests();

    expect(stats.total).toBeLessThanOrEqual(limit);
    expect(stats.extension).toBe(2);

    const invokeCountBeforeExtensionHit = invokeMock.mock.calls.length;
    const extensionIcon = await fetchSystemIconCached({
      path: 'C:/docs/other.txt',
      isDir: false,
      extension: 'txt',
      size: 32,
    });

    expect(extensionIcon).toBe('icon');
    expect(invokeMock).toHaveBeenCalledTimes(invokeCountBeforeExtensionHit);
  });

  it('deduplicates concurrent requests for the same system icon cache key', async () => {
    const pendingRequests: PendingIconRequest[] = [];

    invokeMock.mockImplementation(() => new Promise((resolveIcon) => {
      pendingRequests.push({ resolveIcon });
    }));

    const { fetchSystemIconCached } = await import('@/composables/system-icon-cache');
    const request = createExeIconRequest(0);
    const firstRequest = fetchSystemIconCached(request);
    const secondRequest = fetchSystemIconCached(request);

    await flushPromises();

    expect(invokeMock).toHaveBeenCalledTimes(1);

    pendingRequests.shift()?.resolveIcon('exe-icon');

    expect(await firstRequest).toBe('exe-icon');
    expect(await secondRequest).toBe('exe-icon');
    expect(invokeMock).toHaveBeenCalledTimes(1);
  });

  it('refreshes the least recently used order when a cache entry is hit', async () => {
    invokeMock.mockResolvedValue('exe-icon');

    const {
      fetchSystemIconCached,
      __getSystemIconCacheStatsForTests,
    } = await import('@/composables/system-icon-cache');

    const limit = __getSystemIconCacheStatsForTests().limit;

    for (let entryIndex = 0; entryIndex < limit; entryIndex += 1) {
      await fetchSystemIconCached(createExeIconRequest(entryIndex));
    }

    // 命中 app-0.exe，把它刷新成「最近使用」
    expect(await fetchSystemIconCached(createExeIconRequest(0))).toBe('exe-icon');

    // 再插入一条新条目触发淘汰，最久未使用的 app-1.exe 应当被淘汰
    await fetchSystemIconCached(createExeIconRequest(limit));

    const invokeCountBeforeRefetch = invokeMock.mock.calls.length;

    expect(await fetchSystemIconCached(createExeIconRequest(0))).toBe('exe-icon');
    expect(invokeMock).toHaveBeenCalledTimes(invokeCountBeforeRefetch);

    expect(await fetchSystemIconCached(createExeIconRequest(1))).toBe('exe-icon');
    expect(invokeMock).toHaveBeenCalledTimes(invokeCountBeforeRefetch + 1);
  });

  it('counts directory cache entries separately and clears the cache on demand', async () => {
    invokeMock.mockResolvedValue('icon');

    const {
      fetchSystemIconCached,
      __getSystemIconCacheStatsForTests,
      __resetSystemIconCacheForTests,
    } = await import('@/composables/system-icon-cache');

    await fetchSystemIconCached({
      path: 'C:/Projects',
      isDir: true,
      extension: null,
      size: 32,
    });
    await fetchSystemIconCached(createExeIconRequest(0));

    const stats = __getSystemIconCacheStatsForTests();

    expect(stats.total).toBe(2);
    expect(stats.directory).toBe(1);
    expect(stats.path).toBe(1);
    expect(stats.extension).toBe(0);

    __resetSystemIconCacheForTests();

    expect(__getSystemIconCacheStatsForTests().total).toBe(0);
  });
});

async function drainPendingRequests(
  pendingRequests: PendingIconRequest[],
  expectedRequestCount: number,
) {
  for (let attemptIndex = 0; attemptIndex < expectedRequestCount; attemptIndex += 1) {
    while (pendingRequests.length > 0) {
      pendingRequests.shift()?.resolveIcon('folder-icon');
    }

    await flushPromises();

    if (invokeMock.mock.calls.length >= expectedRequestCount && pendingRequests.length === 0) {
      return;
    }
  }

  expect(invokeMock).toHaveBeenCalledTimes(expectedRequestCount);
}

async function flushPromises() {
  await Promise.resolve();
  await new Promise(resolve => setTimeout(resolve, 0));
}
