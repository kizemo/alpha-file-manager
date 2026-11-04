// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  OFFICIAL_MARKET_SOURCE_ID,
  buildCustomMarketSource,
  collectApprovedExtensionIds,
  createCustomMarketSourceId,
  isPublicHttpsUrl,
  mergeMarketSourceRegistries,
  normalizeMarketSourceRecord,
  normalizeMarketSourceUrl,
  resolveRegistryFetchedAt,
  type MarketSourceFetchOutcome,
} from '../market-sources';
import type {
  ExtensionMarketSource,
  ExtensionRegistry,
  ExtensionRegistryCache,
} from '../../../../types/extension';

const OFFICIAL_URL = 'https://raw.githubusercontent.com/sigma-hub/sfm-marketplace/main/registry.json';
const CUSTOM_URL = 'https://raw.githubusercontent.com/kizemo/sfm-marketplace/main/registry.json';

function createSource(overrides: Partial<ExtensionMarketSource> = {}): ExtensionMarketSource {
  return {
    id: OFFICIAL_MARKET_SOURCE_ID,
    name: 'Official',
    url: OFFICIAL_URL,
    kind: 'official',
    enabled: true,
    ...overrides,
  };
}

function createRegistry(...ids: string[]): ExtensionRegistry {
  return {
    schemaVersion: '1.0.0',
    extensions: ids.map(id => ({
      id,
      name: id,
      description: `${id} description`,
      publisher: { name: 'sigma-hub' },
      repository: `https://github.com/example/${id}`,
      featured: false,
      categories: ['Other'],
    })),
  };
}

function createCache(data: ExtensionRegistry, fetchedAt = 1_000): ExtensionRegistryCache {
  return {
    data,
    fetchedAt,
  };
}

