// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * 批次 25：shell namespace（手机 MTP/WPD）路径的缩略图分流判据。
 *
 * 判据必须和 Rust 侧 `dir_reader::shell_namespace::is_shell_namespace_path` 同源：
 * `::{...}` 或 `shell:` 前缀。手机里的条目**不是文件系统路径**，
 * 前端拿它去做 `convertFileSrc()` + `<video>` / `fetch()` 解码必然失败，
 * 所以两个 composable 都要先按这条判据分流。
 *
 * 判别性：改前必红 —— 两个 composable 还没有导出 `isShellNamespacePath`，
 * 这个 import 直接失败；改后必绿。
 */
import { describe, expect, it } from 'vitest';

import { isShellNamespacePath as imageIsShellNamespacePath } from '../use-image-thumbnails';
import { isShellNamespacePath as videoIsShellNamespacePath } from '../use-video-thumbnails';

const PHONE_IMAGE_PATH
  = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}\\{B725F130-47EF-101A-A5F1-02608C9EEBAC}\\{a-b-c}';
const PHONE_ROOT_PATH = '::{20D04FE0-3AEA-1069-A2D8-08002B30309D}';
const CONTROL_PANEL_PATH = 'shell:::{21EC2020-3AEA-1069-A2DD-08002B30309D}';
const USB_DRIVE_PATH = 'H:\\照片\\IMG_0001.jpg';
const LOCAL_PATH = 'C:\\Users\\me\\Pictures\\a.png';
const NAS_UNC_PATH = '\\\\nas\\share\\photo\\a.jpg';

describe('shell namespace 缩略图分流判据', () => {
  it.each([
    ['手机图片条目', PHONE_IMAGE_PATH, true],
    ['手机所在设备根', PHONE_ROOT_PATH, true],
    ['shell: 前缀路径', CONTROL_PANEL_PATH, true],
    ['U 盘（真实盘符）', USB_DRIVE_PATH, false],
    ['本地磁盘', LOCAL_PATH, false],
    ['NAS 的 UNC 路径', NAS_UNC_PATH, false],
  ])('%s => isShellNamespacePath = %s', (_label, path, expected) => {
    expect(imageIsShellNamespacePath(path)).toBe(expected);
    expect(videoIsShellNamespacePath(path)).toBe(expected);
  });

  it('图片与视频两条通道的判据完全一致', () => {
    for (const path of [
      PHONE_IMAGE_PATH,
      PHONE_ROOT_PATH,
      CONTROL_PANEL_PATH,
      USB_DRIVE_PATH,
      LOCAL_PATH,
      NAS_UNC_PATH,
    ]) {
      expect(imageIsShellNamespacePath(path)).toBe(videoIsShellNamespacePath(path));
    }
  });

  it('U 盘与 NAS 必须留在文件系统缩略图通道（回归保护）', () => {
    // 这两条是已经实测健康的链路（见 .superpowers\sdd\b25-h-control.txt），
    // 一旦被误分流，缩略图会整盘消失。
    expect(imageIsShellNamespacePath(USB_DRIVE_PATH)).toBe(false);
    expect(imageIsShellNamespacePath(NAS_UNC_PATH)).toBe(false);
  });
});
