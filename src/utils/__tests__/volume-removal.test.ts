// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  collectVolumeCloseTargets,
  driveLetterOfPath,
  isPathOnDrive,
} from '@/utils/volume-removal';

type FakeTab = {
  id: string;
  path: string;
};

function tab(id: string, path: string): FakeTab {
  return {
    id,
    path,
  };
}

describe('driveLetterOfPath', () => {
  it.each([
    ['H:\\Users\\duanyi\\Desktop', 'H'],
    ['h:/Temp', 'H'],
    ['C:\\', 'C'],
  ])('reads the letter out of %s', (path, expected) => {
    expect(driveLetterOfPath(path)).toBe(expected);
  });

  it.each([
    ['\\\\nas\\share\\pub'],
    ['/home/duanyi/projects'],
    ['this PC / Documents'],
    [''],
    ['H'],
    ['HV:\\mixed'],
  ])('refuses to invent a letter for %s', (path) => {
    expect(driveLetterOfPath(path)).toBeNull();
  });

  it('tolerates null and undefined', () => {
    expect(driveLetterOfPath(undefined)).toBeNull();
    expect(driveLetterOfPath(null)).toBeNull();
  });
});

describe('isPathOnDrive', () => {
  it('matches regardless of letter case on either side', () => {
    expect(isPathOnDrive('H:\\Data', 'h')).toBe(true);
    expect(isPathOnDrive('h:/Data', 'H')).toBe(true);
    expect(isPathOnDrive(' C:\\Data ', 'C')).toBe(false);
  });

  it('does not treat a same-letter UNC or unix path as being on that drive', () => {
    expect(isPathOnDrive('\\\\H\\share', 'H')).toBe(false);
    expect(isPathOnDrive('/H/Data', 'H')).toBe(false);
  });

  it('returns false for an unusable target letter', () => {
    expect(isPathOnDrive('H:\\Data', '')).toBe(false);
    expect(isPathOnDrive('H:\\Data', 'HH')).toBe(false);
    expect(isPathOnDrive('H:\\Data', null)).toBe(false);
  });
});

describe('collectVolumeCloseTargets', () => {
  it('closes the whole group when every pane sits on the removed drive', () => {
    const tabGroups = [
      [tab('g1-t1', 'H:\\Docs'), tab('g1-t2', 'H:\\Music')],
      [tab('g2-t1', 'C:\\Windows')],
    ];

    expect(collectVolumeCloseTargets(tabGroups, 'H')).toEqual({
      groupIds: ['g1-t1'],
      tabIds: [],
    });
  });

  it('closes only the offending pane of a split group, leaving other panes alone', () => {
    const tabGroups = [
      [tab('mix-c', 'C:\\Projects'), tab('mix-h', 'H:\\Backup'), tab('mix-d', 'D:\\Media')],
    ];

    expect(collectVolumeCloseTargets(tabGroups, 'H')).toEqual({
      groupIds: [],
      tabIds: ['mix-h'],
    });
  });

  it('splits mixed groups and whole groups in one pass', () => {
    const tabGroups = [
      [tab('a1', 'H:\\One')],
      [tab('b1', 'C:\\x'), tab('b2', 'H:\\y')],
      [tab('c1', 'H:\\p'), tab('c2', 'H:\\q')],
      [tab('d1', 'D:\\z')],
    ];

    expect(collectVolumeCloseTargets(tabGroups, 'h')).toEqual({
      groupIds: ['a1', 'c1'],
      tabIds: ['b2'],
    });
  });

  it('returns nothing when no tab is on that drive', () => {
    const tabGroups = [[tab('a1', 'C:\\x')], [tab('b1', '\\\\nas\\share')]];

    expect(collectVolumeCloseTargets(tabGroups, 'H')).toEqual({
      groupIds: [],
      tabIds: [],
    });
  });

  it('skips empty and missing groups instead of handing back an id closeTabGroup would crash on', () => {
    const tabGroups = [[], undefined, null, [tab('a1', 'H:\\x')]];

    expect(collectVolumeCloseTargets(tabGroups, 'H')).toEqual({
      groupIds: ['a1'],
      tabIds: [],
    });
  });

  it('tolerates a missing tabGroups collection', () => {
    expect(collectVolumeCloseTargets(undefined, 'H')).toEqual({
      groupIds: [],
      tabIds: [],
    });
  });
});
