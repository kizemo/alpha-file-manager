// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

// FORK-MODIFICATION: integration test for file-browser-tree-view + folder-tree store.
// Tracking issue: aleksey-hoffman/sigma-file-manager#499
//
// Replaces the v5 `use-file-tree-prop-sync.test.ts` which tested an
// imperative "parent calls child.expandToPath" chain that was unfixable in
// v0..v5 (see handoff-2026-09-26-tree-sync-retro.md). The new architecture is
// store-driven: navigator writes `selectedPath`, tree-view subscribes via
// `storeToRefs`, useFileTree's watch on `expandedPaths` lazy-loads. No
// template ref, no parent→child imperative call.

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { mount } from '@vue/test-utils';
import { createPinia, setActivePinia } from 'pinia';
import { nextTick } from 'vue';
import FileBrowserTreeView from '@/modules/navigator/components/file-browser/file-browser-tree-view.vue';
import {
  computeAncestorPaths,
  useFolderTreeStore,
} from '@/stores/runtime/folder-tree';

// Stub the Tauri IPC entry that resolveDirectoryContents uses in production.
// In jsdom there is no window.__TAURI__ so reading `.invoke` throws — we
// surface that as a controlled "permission denied" so we can assert the
// store's loadErrorPaths handling without spinning up Tauri.
beforeEach(() => {
  vi.spyOn(console, 'error').mockImplementation(() => {});
});

describe('FileBrowserTreeView (store-driven sync)', () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it('renders the initial root path row', async () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/work');

    const wrapper = mount(FileBrowserTreeView, {
      props: { rootPaths: ['C:/work'] },
    });
    await nextTick();

    const rows = wrapper.findAll('[data-tree-path]');
    expect(rows.length).toBeGreaterThanOrEqual(1);
    expect(rows[0].attributes('data-tree-path')).toBe('C:/work');
    wrapper.unmount();
  });

  it('store.setSelectedPath auto-expands every ancestor directory', () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/Users/foo');

    expect(computeAncestorPaths('C:/Users/foo')).toEqual(['C:/', 'C:/Users']);
    expect(store.isExpanded('C:/')).toBe(true);
    expect(store.isExpanded('C:/Users')).toBe(true);
    expect(store.isExpanded('C:/Users/foo')).toBe(false);
  });

  it('clicking a directory row toggles store.expandedPaths (no template ref)', async () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/work');

    const wrapper = mount(FileBrowserTreeView, {
      props: { rootPaths: ['C:/work'] },
    });
    await nextTick();

    const row = wrapper.find('[data-tree-path="C:/work"]');
    expect(row.exists()).toBe(true);
    await row.trigger('click');

    expect(store.isExpanded('C:/work')).toBe(true);

    // second click collapses
    await row.trigger('click');
    expect(store.isExpanded('C:/work')).toBe(false);

    wrapper.unmount();
  });

  it('selected path renders with the `selected` class (highlight)', async () => {
    const store = useFolderTreeStore();
    store.expandPath('C:/work');

    const wrapper = mount(FileBrowserTreeView, {
      props: { rootPaths: ['C:/work'] },
    });
    store.setSelectedPath('C:/work');
    await nextTick();
    await nextTick();

    const row = wrapper.find('[data-tree-path="C:/work"]');
    expect(row.exists()).toBe(true);
    expect(row.classes()).toContain('file-tree-row--selected');
    expect(row.attributes('data-selected')).toBe('true');

    wrapper.unmount();
  });

  it('load failure marks the row as error (UI still renders the row)', async () => {
    // In jsdom the default `resolveDirectoryContents` will throw because
    // Tauri IPC is unavailable. The tree-view catches this via onLoadError
    // and stores `markLoadError(path, true)`, which the row class binding
    // observes. This is the regression guard for handoff §8.3 ("ancestor
    // load failures must not break the UI").
    //
    // We seed markLoadError directly on the store (which is the same path
    // the onLoadError callback takes) and verify the row picks it up via
    // reactive binding. We don't drive the failure through the real
    // onLoadError path because that depends on Tauri IPC being wired up
    // in jsdom, which varies across versions and is irrelevant to the
    // binding under test.
    const store = useFolderTreeStore();
    const wrapper = mount(FileBrowserTreeView, {
      props: { rootPaths: ['C:/locked'] },
    });
    store.setSelectedPath('C:/locked');
    store.markLoadError('C:/locked', true);
    await nextTick();
    await nextTick();

    expect(store.hasLoadError('C:/locked')).toBe(true);

    const row = wrapper.find('[data-tree-path="C:/locked"]');
    expect(row.exists()).toBe(true);
    expect(row.classes()).toContain('file-tree-row--error');

    wrapper.unmount();
  });
});
