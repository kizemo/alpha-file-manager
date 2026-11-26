# 接力 prompt — Phase 2 后续

承接: `F:/soft/00selfmade/filemanager/handoff-2026-09-25-phase2.md`

## 当前状态

- Fork 仓库: `F:/soft/00selfmade/filemanager/sigma-file-manager/`
- 分支: `feat/tree-view`(5 commits, working tree 干净)
- Meta-repo: `main` 上 1 commit (FORK-KEEP-LIST)
- Tests: 241/241 通过,vue-tsc 干净

## 必交付清单

1. 决定下一步走哪条:
   - (a) **toolbar 切换按钮** — 让用户点 UI 切到 tree view
   - (b) **merge feat/tree-view 到 main 并 push fork**
   - (c) **新工作**(Phase 3-5 / 其他)

2. 如果走 (a):
   - 看 `src/modules/navigator/components/navigator-toolbar-actions/navigator-layout-sort-controls.vue` 找现有 layout 切换模式
   - 加 Tree 按钮,emit `update:layout` 或写 userSettingsStore('navigator.layout.type.name', 'tree')
   - 同步加 'tree' 到 `extension-view-settings.ts` 的 `EXTENSION_VIEW_LAYOUTS` 和 `layoutTitleByName` 的 map
   - 启用 `navigator-tree-view.e2e.js` 的 `it.skip`

## 禁止事项

- 不要 `npm run tauri:build`
- 不要提 PR 给 upstream(issue #499 已关闭)
- 不要 cherry-pick 其他分支的 commit 到 feat/tree-view

## 加速模式规则

- 不适用本次任务,标准模式

## 第一句话

> 从 handoff-2026-09-25-phase2.md 接力,做 [你选的下一步]