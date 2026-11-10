<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
-->

# Toolbar Tree Toggle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Tree button to the navigator toolbar's layout-switch dropdown so users can switch to tree view from the UI.

**Architecture:** Reuse the existing `setLayout()` + `persistScopedPatch()` pipeline. Add a third button to the layout-row in the dropdown menu (parallel to list/grid), wired through the same path. Extend the type union to include `'tree'` everywhere it was hardcoded to `'list' | 'grid'`, fixing a Phase 2 latent bug where folder-scoped tree settings would silently fall back to list.

**Tech Stack:** Vue 3 + `<script setup>`, TypeScript, vitest, lucide-vue-next icons, vue-i18n

**Spec:** `docs/superpowers/specs/2026-09-25-toolbar-tree-toggle-design.md`

## Global Constraints

- Branch: `feat/tree-view-toolbar` based on `feat/tree-view` HEAD (`49c2ea2e`); do NOT switch branches mid-task.
- i18n: en.json is the source of truth. Other locales are synced manually via `npm run sync:i18n` (runs `scripts/sync-i18n.js`); a post-edit hook may also auto-trigger after en.json changes.
- Test command: `npx vitest run src/modules/navigator/` from `sigma-file-manager/`.
- Typecheck command: `npx vue-tsc --noEmit -p tsconfig.json` from `sigma-file-manager/`.
- Commit message prefix: `feat(navigator):` for behavior changes, `chore(i18n):` for locale-only changes.
- Do NOT run `npm run tauri:build`.
- Do NOT push or open a PR.
- Workspace cwd for all commands: `F:\soft\00selfmade\filemanager\sigma-file-manager\`.

## File Structure

This change touches 5 files (one new test entry, four modifications). No new files are created. Each file has a single, narrow responsibility for this change:

- `src/types/user-settings.ts` — type definitions only; extend union types
- `src/modules/navigator/utils/resolve-navigator-folder-settings.ts` — pure functions mapping layout names; add `tree` branch
- `src/modules/navigator/utils/__tests__/resolve-navigator-folder-settings.test.ts` — existing test suite; append tree cases
- `src/modules/navigator/components/navigator-toolbar-actions/navigator-toolbar-actions.vue` — UI; add button + extend local `LayoutType`
- `src/localization/messages/en.json` — translation strings; add `"tree"` key

---

## Task 0: Set up working branch

**Files:** none (git only)

**Interfaces:** none

- [ ] **Step 1: Decide whether to branch off `feat/tree-view` or stay on it**

Two options, pick one:
- **Option A (recommended per handoff):** create `feat/tree-view-toolbar` from current HEAD.
- **Option B (simpler):** stay on `feat/tree-view`; all commits land directly on it.

If Option A:
```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git checkout -b feat/tree-view-toolbar
```

If Option B: skip to Task 1.

---

## Task 1: Extend `NavigatorLayout` global type to include `'tree'`

**Files:**
- Modify: `sigma-file-manager/src/types/user-settings.ts:392-396`

**Interfaces:**
- Consumes: nothing (pure type definition)
- Produces: `NavigatorLayout['type']['name']` now includes `'tree'`; `NavigatorLayout['type']['title']` now includes `'treeLayout'`

Phase 2 commit `e8f663b3` extended the local `getLayoutForPath` return type but missed the global `NavigatorLayout` type, so writing `'tree'` to `navigator.layout.type.name` is currently a TypeScript error. Fix it now.

- [ ] **Step 1: Edit `src/types/user-settings.ts` lines 394-395**

Change:
```ts
    title: 'compactListLayout' | 'listLayout' | 'gridLayout';
    name: 'compact-list' | 'list' | 'grid';
```
To:
```ts
    title: 'compactListLayout' | 'listLayout' | 'gridLayout' | 'treeLayout';
    name: 'compact-list' | 'list' | 'grid' | 'tree';
```

- [ ] **Step 2: Run typecheck to verify zero errors**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vue-tsc --noEmit -p tsconfig.json`
Expected: 0 errors. Other call sites that use `navigator.layout.type.name` should still narrow correctly because adding `'tree'` widens the union.

