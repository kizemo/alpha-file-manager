// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { useExtensionsStorageStore } from '@/stores/storage/extensions';

const {
  storedValues,
  lazyStoreGetMock,
  lazyStoreSetMock,
  lazyStoreSaveMock,
} = vi.hoisted(() => {
  const values = new Map<string, unknown>();

  return {
    storedValues: values,
    lazyStoreGetMock: vi.fn(async (key: string) => values.get(key)),
    lazyStoreSetMock: vi.fn((key: string, value: unknown) => {
      values.set(key, value);
    }),
    lazyStoreSaveMock: vi.fn(),
  };
});

vi.mock('@tauri-apps/plugin-store', () => ({
  LazyStore: class {
    async save(): Promise<void> {
      await lazyStoreSaveMock();
    }

    async get<T>(key: string): Promise<T | undefined> {
      return lazyStoreGetMock(key) as T | undefined;
    }

    async set<T>(key: string, value: T): Promise<void> {
      lazyStoreSetMock(key, value);
    }
  },
}));

vi.mock('@/stores/storage/user-paths', () => ({
  useUserPathsStore: () => ({
    customPaths: {
      appUserDataDir: '/tmp/user-data',
      appUserDataExtensionsPath: '/tmp/user-data/user-extensions.json',
    },
  }),
}));

const CUSTOM_URL = 'https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json';

const REGISTRY = {
  schemaVersion: '1.0.0',
  extensions: [
    {
      id: 'ext.video',
      name: 'ext.video',
      description: 'ext.video description',
      publisher: { name: 'sigma-hub' },
      repository: 'https://github.com/example/ext-video',
      featured: false,
      categories: ['Other'],
    },
  ],
};

function createBootstrap(data: Record<string, unknown>) {
  return {
    path: '/tmp/user-data/user-extensions.json',
    status: 'ready' as const,
    data,
    schemaVersion: null,
    error: null,
  };
}

describe('extensions storage market sources', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    storedValues.clear();
    lazyStoreGetMock.mockClear();
    lazyStoreSetMock.mockClear();
    lazyStoreSaveMock.mockClear();
  });

  it('writes market sources and per-source registry caches to the store', async () => {
    const storageStore = useExtensionsStorageStore();

    await storageStore.init(createBootstrap({}));
    await storageStore.setMarketSources([
      {
        id: 'custom-1',
        name: 'My market',
        url: CUSTOM_URL,
        kind: 'custom',
        enabled: true,
      },
    ]);
    await storageStore.updateRegistryCacheForSource('custom-1', REGISTRY);

    expect(storedValues.get('marketSources')).toEqual([
      expect.objectContaining({
        id: 'custom-1',
        name: 'My market',
        url: CUSTOM_URL,
        kind: 'custom',
        enabled: true,
      }),
    ]);
    expect(storedValues.get('registryCaches')).toEqual({
      'custom-1': {
        data: REGISTRY,
        fetchedAt: expect.any(Number),
      },
    });
  });

  it('reads market sources and per-source registry caches back on startup', async () => {
    storedValues.set('marketSources', [
      {
        id: 'custom-1',
        name: 'My market',
        url: CUSTOM_URL,
        kind: 'custom',
        enabled: false,
      },
    ]);
    storedValues.set('registryCaches', {
      'custom-1': {
        data: REGISTRY,
        fetchedAt: 4242,
      },
    });

    const storageStore = useExtensionsStorageStore();

    await storageStore.init(createBootstrap({
      marketSources: [
        {
          id: 'custom-1',
          name: 'My market',
          url: CUSTOM_URL,
          kind: 'custom',
          enabled: false,
        },
      ],
      registryCaches: {
        'custom-1': {
          data: REGISTRY,
          fetchedAt: 4242,
        },
      },
    }));

    expect(storageStore.extensionsData.marketSources).toEqual([
      expect.objectContaining({
        id: 'custom-1',
        enabled: false,
      }),
    ]);
    expect(storageStore.getRegistryCacheForSource('custom-1')).toEqual({
      data: REGISTRY,
      fetchedAt: 4242,
    });
  });

  it('migrates the legacy single registry cache to the official source', async () => {
    const storageStore = useExtensionsStorageStore();

    await storageStore.init(createBootstrap({
      registryCache: {
        data: REGISTRY,
        fetchedAt: 1234,
      },
    }));

    expect(storageStore.getRegistryCacheForSource('official')).toEqual({
      data: REGISTRY,
      fetchedAt: 1234,
    });
  });

  it('removes the cache of a deleted market source', async () => {
    const storageStore = useExtensionsStorageStore();

    await storageStore.init(createBootstrap({}));
    await storageStore.updateRegistryCacheForSource('custom-1', REGISTRY);

    expect(storageStore.getRegistryCacheForSource('custom-1')).toBeDefined();

    await storageStore.removeRegistryCacheForSource('custom-1');

    expect(storageStore.getRegistryCacheForSource('custom-1')).toBeUndefined();
    expect(storedValues.get('registryCaches')).toEqual({});
  });
});
