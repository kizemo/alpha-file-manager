// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import normalizePath, {
  canonicalizePath,
  getAddressBarSegments,
  getPathDisplayName,
  getPathDisplayValue,
  getNativePathDisplayValue,
  getParentPath,
  getPathLeafName,
  getPathSegments,
  isSameOrDescendantPath,
  isUncShareRootPath,
  isUnixFilesystemRoot,
  isWindowsDriveRootPath,
  isUncPath,
  isWslPath,
  isWslHostRootUncPath,
  joinPath,
  stripTrailingSlashesPreservingRoot,
  toNativePathSeparators,
} from '@/utils/normalize-path';

describe('normalizePath', () => {
  it('normalizes Windows separators', () => {
    expect(normalizePath('C:\\Users\\aleks\\Documents')).toBe('C:/Users/aleks/Documents');
  });

  it('detects UNC paths', () => {
    expect(isUncPath('\\\\wsl.localhost\\Ubuntu')).toBe(true);
    expect(isUncPath('C:/Users/aleks')).toBe(false);
  });

  it('detects WSL UNC paths', () => {
    expect(isWslPath('//wsl.localhost/Ubuntu/home')).toBe(true);
    expect(isWslPath('\\\\wsl.localhost\\Ubuntu')).toBe(true);
    expect(isWslPath('//wsl$/Ubuntu')).toBe(true);
    expect(isWslPath('//other-server/share')).toBe(false);
    expect(isWslPath('C:/Users/aleks')).toBe(false);
  });

  it('detects WSL host root UNC paths', () => {
    expect(isWslHostRootUncPath('//wsl.localhost')).toBe(true);
    expect(isWslHostRootUncPath('//wsl.localhost/')).toBe(true);
    expect(isWslHostRootUncPath('//wsl$')).toBe(true);
    expect(isWslHostRootUncPath('//wsl.localhost/Ubuntu')).toBe(false);
    expect(isWslHostRootUncPath('//other-server')).toBe(false);
  });

  it('detects Windows drive roots', () => {
    expect(isWindowsDriveRootPath('C:/')).toBe(true);
    expect(isWindowsDriveRootPath('C:')).toBe(true);
    expect(isWindowsDriveRootPath('C:/Users')).toBe(false);
  });

  it('detects UNC share roots', () => {
    expect(isUncShareRootPath('//wsl.localhost/Ubuntu-24.04/')).toBe(true);
    expect(isUncShareRootPath('//wsl.localhost/Ubuntu-24.04/home')).toBe(false);
  });

  it('preserves unix filesystem root when canonicalizing', () => {
    expect(canonicalizePath('/')).toBe('/');
    expect(canonicalizePath('///')).toBe('/');
    expect(canonicalizePath('/home/')).toBe('/home');
    expect(canonicalizePath('')).toBe('');
    expect(stripTrailingSlashesPreservingRoot('/')).toBe('/');
    expect(isUnixFilesystemRoot('/')).toBe(true);
    expect(isUnixFilesystemRoot('/home')).toBe(false);
  });

  it('extracts path segments from UNC paths', () => {
    expect(getPathSegments('//wsl.localhost/Ubuntu/home')).toEqual([
      'wsl.localhost',
      'Ubuntu',
      'home',
    ]);
  });

  it('keeps root-aware address-bar segments and parent behavior', () => {
    expect(getPathSegments('/')).toEqual([]);
    expect(getAddressBarSegments('/')).toEqual(['/']);
    expect(getPathLeafName('/')).toBe('/');
    expect(getParentPath('/')).toBeNull();
    expect(getParentPath('/home')).toBe('/');
  });

  it('joins and detects descendants without inventing // for root', () => {
    expect(joinPath('/', 'home')).toBe('/home');
    expect(joinPath('/home/', 'user')).toBe('/home/user');
    expect(isSameOrDescendantPath('/home', '/')).toBe(true);
    expect(isSameOrDescendantPath('/', '/')).toBe(true);
    expect(isSameOrDescendantPath('/tmp', '/home')).toBe(false);
  });

  it('returns the leaf name for UNC roots and child paths', () => {
    expect(getPathLeafName('//wsl.localhost')).toBe('wsl.localhost');
    expect(getPathLeafName('//wsl.localhost/Ubuntu')).toBe('Ubuntu');
  });

  it('returns parents for UNC paths without dropping the UNC prefix', () => {
    expect(getParentPath('//wsl.localhost')).toBeNull();
    expect(getParentPath('//wsl.localhost/Ubuntu')).toBe('//wsl.localhost');
    expect(getParentPath('//wsl.localhost/Ubuntu/home')).toBe('//wsl.localhost/Ubuntu');
  });

  it('returns parents for local Windows paths', () => {
    expect(getParentPath('C:/Users/aleks/Documents')).toBe('C:/Users/aleks');
    expect(getParentPath('C:/Users')).toBe('C:/');
  });

  it('formats display names consistently for root paths', () => {
    expect(getPathDisplayName('C:/')).toBe('C:');
    expect(getPathDisplayName('/')).toBe('/');
    expect(getPathDisplayName('//wsl.localhost/Ubuntu-24.04/')).toBe('Ubuntu-24.04');
  });

  it('formats the virtual locations path with and without translation', () => {
    expect(getPathDisplayName('sfm://locations')).toBe('Locations');
    expect(getPathDisplayName('sfm://locations', key => `t:${key}`)).toBe('t:locations');
  });

  it('formats display path values for root paths', () => {
    expect(getPathDisplayValue('C:/')).toBe('C:');
    expect(getPathDisplayValue('/')).toBe('/');
    expect(getPathDisplayValue('//wsl.localhost/Ubuntu-24.04/')).toBe('//wsl.localhost/Ubuntu-24.04');
    expect(getPathDisplayValue('C:/Users/aleks')).toBe('C:/Users/aleks');
  });
});

describe('toNativePathSeparators', () => {
  it('converts Windows paths to backslash separators', () => {
    expect(toNativePathSeparators('C:/Users/aleks/Documents', true)).toBe('C:\\Users\\aleks\\Documents');
  });

  it('keeps non-Windows platforms untouched', () => {
    expect(toNativePathSeparators('/Users/aleks/Documents', false)).toBe('/Users/aleks/Documents');
    expect(toNativePathSeparators('C:/Users/aleks', false)).toBe('C:/Users/aleks');
  });

  it('keeps UNC paths but switches to the native separator', () => {
    expect(toNativePathSeparators('//wsl.localhost/Ubuntu/home', true)).toBe('\\\\wsl.localhost\\Ubuntu\\home');
  });

  it('keeps remote and virtual protocol paths untouched', () => {
    expect(toNativePathSeparators('sfm://remote/c/x', true)).toBe('sfm://remote/c/x');
    expect(toNativePathSeparators('sfm://locations', true)).toBe('sfm://locations');
  });

  it('keeps empty paths empty', () => {
    expect(toNativePathSeparators('', true)).toBe('');
  });
});

describe('getNativePathDisplayValue', () => {
  it('formats drive roots and paths with native separators', () => {
    expect(getNativePathDisplayValue('C:/', true)).toBe('C:');
    expect(getNativePathDisplayValue('C:/Users/aleks', true)).toBe('C:\\Users\\aleks');
  });

  it('keeps posix separators on non-Windows platforms', () => {
    expect(getNativePathDisplayValue('/Users/aleks', false)).toBe('/Users/aleks');
  });
});
