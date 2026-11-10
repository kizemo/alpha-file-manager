<!-- SPDX-License-Identifier: GPL-3.0-or-later
License: GNU GPLv3 or later. See the license file in the project root for more information.
-->

# Spec — Toolbar Tree Toggle

**Date**: 2026-09-25
**Phase**: Sigma Fork Phase 2 follow-up
**Handoff in**: `handoff-2026-09-25-phase2.md`

## Goal

让用户通过 toolbar 的 layout 切换菜单(list / grid / tree 三选项)把 navigator 切到 tree view。

## Approach

在现有 dropdown 的 layout-row 加 Tree 第三按钮,复用 `setLayout()` + `persistScopedPatch()` 链路,模式与 list/grid 完全对称。同步修复 Phase 2 遗留的 folder-scoped settings bug:`'tree'` 会被 `resolve-navigator-folder-settings` 回退成 `'list'`,因为 `NavigatorFolderLayoutName` 没扩展。

## Files Touched

| # | Path | Change |
|---|---|---|
| 1 | `sigma-file-manager/src/modules/navigator/components/navigator-toolbar-actions/navigator-toolbar-actions.vue` | `LayoutType` 加 `'tree'`;layout-row 加第 3 按钮用 `FolderTreeIcon` |
| 2 | `sigma-file-manager/src/types/user-settings.ts` | `NavigatorFolderLayoutName` 加 `'tree'` |
| 3 | `sigma-file-manager/src/modules/navigator/utils/resolve-navigator-folder-settings.ts` | 4 处 list/grid 分支加 `'tree'`(`getNavigatorFolderLayoutName`、`toNavigatorFolderLayoutType`、`isNavigatorFolderLayoutName`、`resolveFolderLayoutName`) |
| 4 | `sigma-file-manager/src/localization/messages/en.json` | 加 `"tree": "Tree"`(其它 locale 由 `vite-plugin-run` 自动同步) |

## Implementation Details

### Toolbar 组件

- 局部 `type LayoutType = 'list' | 'grid' | 'tree'`
- 导入 `FolderTreeIcon` from `@lucide/vue`
- layout-row 复制 list/grid 按钮模板,改 `'tree'` + `FolderTreeIcon` + `t('tree')`

### Folder settings 类型扩展

```ts
// types/user-settings.ts:345
export type NavigatorFolderLayoutName = 'list' | 'grid' | 'tree';
```

### Folder settings 联动修复

```ts
// resolve-navigator-folder-settings.ts
export function getNavigatorFolderLayoutName(
  layoutName: NavigatorLayout['type']['name'],
): NavigatorFolderLayoutName {
  if (layoutName === 'grid') return 'grid';
  if (layoutName === 'tree') return 'tree';
  return 'list';
}

export function toNavigatorFolderLayoutType(
  layoutName: NavigatorFolderLayoutName,
): NavigatorLayout['type'] {
  if (layoutName === 'grid') return { title: 'gridLayout', name: 'grid' };
  if (layoutName === 'tree') return { title: 'treeLayout', name: 'tree' };
  return { title: 'listLayout', name: 'list' };
}

function isNavigatorFolderLayoutName(value: unknown): value is NavigatorFolderLayoutName {
  return value === 'list' || value === 'grid' || value === 'tree';
}
```

`resolveFolderLayoutName` 不用改 —— `'tree'` 会走 `isNavigatorFolderLayoutName` true 分支。

### i18n

en.json 现有:
```
"list": "List",
"grid": "Grid",
```
追加:
```
"tree": "Tree",
```
同步方式:`npm run sync:i18n`(执行 `scripts/sync-i18n.js`);en.json 保存后也可能由 post-edit hook 自动触发。

## Out of Scope

- toolbar 主行加 quick-toggle 图标按钮(保持 list/grid 入口对称)
- e2e 测试去掉 `it.skip`(下一步接力再做)
- 全局 `NavigatorLayout['type']['name']` 扩展(Phase 2 已做)
- 组件单测(项目里 `navigator-toolbar-actions` 无测试先例)

## Verification

1. `npx vitest run src/modules/navigator/` — 全绿
2. `npx vue-tsc --noEmit` — 零错
3. 手动:打开 dropdown → 看到三个按钮 → 选 tree → 渲染 tree view → 切回 list/grid 正常 → folder-scoped tree 设置刷新不丢

## Risks

- **低**:`persistScopedPatch` 已经在用,模式不变
- **低**:`getNavigatorFolderLayoutName` 是纯函数,只多一个分支
- **极低**:`FolderTreeIcon` 是 lucide 标准图标,无新增依赖