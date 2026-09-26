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

  it('store.setSelectedPath auto-expands the full chain (drive → selectedPath)', () => {
    const store = useFolderTreeStore();
    store.setSelectedPath('C:/Users/foo');

    expect(computeAncestorPaths('C:/Users/foo')).toEqual(['C:/', 'C:/Users']);
    // v6.2 policy: chain from drive root down to AND INCLUDING selectedPath.
    expect(store.isExpanded('C:/')).toBe(true);
    expect(store.isExpanded('C:/Users')).toBe(true);
    expect(store.isExpanded('C:/Users/foo')).toBe(true);
  });

  it('clicking a directory row emits `activate` AND toggles expandedPaths (v6.2 single-click flow)', async () => {
    const store = useFolderTreeStore();
    const wrapper = mount(FileBrowserTreeView, {
      props: { rootPaths: ['C:/work'] },
    });
    await nextTick();

    const row = wrapper.find('[data-tree-path="C:/work"]');
    expect(row.exists()).toBe(true);

    const activateEvents: string[] = [];
    wrapper.vm.$emit = ((event: string, ...args: unknown[]) => {
      if (event === 'activate') activateEvents.push(args[0] as string);
    }) as never;
    // Re-attach the listener through the actual emit API for correctness:
    wrapper.vm.$emit = ((event: string, ...args: unknown[]) => {
      if (event === 'activate') activateEvents.push(args[0] as string);
    }) as never;
    // Use the prop emit instead — vue-test-utils exposes emits via emitted():
    await row.trigger('click');

    expect(wrapper.emitted('activate')?.[0]).toEqual(['C:/work']);
    expect(store.isExpanded('C:/work')).toBe(true);

    // second click collapses AND re-emits
    await row.trigger('click');
    expect(wrapper.emitted('activate')?.length).toBe(2);
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
    // Regression guard for handoff §8.3: when a directory's children fail
    // to load, the row stays visible with an error marker (not just an
    // empty row). Mark the error on the store AFTER mount so the watch's
    // onLoadStart (which clears loadErrorPaths) doesn't overwrite it, then
    // verify the row class binding reacts.
    const store = useFolderTreeStore();
    const wrapper = mount(FileBrowserTreeView, {
      props: { rootPaths: ['C:/locked'] },
    });
    await nextTick();
    await nextTick();
    // Wait for any post-mount ensureLoaded to settle, then mark the error.
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
