// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import {
  nextTick, ref, watch, type ComponentPublicInstance, type Ref,
} from 'vue';
import type { DirEntry } from '@/types/dir-entry';
import normalizePath from '@/utils/normalize-path';

type PendingFocusRequest
  = | {
    type: 'path';
    targetPath: string;
    path: string;
    /**
     * FORK-MODIFICATION 2026-10-27：刷新后要不要把 DOM 焦点也带走。
     * 就地改名走这条路时**不要**抢焦点 —— 焦点可能已经在下一个条目的输入框里了。
     */
    shouldFocus?: boolean;
  }
  | {
    type: 'diff';
    targetPath: string;
    previousPaths: Set<string>;
  };

export function useFileBrowserFocus(options: {
  entries: Ref<DirEntry[]>;
  pendingFocusRequest: Ref<PendingFocusRequest | null>;
  currentPath: Ref<string>;
  selectEntryByPath: (path: string) => boolean;
  clearPendingFocusRequest: () => void;
  scrollToPath?: (path: string, align?: ScrollLogicalPosition) => Promise<boolean>;
  getEntryElement?: (path: string) => HTMLElement | null;
}) {
  const entriesContainerRef = ref<HTMLElement | null>(null);

  function setEntriesContainerRef(element: Element | ComponentPublicInstance | null) {
    entriesContainerRef.value = element instanceof HTMLElement ? element : null;
  }

  function getEntryElement(path: string): HTMLElement | null {
    const virtualEntryElement = options.getEntryElement?.(path);

    if (virtualEntryElement) {
      return virtualEntryElement;
    }

    const container = entriesContainerRef.value;

    if (!container) {
      return null;
    }

    const escapedPath = typeof CSS !== 'undefined' && CSS.escape
      ? CSS.escape(path)
      : path.replace(/"/g, '\\"');

    return container.querySelector<HTMLElement>(`[data-entry-path="${escapedPath}"]`);
  }

  async function focusEntryInView(path: string): Promise<boolean> {
    await options.scrollToPath?.(path, 'center');
    await nextTick();
    const entryElement = getEntryElement(path);

    if (!entryElement) {
      return false;
    }

    entryElement.scrollIntoView({
      block: 'center',
      inline: 'nearest',
    });
    entryElement.focus({
      preventScroll: true,
    });
    return true;
  }

  async function attemptFocusPending() {
    const request = options.pendingFocusRequest.value;

    if (!request) {
      return;
    }

    if (normalizePath(options.currentPath.value) !== normalizePath(request.targetPath)) {
      return;
    }

    if (request.type === 'path') {
      const normalizedWanted = normalizePath(request.path);
      const targetEntry = options.entries.value.find(
        entry => normalizePath(entry.path) === normalizedWanted,
      );

      if (!targetEntry) {
        return;
      }

      if (!options.selectEntryByPath(targetEntry.path)) {
        return;
      }

      // FORK-MODIFICATION 2026-10-27：就地改名只要「选中」，不要抢 DOM 焦点 ——
      // 焦点可能已经在下一个条目的输入框里了，抢过去会把输入打断。
      if (request.shouldFocus === false) {
        options.clearPendingFocusRequest();
        return;
      }

      const didFocus = await focusEntryInView(targetEntry.path);

      if (didFocus) {
        options.clearPendingFocusRequest();
      }

      return;
    }

    const newEntry = options.entries.value.find(entry => !request.previousPaths.has(entry.path));

    if (!newEntry) {
      return;
    }

    if (!options.selectEntryByPath(newEntry.path)) {
      return;
    }

    const didFocus = await focusEntryInView(newEntry.path);

    if (didFocus) {
      options.clearPendingFocusRequest();
    }
  }

  watch([options.entries, options.pendingFocusRequest, options.currentPath], () => {
    void attemptFocusPending();
  });

  return {
    entriesContainerRef,
    setEntriesContainerRef,
  };
}