describe('isPublicHttpsUrl', () => {
  it('accepts public https urls', () => {
    expect(isPublicHttpsUrl(OFFICIAL_URL)).toBe(true);
    expect(isPublicHttpsUrl('https://example.com/registry.json')).toBe(true);
    expect(isPublicHttpsUrl('https://8.8.8.8/registry.json')).toBe(true);
  });

  it('rejects non-https schemes', () => {
    expect(isPublicHttpsUrl('http://example.com/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('file:///C:/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('ftp://example.com/registry.json')).toBe(false);
  });

  it('rejects loopback, private and link-local addresses', () => {
    expect(isPublicHttpsUrl('https://localhost/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://127.0.0.1/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://10.0.0.5/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://172.16.3.4/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://192.168.1.10/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://169.254.10.10/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://[::1]/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://[fd00::1]/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://[fe80::1]/registry.json')).toBe(false);
  });

  it('rejects local-only hostnames', () => {
    expect(isPublicHttpsUrl('https://nas.local/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://build.internal/registry.json')).toBe(false);
    expect(isPublicHttpsUrl('https://intranet/registry.json')).toBe(false);
  });
});

describe('normalizeMarketSourceUrl', () => {
  it('trims the url and drops the fragment', () => {
    expect(normalizeMarketSourceUrl(`  ${OFFICIAL_URL}#top  `)).toBe(OFFICIAL_URL);
  });

  it('returns null for unusable urls', () => {
    expect(normalizeMarketSourceUrl('http://example.com/registry.json')).toBeNull();
    expect(normalizeMarketSourceUrl('   ')).toBeNull();
    expect(normalizeMarketSourceUrl('not a url')).toBeNull();
  });
});

describe('createCustomMarketSourceId', () => {
  it('derives a stable id from the url', () => {
    expect(createCustomMarketSourceId(CUSTOM_URL)).toBe(createCustomMarketSourceId(CUSTOM_URL));
    expect(createCustomMarketSourceId(CUSTOM_URL)).not.toBe(createCustomMarketSourceId(OFFICIAL_URL));
  });

  it('produces a non-official id', () => {
    expect(createCustomMarketSourceId(CUSTOM_URL)).not.toBe(OFFICIAL_MARKET_SOURCE_ID);
  });
});

describe('buildCustomMarketSource', () => {
  it('builds an enabled custom source with a normalized url', () => {
    const source = buildCustomMarketSource('  My market  ', ` ${CUSTOM_URL} `);

    expect(source.kind).toBe('custom');
    expect(source.enabled).toBe(true);
    expect(source.name).toBe('My market');
    expect(source.url).toBe(CUSTOM_URL);
    expect(source.id).toBe(createCustomMarketSourceId(CUSTOM_URL));
  });
});

describe('normalizeMarketSourceRecord', () => {
  it('rejects non-object records', () => {
    expect(normalizeMarketSourceRecord(null)).toBeNull();
    expect(normalizeMarketSourceRecord('nope')).toBeNull();
    expect(normalizeMarketSourceRecord({})).toBeNull();
    expect(normalizeMarketSourceRecord({ id: 'custom-1' })).toBeNull();
  });

  it('fills in defaults for partially written records', () => {
    const source = normalizeMarketSourceRecord({
      id: 'custom-1',
      url: CUSTOM_URL,
    });

    expect(source).not.toBeNull();
    expect(source?.name).toBe('custom-1');
    expect(source?.kind).toBe('custom');
    expect(source?.enabled).toBe(true);
    expect(source?.builtIn).toBe(false);
  });

  it('keeps the disabled flag', () => {
    const source = normalizeMarketSourceRecord({
      id: 'custom-1',
      name: 'My market',
      url: CUSTOM_URL,
      enabled: false,
    });

    expect(source?.enabled).toBe(false);
  });
});

describe('mergeMarketSourceRegistries', () => {
  it('merges sources in order and keeps the first entry for duplicated ids', () => {
    const official = createSource();
    const custom = createSource({
      id: 'custom-1',
      name: 'My market',
      url: CUSTOM_URL,
      kind: 'custom',
    });

    const outcomes = new Map<string, MarketSourceFetchOutcome>([
      [official.id, {
        status: 'ok',
        registry: createRegistry('a', 'shared'),
        fetchedAt: 100,
      }],
      [custom.id, {
        status: 'ok',
        registry: createRegistry('b', 'shared'),
        fetchedAt: 200,
      }],
    ]);

    const merged = mergeMarketSourceRegistries([official, custom], outcomes);

    expect(merged.entries.map(entry => entry.id)).toEqual(['a', 'shared', 'b']);
    expect(merged.entries[0].sourceId).toBe(official.id);
    expect(merged.entries[2].sourceId).toBe(custom.id);
    expect(merged.conflicts).toEqual([
      {
        id: 'shared',
        keptSourceId: official.id,
        droppedSourceId: custom.id,
      },
    ]);
    expect(merged.failedSourceIds).toEqual([]);
  });

  it('reports sources that failed without a cache', () => {
    const official = createSource();
    const custom = createSource({
      id: 'custom-1',
      name: 'My market',
      url: CUSTOM_URL,
      kind: 'custom',
    });

    const outcomes = new Map<string, MarketSourceFetchOutcome>([
      [official.id, {
        status: 'ok',
        registry: createRegistry('a'),
        fetchedAt: 100,
      }],
      [custom.id, {
        status: 'error',
        error: 'boom',
      }],
    ]);

    const merged = mergeMarketSourceRegistries([official, custom], outcomes);

    expect(merged.entries.map(entry => entry.id)).toEqual(['a']);
    expect(merged.failedSourceIds).toEqual(['custom-1']);
  });

  it('keeps stale entries of a source that failed but still has a cache', () => {
    const official = createSource();

    const outcomes = new Map<string, MarketSourceFetchOutcome>([
      [official.id, {
        status: 'stale',
        registry: createRegistry('a'),
        fetchedAt: 100,
        error: 'offline',
      }],
    ]);

    const merged = mergeMarketSourceRegistries([official], outcomes);

    expect(merged.entries.map(entry => entry.id)).toEqual(['a']);
    expect(merged.failedSourceIds).toEqual([]);
  });
});

describe('collectApprovedExtensionIds', () => {
  it('keeps entries of disabled sources approved', () => {
    const mergedEntries = mergeMarketSourceRegistries(
      [createSource()],
      new Map<string, MarketSourceFetchOutcome>([
        [OFFICIAL_MARKET_SOURCE_ID, {
          status: 'ok',
          registry: createRegistry('a'),
          fetchedAt: 100,
        }],
      ]),
    ).entries;

    const approvedIds = collectApprovedExtensionIds(
      mergedEntries,
      [createCache(createRegistry('disabled-one'))],
    );

    expect(approvedIds.has('a')).toBe(true);
    expect(approvedIds.has('disabled-one')).toBe(true);
  });

  it('ignores missing caches', () => {
    const approvedIds = collectApprovedExtensionIds([], [undefined, createCache(createRegistry('b'))]);

    expect([...approvedIds]).toEqual(['b']);
  });
});

describe('resolveRegistryFetchedAt', () => {
  it('uses the oldest successful fetch time', () => {
    const fetchedAt = resolveRegistryFetchedAt(new Map<string, MarketSourceFetchOutcome>([
      ['a', {
        status: 'ok',
        registry: createRegistry('x'),
        fetchedAt: 500,
      }],
      ['b', {
        status: 'ok',
        registry: createRegistry('y'),
        fetchedAt: 200,
      }],
      ['c', {
        status: 'error',
        error: 'boom',
      }],
    ]));

    expect(fetchedAt).toBe(200);
  });

  it('returns null when every source failed', () => {
    const fetchedAt = resolveRegistryFetchedAt(new Map<string, MarketSourceFetchOutcome>([
      ['a', {
        status: 'error',
        error: 'boom',
      }],
    ]));

    expect(fetchedAt).toBeNull();
  });
});
