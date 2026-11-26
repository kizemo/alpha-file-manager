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

// FORK-MODIFICATION 2026-11-19（批次 23）：shell 命名空间路径（手机 / MTP）。
//
// 这批路径**不是文件系统路径**：`\` 是 shell 语义、没有可切分的层级、
// 整体是一个不透明标识符。任何「规范化」动作都会把它改坏 ——
// 改坏之后后端认不出来，用户双击手机就只会看到「跳到位置页」。
describe('shell 命名空间路径', () => {
  const devicePath = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9&pid_2765';

  it('normalizePath 与 canonicalizePath 原样返回', () => {
    expect(normalizePath(devicePath)).toBe(devicePath);
    expect(canonicalizePath(devicePath)).toBe(devicePath);
  });

  it('前缀判定大小写不敏感', () => {
    expect(normalizePath('SHELL:Downloads')).toBe('SHELL:Downloads');
    expect(normalizePath('::{20D04FE0-3AEA-1069-A2D8-08002B30309D}'))
      .toBe('::{20D04FE0-3AEA-1069-A2D8-08002B30309D}');
  });

  it('不切段、不给末段名', () => {
    expect(getPathSegments(devicePath)).toEqual([]);
    expect(getPathLeafName(devicePath)).toBe('');
  });

  it('上一层是前缀截断，没有更上一级时为 null', () => {
    // 设备根没有更上一级（`?` 是解析名标记，不是目录层级）。
    // 照字面截断会得到 `::{GUID}\?` 这种后端解析不出来的垃圾路径。
    expect(getParentPath('::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9')).toBeNull();
    expect(getParentPath('::{20D04FE0-3AEA-1069-A2D8-08002B30309D}')).toBeNull();

    // 设备里的下一层：上一层是设备根本身。
    expect(getParentPath(
      '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9\\#{6ac27878-a6fa-4155-ba85-f98f491d4f33}\\SID-{10001}',
    )).toBe(
      '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\?\\usb#vid_22d9\\#{6ac27878-a6fa-4155-ba85-f98f491d4f33}',
    );
  });

  it('显示名是短标签，不是 ::{...} 整串', () => {
    expect(getPathDisplayName(devicePath)).toBe('Device');
    expect(getPathDisplayName(devicePath, key => `t:${key}`)).toBe('t:fileBrowser.device');
    expect(getPathDisplayName(devicePath)).not.toContain('::{');
  });

  it('显示值原样保留（复制出去还能被后端认出来）', () => {
    expect(getPathDisplayValue(devicePath)).toBe(devicePath);
  });

  it('原生分隔符转换不碰 shell 路径', () => {
    expect(toNativePathSeparators(devicePath, true)).toBe(devicePath);
  });

  it('不会被误判成盘符根、UNC 路径或 Unix 根', () => {
    expect(isWindowsDriveRootPath(devicePath)).toBe(false);
    expect(isUncShareRootPath(devicePath)).toBe(false);
    expect(isUnixFilesystemRoot(devicePath)).toBe(false);
    expect(isUncPath(devicePath)).toBe(false);
  });

  it('同路径与子路径判定走 shell 的反斜杠前缀', () => {
    expect(isSameOrDescendantPath(devicePath, devicePath)).toBe(true);
    expect(isSameOrDescendantPath(`${devicePath}\\DCIM`, devicePath)).toBe(true);
    expect(isSameOrDescendantPath('C:/Users', devicePath)).toBe(false);
  });
});