- [ ] **Step 3: Commit**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add src/types/user-settings.ts
git commit -m "feat(navigator): extend NavigatorLayout type to include 'tree'"
```

---

## Task 2: Extend `NavigatorFolderLayoutName` to include `'tree'`

**Files:**
- Modify: `sigma-file-manager/src/types/user-settings.ts:345`

**Interfaces:**
- Consumes: nothing
- Produces: `NavigatorFolderLayoutName` = `'list' | 'grid' | 'tree'`

- [ ] **Step 1: Edit `src/types/user-settings.ts` line 345**

Change:
```ts
export type NavigatorFolderLayoutName = 'list' | 'grid';
```
To:
```ts
export type NavigatorFolderLayoutName = 'list' | 'grid' | 'tree';
```

- [ ] **Step 2: Run typecheck**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vue-tsc --noEmit -p tsconfig.json`
Expected: 0 errors. The wider union does not break existing consumers.

- [ ] **Step 3: Commit**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add src/types/user-settings.ts
git commit -m "feat(navigator): extend NavigatorFolderLayoutName to include 'tree'"
```

---

## Task 3: Add failing tests for tree in `resolve-navigator-folder-settings`

**Files:**
- Modify: `sigma-file-manager/src/modules/navigator/utils/__tests__/resolve-navigator-folder-settings.test.ts` (append)

**Interfaces:**
- Consumes: existing `createNavigator()` and `createFolderSettings()` factory helpers (lines 18-88)
- Produces: three new test cases asserting `'tree'` flows through `getNavigatorFolderLayoutName`, the snapshot, and the folder-settings merge

- [ ] **Step 1: Append failing test cases to the existing `describe` block**

Add these tests inside the existing `describe('resolve navigator folder settings', ...)` block, after the last `it(...)`:

```ts
  it('returns tree layout when global navigator is set to tree', () => {
    const navigator = createNavigator({
      layout: {
        type: { title: 'treeLayout', name: 'tree' },
        dirItemOptions: {
          title: { height: 32 },
          directory: { height: 48 },
          file: { height: 48 },
        },
      },
    });

    expect(createNavigatorFolderSettingsSnapshot(navigator)).toEqual(
      expect.objectContaining({ layout: 'tree' }),
    );
  });

  it('resolves a folder-scoped tree layout override', () => {
    const folderSettings = createFolderSettings({ layout: 'tree' });
    const navigator = createNavigator({
      layout: {
        type: { title: 'listLayout', name: 'list' },
        dirItemOptions: {
          title: { height: 32 },
          directory: { height: 48 },
          file: { height: 48 },
        },
      },
      folderSettings: {
        'C:/Users/aleks/Documents': folderSettings,
      },
    });

    expect(resolveNavigatorFolderSettings(navigator, 'C:/Users/aleks/Documents')).toEqual(
      expect.objectContaining({ layout: 'tree' }),
    );
  });

  it('falls back to the global tree layout when folder setting is missing', () => {
    const navigator = createNavigator({
      layout: {
        type: { title: 'treeLayout', name: 'tree' },
        dirItemOptions: {
          title: { height: 32 },
          directory: { height: 48 },
          file: { height: 48 },
        },
      },
    });

    expect(resolveNavigatorFolderSettings(navigator, 'C:/Users/aleks/Documents')).toEqual(
      expect.objectContaining({ layout: 'tree' }),
    );
  });
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vitest run src/modules/navigator/utils/__tests__/resolve-navigator-folder-settings.test.ts`
Expected: 3 new tests FAIL. Failure modes are TypeScript compile error (`'tree'` not assignable to `NavigatorFolderLayoutName`) or runtime assertion failure (`layout: 'list'` instead of `'tree'`).

- [ ] **Step 3: Commit (the failing tests)**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add src/modules/navigator/utils/__tests__/resolve-navigator-folder-settings.test.ts
git commit -m "test(navigator): cover tree layout in folder settings resolution"
```

---

## Task 4: Implement tree branches in `resolve-navigator-folder-settings.ts`

**Files:**
- Modify: `sigma-file-manager/src/modules/navigator/utils/resolve-navigator-folder-settings.ts:25-58`

**Interfaces:**
- Consumes: `NavigatorFolderLayoutName` (now `'list' | 'grid' | 'tree'`) and `NavigatorLayout['type']['name']` (now `'compact-list' | 'list' | 'grid' | 'tree'`)
- Produces: `getNavigatorFolderLayoutName('tree')` returns `'tree'`; `toNavigatorFolderLayoutType('tree')` returns `{ title: 'treeLayout', name: 'tree' }`; `isNavigatorFolderLayoutName('tree')` returns `true`

