// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

import { describe, expect, it } from 'vitest';
import type { DirEntry } from '@/types/dir-entry';
import type { ContextMenuActionInput } from '@/core/actions/context-menu-adapter';
import {
  buildContextMenuActionRegistry,
  getContextMenuActionEntries,
} from '@/core/actions/context-menu-adapter';
import { getActionContext } from '@/core/actions/predicates';
import {
  CONTEXT_MENU_ACTION_IDS,
  buildContextMenuActionDeclarations,
  isContextMenuActionAvailable,
  type ContextMenuActionVisibilityContext,
} from '@/modules/navigator/components/file-browser/utils/context-menu-action-visibility';

/** 菜单动作表（当字符串数组用，避免 `includes` 的字面量类型收窄）。 */
const MENU_ACTION_IDS = CONTEXT_MENU_ACTION_IDS as readonly string[];

function createEntry(path: string, isDir = false): DirEntry {
  return {
    name: path.split(/[\\/]/).pop() ?? path,
    ext: null,
    path,
    size: 0,
    item_count: null,
    modified_time: 0,
    accessed_time: 0,
    created_time: 0,
    mime: null,
    is_file: !isDir,
    is_dir: isDir,
    is_symlink: false,
    is_hidden: false,
  };
}

function createVisibility(
  overrides: Partial<ContextMenuActionVisibilityContext> = {},
): ContextMenuActionVisibilityContext {
  return {
    entries: [],
    platform: 'windows',
    disableDestructiveActions: false,
    isSearchResultsView: false,
    remoteTransfer: {
      hasRemoteCounterpart: false,
      hasLocalCounterpart: false,
    },
    canPaste: false,
    canCreateLink: true,
    canCreateFile: true,
    hasCreateTarget: false,
    ...overrides,
  };
}

type MenuConsistency = {
  projectedIds: string[];
  expectedIds: string[];
  /** 菜单动作表里有、注册表里查不到的动作。 */
  missingInRegistry: string[];
  /** 注册表声明要出现、投影里却没有的动作。 */
  missingInProjection: string[];
  /** 投影里冒出来的、菜单动作表之外的动作。 */
  unexpectedInProjection: string[];
};

/**
 * 判据 A 的检查器。
 *
 * 两个方向：
 *
 * - **菜单 → 注册表**：菜单动作表里的每个动作都必须在注册表里查得到；
 * - **注册表 → 菜单**：注册表声明「要出现」的动作都必须在投影结果里。
 */
function inspectMenuConsistency(
  visibility: ContextMenuActionVisibilityContext,
  declarations: ContextMenuActionInput[] = buildContextMenuActionDeclarations(visibility),
): MenuConsistency {
  const registry = buildContextMenuActionRegistry({ actions: declarations });
  const projectedIds = getContextMenuActionEntries(registry, getActionContext(null))
    .map(entry => entry.actionId);
  const expectedIds = CONTEXT_MENU_ACTION_IDS
    .filter(action => isContextMenuActionAvailable(action, visibility));

  return {
    projectedIds,
    expectedIds,
    missingInRegistry: MENU_ACTION_IDS.filter(id => !registry.has(id)),
    missingInProjection: expectedIds.filter(id => !projectedIds.includes(id)),
    unexpectedInProjection: projectedIds.filter(id => !MENU_ACTION_IDS.includes(id)),
  };
}

describe('右键菜单的动作注册表投影（判据 A）', () => {
  it('菜单动作表里的每个动作都能在注册表里查到，注册表声明要出现的动作也都在投影里', () => {
    const scenarios: ContextMenuActionVisibilityContext[] = [
      createVisibility({ entries: [createEntry('C:\\Users\\test\\a.txt')] }),
      createVisibility({
        entries: [createEntry('C:\\Users\\test\\a.txt'), createEntry('C:\\Users\\test\\b.txt')],
      }),
      createVisibility({ entries: [createEntry('C:\\Users\\test\\dir', true)] }),
      createVisibility({ entries: [] }),
      createVisibility({
        entries: [createEntry('C:\\Users\\test\\a.txt')],
        isSearchResultsView: true,
      }),
      createVisibility({
        entries: [createEntry('C:\\Users\\test\\a.txt')],
        platform: 'linux',
      }),
      createVisibility({
        entries: [createEntry('C:\\Users\\test\\dir', true)],
        canPaste: true,
        hasCreateTarget: true,
      }),
    ];

    for (const scenario of scenarios) {
      const consistency = inspectMenuConsistency(scenario);

      expect(consistency.missingInRegistry).toEqual([]);
      expect(consistency.missingInProjection).toEqual([]);
      expect(consistency.unexpectedInProjection).toEqual([]);
    }
  });

  it('单选普通文件时投影里确实有动作，避免上面那条是恒真断言', () => {
    const consistency = inspectMenuConsistency(
      createVisibility({ entries: [createEntry('C:\\Users\\test\\a.txt')] }),
    );

    expect(consistency.projectedIds.length).toBeGreaterThan(0);
    expect(consistency.projectedIds).toContain('rename');
    expect(consistency.projectedIds).toContain('copy');
    expect(consistency.projectedIds).toContain('open-containing-directory');
    // 应用内「打开所属目录」只在搜索结果视图里出现。
    expect(consistency.projectedIds).not.toContain('open-containing-directory-in-app');
    // 「粘贴」看剪贴板状态，这里没给 ⇒ 不出现。
    expect(consistency.projectedIds).not.toContain('paste');
  });

  it('负对照：少声明一个动作时判据必须报错（实现改坏就会红）', () => {
    const visibility = createVisibility({ entries: [createEntry('C:\\Users\\test\\a.txt')] });
    const declarations = buildContextMenuActionDeclarations(visibility)
      .filter(declaration => declaration.id !== 'copy');
    const consistency = inspectMenuConsistency(visibility, declarations);

    expect(consistency.missingInRegistry).toContain('copy');
    expect(consistency.missingInProjection).toContain('copy');
  });

  it('负对照：多声明一个菜单之外的动作时判据必须报错', () => {
    const visibility = createVisibility({ entries: [createEntry('C:\\Users\\test\\a.txt')] });
    const declarations: ContextMenuActionInput[] = [
      ...buildContextMenuActionDeclarations(visibility),
      {
        id: 'rogue-action',
        title: 'rogue-action',
      },
    ];
    const consistency = inspectMenuConsistency(visibility, declarations);

    expect(consistency.unexpectedInProjection).toContain('rogue-action');
  });

  it('负对照：适配器必须保留「显示但置灰」的动作、丢掉 hidden 的动作', () => {
    const registry = buildContextMenuActionRegistry({
      actions: [
        {
          id: 'greyed-action',
          title: 'greyed-action',
          when: () => false,
          hideWhenUnavailable: false,
        },
        {
          id: 'hidden-action',
          title: 'hidden-action',
          when: () => false,
          hideWhenUnavailable: true,
        },
      ],
    });
    const entries = getContextMenuActionEntries(registry, getActionContext(null));

    expect(entries).toEqual([{
      actionId: 'greyed-action',
      availability: 'disabled',
    }]);
  });
});
