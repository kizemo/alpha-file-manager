// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';
import {
  applyGlobalSearchSingleSelection,
  applyGlobalSearchToggleSelection,
  findGlobalSearchResultIndex,
  getGlobalSearchDriveRoot,
  getGlobalSearchResultRowKey,
  groupGlobalSearchResults,
  resolveGlobalSearchRange,
} from '../global-search-result-rows';

function createEntry(path: string, overrides: Partial<DirEntry> = {}): DirEntry {
  const segments = path.split('/').filter(Boolean);

  return {
    name: segments[segments.length - 1] ?? path,
    ext: null,
    path,
    size: 0,
    item_count: null,
    modified_time: 0,
    accessed_time: 0,
    created_time: 0,
    mime: null,
    is_file: true,
    is_dir: false,
    is_symlink: false,
    is_hidden: false,
    ...overrides,
  };
}

describe('global-search-result-rows', () => {
  it('groups drive paths by drive letter and other paths by first segment', () => {
    expect(getGlobalSearchDriveRoot('C:/Users/Duanyi/notes.txt')).toBe('C:/');
    expect(getGlobalSearchDriveRoot('d:/data')).toBe('D:/');
    expect(getGlobalSearchDriveRoot('/home/user/docs')).toBe('/home');
    expect(getGlobalSearchDriveRoot('/')).toBe('/');
  });

  it('keeps entries of the same root together and sorts roots alphabetically', () => {
    const entries = [
      createEntry('D:/second.txt'),
      createEntry('C:/first.txt'),
      createEntry('C:/nested/second.txt'),
    ];

    const groups = groupGlobalSearchResults(entries);

    expect(groups.map(group => group.driveRoot)).toEqual(['C:/', 'D:/']);
    expect(groups[0].entries.map(entry => entry.path)).toEqual([
      'C:/first.txt',
      'C:/nested/second.txt',
    ]);
    expect(groups[1].entries.map(entry => entry.path)).toEqual(['D:/second.txt']);
  });

  it('uses the index as row key instead of a long path string', () => {
    expect(getGlobalSearchResultRowKey(0)).toBe('0');
    expect(getGlobalSearchResultRowKey(17)).toBe('17');
  });

  it('replaces the selection on a plain click', () => {
    const first = createEntry('C:/first.txt');
    const second = createEntry('C:/second.txt');

    expect(applyGlobalSearchSingleSelection(second)).toEqual([second]);
    expect(applyGlobalSearchSingleSelection(second)).not.toContain(first);
  });

  it('toggles an entry on ctrl click without dropping the other selections', () => {
    const first = createEntry('C:/first.txt');
    const second = createEntry('C:/second.txt');

    const afterAdd = applyGlobalSearchToggleSelection([first], second);

    expect(afterAdd.map(entry => entry.path)).toEqual(['C:/first.txt', 'C:/second.txt']);

    const afterRemove = applyGlobalSearchToggleSelection(afterAdd, second);

    expect(afterRemove.map(entry => entry.path)).toEqual(['C:/first.txt']);
  });

  it('finds the index of an entry by path', () => {
    const entries = [
      createEntry('C:/first.txt'),
      createEntry('C:/second.txt'),
    ];

    expect(findGlobalSearchResultIndex(entries, 'C:/second.txt')).toBe(1);
    expect(findGlobalSearchResultIndex(entries, 'C:/missing.txt')).toBe(-1);
  });

  it('resolves a shift range in both directions and clamps it to the list', () => {
    const entries = [
      createEntry('C:/a.txt'),
      createEntry('C:/b.txt'),
      createEntry('C:/c.txt'),
    ];

    expect(resolveGlobalSearchRange(entries, 0, 2).map(entry => entry.path)).toEqual([
      'C:/a.txt',
      'C:/b.txt',
      'C:/c.txt',
    ]);
    expect(resolveGlobalSearchRange(entries, 2, 0).map(entry => entry.path)).toEqual([
      'C:/a.txt',
      'C:/b.txt',
      'C:/c.txt',
    ]);
    expect(resolveGlobalSearchRange(entries, 1, 1).map(entry => entry.path)).toEqual(['C:/b.txt']);
    expect(resolveGlobalSearchRange(entries, -1, 1)).toEqual([]);
    expect(resolveGlobalSearchRange([], 0, 0)).toEqual([]);
  });
});
