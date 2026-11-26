// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: 批次 58 —— 「导出设置 / 导入设置」的数据格式单测。
//
// 三个刻意的取舍都在这里被钉住：
//   1. history 与 frequentItems **不导出**；
//   2. 凭据（credentialKey / proxy.password）**不导出**；
//   3. 导入时凭据用**本机**的，元数据用导入的。

import { describe, expect, it } from 'vitest';
import {
  buildSettingsTransferBundle,
  mergeImportedRemoteConnections,
  normalizeSettingsForExport,
  parseSettingsTransferBundle,
  SETTINGS_TRANSFER_FORMAT,
  SETTINGS_TRANSFER_VERSION,
} from '@/modules/settings/utils/settings-transfer';
import type { RemoteConnection } from '@/types/remote';
import type {
  SettingsTransferBundle,
  SettingsTransferParseResult,
} from '@/modules/settings/utils/settings-transfer';

function makeConnection(overrides: Record<string, unknown>): RemoteConnection {
  return {
    id: 'c1',
    name: '速豹堂',
    protocol: 'sftp',
    host: 'example.com',
    port: 22,
    username: 'user',
    initialPath: '/',
    proxy: { kind: 'none' },
    ...overrides,
  } as unknown as RemoteConnection;
}

/**
 * 把解析结果收敛成 bundle。
 *
 * **不用 `if (result.ok) { expect(...) }`** —— 那种写法会被 `vitest/no-conditional-expect`
 * 判红（条件分支里的断言，分支没走到就被静默跳过，等于没有断言）。
 */
function expectOk(result: SettingsTransferParseResult): SettingsTransferBundle {
  expect(result.ok).toBe(true);

  if (!result.ok) {
    throw new Error(`parse failed: ${result.reason}`);
  }

  return result.bundle;
}

describe('导出的设置集合归一化', () => {
  it('去掉 schema 版本键', () => {
    expect(normalizeSettingsForExport({
      __schemaVersion: 3,
      theme: 'afm-warm-light',
    })).toEqual({ theme: 'afm-warm-light' });
  });

  it('非对象一律归一成空对象', () => {
    expect(normalizeSettingsForExport(null)).toEqual({});
    expect(normalizeSettingsForExport('nope')).toEqual({});
  });
});

describe('导出包构造', () => {
  it('不含 history 与 frequentItems', () => {
    const bundle = buildSettingsTransferBundle({
      stats: {
        favorites: [
          {
            path: 'C:/a',
            addedAt: 1,
            isFile: false,
            categoryId: null,
          },
        ],
        history: [{
          path: 'C:/h',
          openedAt: 1,
        }],
        frequentItems: [{
          path: 'C:/f',
          openCount: 3,
        }],
      },
    });

    expect(Object.keys(bundle)).not.toContain('history');
    expect(Object.keys(bundle)).not.toContain('frequentItems');
    expect(bundle.favorites).toHaveLength(1);
  });

  it('分类 / 标签 / 标记一并导出', () => {
    const bundle = buildSettingsTransferBundle({
      stats: {
        favoriteCategories: [{
          id: 'g1',
          name: '工作',
          createdAt: 1,
        }],
        tags: [{
          id: 't1',
          name: '重要',
          color: '#ff0000',
        }],
        taggedItems: [{
          path: 'C:/a',
          tagIds: ['t1'],
          addedAt: 1,
          isFile: false,
        }],
      },
    });

    expect(bundle.favoriteCategories).toHaveLength(1);
    expect(bundle.tags).toHaveLength(1);
    expect(bundle.taggedItems).toHaveLength(1);
  });

  it('远程连接不导出凭据', () => {
    const bundle = buildSettingsTransferBundle({
      remoteConnections: {
        connections: [
          makeConnection({
            credentialKey: 'dpapi-key',
            proxy: {
              kind: 'http',
              password: 'secret',
            },
          }),
        ],
      },
    });

    const connection = bundle.remoteConnections[0] as unknown as Record<string, unknown>;

    expect(connection.credentialKey).toBeUndefined();
    expect((connection.proxy as Record<string, unknown>).password).toBeUndefined();
  });

  it('格式与版本号是固定值', () => {
    const bundle = buildSettingsTransferBundle({});

    expect(bundle.format).toBe(SETTINGS_TRANSFER_FORMAT);
    expect(bundle.version).toBe(SETTINGS_TRANSFER_VERSION);
  });
});

describe('导入包解析', () => {
  it('不是对象一律判 structure', () => {
    expect(parseSettingsTransferBundle('nope')).toEqual({
      ok: false,
      reason: 'structure',
    });
    expect(parseSettingsTransferBundle(null)).toEqual({
      ok: false,
      reason: 'structure',
    });
  });

  it('格式标识不对判 format', () => {
    expect(parseSettingsTransferBundle({
      format: 'other',
      version: 1,
    }))
      .toEqual({
        ok: false,
        reason: 'format',
      });
  });

  it('版本更高判 version', () => {
    expect(parseSettingsTransferBundle({
      format: SETTINGS_TRANSFER_FORMAT,
      version: SETTINGS_TRANSFER_VERSION + 1,
    })).toEqual({
      ok: false,
      reason: 'version',
    });
  });

  it('正常文件能解析出收藏与分类', () => {
    const bundle = expectOk(parseSettingsTransferBundle({
      format: SETTINGS_TRANSFER_FORMAT,
      version: SETTINGS_TRANSFER_VERSION,
      favorites: [{
        path: 'C:/a',
        addedAt: 1,
        isFile: false,
        categoryId: null,
      }],
      favoriteCategories: [{
        id: 'g1',
        name: '工作',
        createdAt: 1,
      }],
    }));

    expect(bundle.favorites).toHaveLength(1);
    expect(bundle.favoriteCategories).toHaveLength(1);
  });

  it('导入包里没有 history / frequentItems 也不会出错', () => {
    const bundle = expectOk(parseSettingsTransferBundle({
      format: SETTINGS_TRANSFER_FORMAT,
      version: SETTINGS_TRANSFER_VERSION,
      history: [{
        path: 'C:/h',
        openedAt: 1,
      }],
      frequentItems: [{
        path: 'C:/f',
        openCount: 3,
      }],
    }));

    expect(Object.keys(bundle)).not.toContain('history');
    expect(Object.keys(bundle)).not.toContain('frequentItems');
  });
});

describe('远程连接合并', () => {
  it('同 id 沿用本机凭据', () => {
    const merged = mergeImportedRemoteConnections(
      [makeConnection({ credentialKey: undefined })],
      [makeConnection({ credentialKey: 'local-key' })],
    );

    expect(merged.connections[0].credentialKey).toBe('local-key');
    expect(merged.needsCredential).toEqual([]);
  });

  it('本机没有的连接列入 needsCredential', () => {
    const merged = mergeImportedRemoteConnections(
      [makeConnection({
        id: 'b',
        credentialKey: undefined,
      })],
      [],
    );

    expect(merged.needsCredential).toEqual(['b']);
  });

  it('本机 proxy 密码优先于导入包', () => {
    const merged = mergeImportedRemoteConnections(
      [makeConnection({ proxy: { kind: 'http' } })],
      [makeConnection({
        proxy: {
          kind: 'http',
          password: 'local-pass',
        },
      })],
    );

    const proxy = merged.connections[0].proxy as unknown as Record<string, unknown>;

    expect(proxy.password).toBe('local-pass');
  });
});
