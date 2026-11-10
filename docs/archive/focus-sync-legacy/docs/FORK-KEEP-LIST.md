# Fork 保留清单 (FORK-KEEP-LIST)

这些文件/区域 **永远用 fork 版本**,即使 upstream 重写。
未来 sync upstream 时(`scripts/sync-upstream.sh`)按此清单决定哪些改动保留、哪些丢弃。

> 约定:fork 改动带 `[fork-keep]` 注释(commit message 前缀 + 代码块注释)。
> grep 关键字:`FORK-MODIFICATION`、`fork-keep`

---

## 文件树视图(对应 upstream issue #499)

upstream 维护者误关闭但未实现。本地 fork 自己落地,长期保留。

### 核心改动文件

| 文件 | 类型 | 说明 |
|---|---|---|
| `sigma-file-manager/src/modules/navigator/types/file-tree.ts` | 新增 | 类型定义(FileTreeNode / FileTreeFlatRow) |
| `sigma-file-manager/src/modules/navigator/types/__tests__/file-tree.test.ts` | 新增 | 类型测试 |
| `sigma-file-manager/src/modules/navigator/composables/use-file-tree.ts` | 新增 | 懒加载 composable |
| `sigma-file-manager/src/modules/navigator/composables/__tests__/use-file-tree.test.ts` | 新增 | composable 测试 |
| `sigma-file-manager/src/modules/navigator/components/file-browser/file-browser-tree-view.vue` | 新增 | 树视图组件 |
| `sigma-file-manager/e2e-webdriver/test/specs/navigator-tree-view.e2e.js` | 新增 | e2e 占位 |

### 修改文件(找 `FORK-MODIFICATION` 注释)

| 文件 | 改动 |
|---|---|
| `sigma-file-manager/src/modules/navigator/components/file-browser/types.ts` | `FileBrowserProps.layout` 加 `'tree'` |
| `sigma-file-manager/src/modules/navigator/components/file-browser/file-browser.vue` | `layout` prop 加 `'tree'` |
| `sigma-file-manager/src/modules/navigator/components/file-browser/file-browser-content.vue` | layout prop + 新增 tree 分支渲染 FileBrowserTreeView |
| `sigma-file-manager/src/modules/navigator/components/file-browser/file-browser-content-body.vue` | layout prop 加 `'tree'` |
| `sigma-file-manager/src/modules/navigator/pages/navigator.vue` | `getLayoutForPath` 返回类型加 `'tree'` |

### 同步策略

- 若 upstream 新增 tree view:
  1. 对比 `useFileTree` 接口,若签名兼容则优先用 upstream 实现,本地 fork 删掉
  2. 若实现差异大(状态机 / 展开策略 / 性能特征),继续保留本地版本
- 若 upstream 重构 `file-browser.vue` / `file-browser-content.vue`:
  1. 优先保留 fork 的 tree 分支接入点
  2. 其他改动按正常 sync 流程合并
- 若 upstream 修改 `types.ts` 的 `FileBrowserProps.layout`:union 类型要保持包含 `'tree'`

### 已知限制 / 待办

- toolbar layout 切换按钮未添加(tree 模式目前通过修改 user settings 激活)
- e2e 测试占位(`it.skip`)等切换 UI 上线后启用
- `useFileTree` 默认 `readDir` 走 `resolveDirectoryContents`(Tauri invoke),测试里必须注入 mock

---

## 自定义启动目录(Phase 1,对应 PR #546)

不属 fork 独有特性,已向上游提 PR (#546)。sync 时按 upstream 合并决定保留。

### 修改文件

| 文件 | 改动 |
|---|---|
| `sigma-file-manager/src/types/user-settings.ts` | user settings 类型 |
| `sigma-file-manager/src/composables/use-init.ts` | customPath 分支 |
| `sigma-file-manager/src/utils/last-route.ts` | switch 加 case |
| `sigma-file-manager/src/modules/settings/ui/categories/general/startup-page.vue` | UI 选项 |
| `sigma-file-manager/src/localization/messages/{en,ch,...}.json` | i18n keys(vite-plugin-run 自动同步) |

### 同步策略

- PR 合并前:本地保留
- PR 合并后:等同 upstream,正常 sync

---

## 维护规则

新增 fork 改动时:

1. commit message 前缀加 `[fork-keep]`,代码块顶部加 `<!-- FORK-MODIFICATION: ... -->`
2. 更新本文档对应章节,列出新增/修改文件
3. 在 PR / sync 时优先保留 `[fork-keep]` 标记的改动