- [ ] **Step 1: Edit `getNavigatorFolderLayoutName` (lines 25-29)**

Change:
```ts
export function getNavigatorFolderLayoutName(
  layoutName: NavigatorLayout['type']['name'],
): NavigatorFolderLayoutName {
  return layoutName === 'grid' ? 'grid' : 'list';
}
```
To:
```ts
export function getNavigatorFolderLayoutName(
  layoutName: NavigatorLayout['type']['name'],
): NavigatorFolderLayoutName {
  if (layoutName === 'grid') return 'grid';
  if (layoutName === 'tree') return 'tree';
  return 'list';
}
```

- [ ] **Step 2: Edit `toNavigatorFolderLayoutType` (lines 31-38)**

Change:
```ts
export function toNavigatorFolderLayoutType(
  layoutName: NavigatorFolderLayoutName,
): NavigatorLayout['type'] {
  return {
    title: layoutName === 'grid' ? 'gridLayout' : 'listLayout',
    name: layoutName,
  };
}
```
To:
```ts
export function toNavigatorFolderLayoutType(
  layoutName: NavigatorFolderLayoutName,
): NavigatorLayout['type'] {
  if (layoutName === 'grid') return { title: 'gridLayout', name: 'grid' };
  if (layoutName === 'tree') return { title: 'treeLayout', name: 'tree' };
  return { title: 'listLayout', name: 'list' };
}
```

- [ ] **Step 3: Edit `isNavigatorFolderLayoutName` (lines 44-46)**

Change:
```ts
function isNavigatorFolderLayoutName(value: unknown): value is NavigatorFolderLayoutName {
  return value === 'list' || value === 'grid';
}
```
To:
```ts
function isNavigatorFolderLayoutName(value: unknown): value is NavigatorFolderLayoutName {
  return value === 'list' || value === 'grid' || value === 'tree';
}
```

- [ ] **Step 4: Leave `resolveFolderLayoutName` (lines 48-58) unchanged**

`resolveFolderLayoutName` already delegates to `isNavigatorFolderLayoutName`, which now accepts `'tree'`. No edit needed; verify by reading the function in the file.

- [ ] **Step 5: Run the failing tests to verify they pass**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vitest run src/modules/navigator/utils/__tests__/resolve-navigator-folder-settings.test.ts`
Expected: All tests in this file pass (existing + new 3).

- [ ] **Step 6: Run full navigator test suite to verify no regressions**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vitest run src/modules/navigator/`
Expected: All passing, including the 3 new tree tests.

- [ ] **Step 7: Commit**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add src/modules/navigator/utils/resolve-navigator-folder-settings.ts
git commit -m "feat(navigator): support tree layout in folder settings resolution"
```

---

## Task 5: Add Tree button to `navigator-toolbar-actions.vue`

**Files:**
- Modify: `sigma-file-manager/src/modules/navigator/components/navigator-toolbar-actions/navigator-toolbar-actions.vue`

**Interfaces:**
- Consumes: `persistScopedPatch({ layout })` from existing scope (line 114); `LayoutType` local type (line 49); `t('list')` / `t('grid')` keys (lines 256, 265)
- Produces: a third button in the layout-row that calls `setLayout('tree')`, shows `FolderTreeIcon`, and uses `t('tree')`

- [ ] **Step 1: Add `FolderTreeIcon` to lucide imports**

Find the lucide import block (lines 27-38). After `LayoutGridIcon,` and before `CircleHelpIcon,`, add `FolderTreeIcon,`. The new line:

```ts
import {
  FlipHorizontalIcon,
  PanelLeftRightDashedIcon,
  PanelRightOpenIcon,
  LayoutGridIcon,
  FolderTreeIcon,
  CircleHelpIcon,
  EllipsisVerticalIcon,
  EyeOffIcon,
  InfoIcon,
  PanelRightIcon,
} from '@lucide/vue';
```

- [ ] **Step 2: Extend local `LayoutType` (line 49)**

Change:
```ts
type LayoutType = 'list' | 'grid';
```
To:
```ts
type LayoutType = 'list' | 'grid' | 'tree';
```

- [ ] **Step 3: Add the Tree button to the layout-row (after the grid button, before closing `</div>` of `.navigator-settings-menu__layout-row`)**

The current grid button template is at lines 258-266. After it, before line 267 `</div>`, insert:

```vue
                    <button
                      type="button"
                      class="navigator-settings-menu__layout-option"
                      :class="{ 'navigator-settings-menu__layout-option--active': currentLayout === 'tree' }"
                      @click="setLayout('tree')"
                    >
                      <FolderTreeIcon :size="24" />
                      <span>{{ t('tree') }}</span>
                    </button>
