// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  beforeEach, describe, expect, it, vi,
} from 'vitest';
import { createPinia, setActivePinia } from 'pinia';
import { ref } from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import { useContextMenuItems } from '@/modules/navigator/components/file-browser/composables/use-context-menu-items';
import { usePlatformStore } from '@/stores/runtime/platform';
import { LOCATIONS_VIRTUAL_PATH } from '@/utils/virtual-path-constants';

function createDirectoryEntry(path: string): DirEntry {
  return {
    name: path,
    path,
    is_file: false,
    is_dir: true,
    is_hidden: false,
    is_symlink: false,
    size: 0,
    item_count: null,
    created_time: 0,
    modified_time: 0,
    accessed_time: 0,
    ext: null,
    mime: null,
  };
}

describe('useContextMenuItems virtual locations', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    usePlatformStore().currentPlatform = 'windows';

    // 剪贴板 store → 用户设置 store → useTheme.init() 会调 window.matchMedia，
    // jsdom 没实现它。与仓库里其它用例同一套桩。
    Object.defineProperty(window, 'matchMedia', {
      configurable: true,
      writable: true,
      value: vi.fn(() => ({
        matches: false,
        addEventListener: vi.fn(),
        removeEventListener: vi.fn(),
      })),
    });
  });

  it('delegates visibility rules to the shared action visibility helper', () => {
    const selectedEntries = ref([createDirectoryEntry(LOCATIONS_VIRTUAL_PATH)]);
    const { isMenuActionVisible } = useContextMenuItems(selectedEntries);

    expect(isMenuActionVisible('toggle-favorite')).toBe(true);
    expect(isMenuActionVisible('delete')).toBe(false);
  });
});
