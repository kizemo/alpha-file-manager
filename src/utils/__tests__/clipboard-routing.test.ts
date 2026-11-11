// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import {
  clipboardHoldsRemoteItems,
  shouldSyncPathsToSystemClipboard,
} from '@/utils/clipboard-routing';

/**
 * T9（2026-11-11）：网盘条目 Ctrl+C / Ctrl+V 无效的**真正**断点。
 *
 * 门禁放行了 `copy` 还不够 —— 真机取证（`t8-item9-verdict.md` 步骤 3/4）显示：
 *
 * 1. `clipboard.ts::setClipboard` → `syncToSystemClipboard()` 会把
 *    `sfm://remote/...` 当**本地路径**喂给 `set_system_clipboard_files`。
 *    系统剪贴板 CF_HDROP 只认真实绝对路径 ⇒ 这次写入注定无效，
 *    但应用**没有任何提示**，用户以为复制成功了。
 * 2. `use-dir-entry-actions.ts::pasteItems` 先读系统剪贴板。
 *    系统里只要残留着上一次真实复制的本地文件（实测残留
 *    `C:\downloads\谷歌系浏览器专用-猫咪书签清理1.46.crx`），
 *    它就会走「外部粘贴」分支、**忽略应用内剪贴板里的远程条目**
 *    ⇒ Ctrl+V 粘出来的是那个 crx，而不是网盘文件。
 *
 * 所以路由判定必须是**一眼可判**的两条纯函数，而不是埋在 store 里靠 mock 才能测的逻辑。
 */

// 注意：Windows 路径字面量里的反斜杠必须成对转义（`\\`）。
// 单写 `\x` 会让 esbuild 直接 PARSE_ERROR（本轮踩过）。
const LOCAL_BOOK = 'C:\\downloads\\books.pdf';
const LOCAL_IMAGE = 'D:\\temp\\x.png';
const REMOTE_IMAGE = 'sfm://remote/conn-a/pub/x.jpg';

describe('clipboard-routing', () => {
  describe('shouldSyncPathsToSystemClipboard', () => {
    it('refuses to write remote paths into the system clipboard', () => {
      expect(shouldSyncPathsToSystemClipboard([REMOTE_IMAGE])).toBe(false);
    });

    it('refuses on a mixed selection: remote wins, do not half-sync', () => {
      expect(shouldSyncPathsToSystemClipboard([LOCAL_BOOK, REMOTE_IMAGE])).toBe(false);
    });

    it('keeps syncing pure local selections', () => {
      expect(shouldSyncPathsToSystemClipboard([LOCAL_BOOK, LOCAL_IMAGE])).toBe(true);
    });

    it('does not sync an empty selection', () => {
      expect(shouldSyncPathsToSystemClipboard([])).toBe(false);
    });
  });

  describe('clipboardHoldsRemoteItems', () => {
    it('detects a remote item among clipboard items', () => {
      expect(clipboardHoldsRemoteItems([{ path: LOCAL_BOOK }, { path: REMOTE_IMAGE }])).toBe(true);
    });

    it('returns false for a purely local selection', () => {
      expect(clipboardHoldsRemoteItems([{ path: LOCAL_BOOK }])).toBe(false);
    });

    it('returns false when the clipboard is empty', () => {
      expect(clipboardHoldsRemoteItems([])).toBe(false);
    });
  });
});