```

Result: three buttons sit side by side inside `.navigator-settings-menu__layout-row` with `flex: 1` each, so they auto-fit the 280px dropdown width without CSS changes.

- [ ] **Step 4: Run typecheck**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vue-tsc --noEmit -p tsconfig.json`
Expected: 0 errors. `setLayout('tree')` is now valid because `LayoutType` includes `'tree'`; `currentLayout === 'tree'` narrows correctly because `scopedSettings.value.layout` is now `'list' | 'grid' | 'tree'`.

- [ ] **Step 5: Commit**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add src/modules/navigator/components/navigator-toolbar-actions/navigator-toolbar-actions.vue
git commit -m "feat(navigator): add tree layout option to toolbar dropdown"
```

---

## Task 6: Add `"tree"` translation key to `en.json`

**Files:**
- Modify: `sigma-file-manager/src/localization/messages/en.json`

**Interfaces:**
- Consumes: existing `"list"` (line 765) and `"grid"` (search for `"grid":`) keys
- Produces: new `"tree": "Tree"` entry adjacent to `"list"` and `"grid"`

- [ ] **Step 1: Locate `"grid"` key in `en.json` and add `"tree"` adjacent to it**

Use Grep for `"grid"` in `en.json` and find the entry. If `"grid": "Grid"` exists, add the line immediately after it:
```json
  "tree": "Tree",
```
If `"grid"` does not exist at the top level (it may be inside a section), find the closest sibling to `"list"` and add `"tree"` there. Confirm by reading 3 lines of context around the insertion.

- [ ] **Step 2: Run sync script (optional, to verify auto-sync works)**

If `vite-plugin-run` is configured to watch i18n, simply save the file. To force-sync once, run:
```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
npm run sync:i18n
```
Expected: exit 0, and at least one other locale file (e.g. `ch.json`) gains a `"tree"` key. If the script does not exist or fails, skip and rely on the plugin to sync during dev.

- [ ] **Step 3: Run typecheck to verify no string-typed compile errors**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vue-tsc --noEmit -p tsconfig.json`
Expected: 0 errors. i18n keys are usually `Record<string, string>` so missing key is a runtime warning, not a TS error.

- [ ] **Step 4: Commit**

```bash
cd /f/soft/00selfmade/filemanager/sigma-file-manager
git add src/localization/messages/en.json
git commit -m "chore(i18n): add 'tree' layout label to en.json"
```

---

## Task 7: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Run full navigator test suite**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vitest run src/modules/navigator/`
Expected: All tests pass. No new failures beyond any pre-existing flaky timeout.

- [ ] **Step 2: Run full typecheck**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && npx vue-tsc --noEmit -p tsconfig.json`
Expected: 0 errors.

- [ ] **Step 3: Confirm git log shows expected commits**

Run: `cd /f/soft/00selfmade/filemanager/sigma-file-manager && git log --oneline feat/tree-view..HEAD`
Expected: 6 new commits, ordered:
1. `feat(navigator): extend NavigatorLayout type to include 'tree'`
2. `feat(navigator): extend NavigatorFolderLayoutName to include 'tree'`
3. `test(navigator): cover tree layout in folder settings resolution`
4. `feat(navigator): support tree layout in folder settings resolution`
5. `feat(navigator): add tree layout option to toolbar dropdown`
6. `chore(i18n): add 'tree' layout label to en.json`

---

## Self-Review Notes

- Spec coverage: dropdown row button ✓ (Task 5); type extensions ✓ (Tasks 1, 2); folder settings fix ✓ (Tasks 3-4); i18n ✓ (Task 6); verification ✓ (Task 7).
- No placeholders: each step has actual code or exact commands.
- Type consistency: `LayoutType` in Task 5 matches `NavigatorFolderLayoutName` in Task 2 and `NavigatorLayout['type']['name']` in Task 1. Button click handler signature matches `setLayout()` signature.
- Tasks 1 and 2 both modify `user-settings.ts`; each is small enough to be independently reviewable (union extension at two distinct lines), so they are split per the right-sizing rule.