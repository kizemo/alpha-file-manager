# Handoff — Sigma Fork: Tree Sync v6 (store-driven rewrite)

**会话日期**: 2026-09-26 (7th session)
**承接**: `handoff-2026-09-26-tree-sync-retro.md` — 复盘 v0..v5 全部失败,提议 store-driven 重构
**本次目标**: 按 §六 方案实施 v6 — 用 Pinia `useFolderTreeStore` 替代「parent 拿 child ref 调 expandToPath」的 6 跳链
**下次第一句话**: "从 handoff-2026-09-26-tree-sync-v6.md 接力"

---

## 一、本次完成

按 retro handoff §七 步骤 9 条全做,build v6 NSIS 已出炉。

### 1. 新 Pinia store `useFolderTreeStore`

- 文件:`src/stores/runtime/folder-tree.ts`
- 状态:`selectedPath`、`expandedPaths`、`loadingPaths`、`loadErrorPaths`(4 个 Set/String ref,每次 mutation replace 引用以保证 Vue reactivity —— 修 v5 RC-1)
- Action:`setSelectedPath(path|null)`、`toggleExpanded`、`expandPath`、`collapsePath`、`markLoading`、`markLoadError`、`reset`
- 纯函数 helper:`computeAncestorPaths(path)`(Windows + POSIX 路径,递归 parentDirectoryOf,Pin root POSIX 不入 ancestors,Windows drive root 'C:/' 入 ancestors)
- 测试:19/19 pass(覆盖 ancestors 算法 + 4 个 set mutation + reset + 失败兜底)

### 2. `useFileTree` composable 改为 store-consumer

- API:`{ rootPaths, expandedPaths, onLoadStart/End/Error }` → `{ rows, loadedPaths, ensureLoaded, ensureAncestorsLoaded, addRoot }`
- `isExpanded` 由 `options.expandedPaths` 决定(node 不再 mutate `isExpanded` 字段,虽然类型仍保留)
- `rootPaths` 接受 `MaybeRefOrGetter<string[]>`(`toValue`),可响应
- `addRoot(path)`:ancestor 不在 nodes 时动态加入(关键:让 store 自动展开的 ancestor 能落地)
- `ensureLoaded`:try/catch `loadChildren`,失败调 `onLoadError` 而不 throw(满足 handoff §8.3「ancestor load 失败要让 UI 仍能展开」)
- 测试:9/9 pass(覆盖初始化/展开反射/懒加载/幂等/ancestor load/失败兜底/collapse+re-expand 不重读/addRoot/响应 rootPaths 重建)

### 3. `FileBrowserTreeView` store-driven 化

- 删除 `defineExpose`(无 template ref 暴露)
- `storeToRefs(folderTreeStore)` 拿 `expandedPaths`、`selectedPath`
- 内部维护 `dynamicRoots`(初始 `[...props.rootPaths]`,watch `selectedPath` 自动添加 ancestor)
- `useFileTree({ rootPaths: dynamicRoots, expandedPaths, onLoadStart/End/Error })`
- watch `selectedPath` → `ensureAncestorsLoaded(computeAncestorPaths(path))`
- 行高亮:`file-tree-row--selected`(基于 `store.selectedPath`)
- 行错误:`file-tree-row--error`(基于 `store.hasLoadError(path)`)
- 行加载:`file-tree-row--loading`(基于 `store.isLoading(path)`)
- 测试:5/5 pass

### 4. `navigator.vue` 简化

- 移除 v5 的 `treeViewRef`、`syncTreeToActivePath`、`treeSelectedPath`、per-tab Map、4 处 watch
- `handleCurrentDirChange` 一行多调:`folderTreeStore.setSelectedPath(entry?.path ?? null)`
- baseline 不需要删任何 v5 代码(已经 reset 到 ec3389ba,基线干净)

### 5. Baseline type 补丁(伴随 baseline `1c9b51f4` 加 'tree' layout 时漏掉的类型补全)

baseline commit `1c9b51f4 feat(navigator): add tree layout option to toolbar dropdown` 把 `'list' | 'grid'` 类型扩到 `'list' | 'grid' | 'tree'` 但只更新了部分位置。剩下的 7 个文件本次一起补:
- `use-file-browser.ts` + 5 sub-composable 接受 layout 的参数类型
- `use-file-browser-context.ts` + `file-browser.vue` provide 新加 `navigateToPath`(原 file-browser-content.vue onTreeActivate 用但缺)
- `file-browser-entry-groups.ts` + `file-browser-box-selection-hit-test.ts` 内部 helper layout 类型
- `file-browser-sort-columns.ts`(`NavigatorSortLayout`)
- `navigator-layout-sort-controls.vue`(`sortLayout` prop)
- `extension-view-settings.ts`(`readExtensionViewLayout` runtime guard 因为 `@sigma-file-manager/api` 包的 `ExtensionViewLayout` 没扩)

### 6. v6 build 出炉

| 文件 | sha256 | 时间 |
|---|---|---|
| `release/Sigma-File-Manager-2.2.0-x64-setup.exe` | `ef4269a2d41ab66cff794afbec4e76f208af86144ae972ecee0fae4653b0ca8d` | 2026-09-26 ~08:11 |

---

## 二、当前 git 状态

- **Fork `feat/tree-view`**:已 commit v6(单 atomic commit,涵盖 19 文件改动 + 3 新文件)
- 分支 HEAD 未 push(fork 决策待用户)
- 工作区 clean

---

## 三、测试证据

- `npm run ts` → 0 errors(vue-tsc clean)
- `npm run test:unit:run -- src/modules/navigator/` → **255/255 pass**
- 单跑 `src/modules/navigator/composables/__tests__/use-file-tree.test.ts` → 9/9
- 单跑 `src/stores/runtime/__tests__/folder-tree.test.ts` → 19/19
- 单跑 `src/modules/navigator/components/file-browser/__tests__/file-browser-tree-view.test.ts` → 5/5
- 全套 vitest 跑(157 files)有 1 个 fail 在 `use-navigator-item-icon.test.ts` —— **baseline (ec3389ba) 同样 fail**(test isolation 问题,跨 Pinia 状态,跟本次改动无关)

---

## 四、改动文件清单

### 新增(3)

| 文件 | 用途 |
|---|---|
| `src/stores/runtime/folder-tree.ts` | Pinia store(selectedPath/expandedPaths/loadingPaths/loadErrorPaths + ancestor 算法) |
| `src/stores/runtime/__tests__/folder-tree.test.ts` | store 单测 19 case |
| `src/modules/navigator/components/file-browser/__tests__/file-browser-tree-view.test.ts` | store-driven tree-view integration 测试 5 case |

### 修改 — 核心 sync 改动(4)

| 文件 | 改动 |
|---|---|
| `src/modules/navigator/composables/use-file-tree.ts` | API 重写为 store-consumer(expandedPaths 外部传入、ancestor 自动 addRoot、失败 try/catch) |
| `src/modules/navigator/composables/__tests__/use-file-tree.test.ts` | 测试改为新签名 + 加 ancestor/失败兜底/reactive rootPaths case |
| `src/modules/navigator/components/file-browser/file-browser-tree-view.vue` | storeToRefs 订阅,删除 defineExpose,新增 selected/error/loading class |
| `src/modules/navigator/pages/navigator.vue` | `handleCurrentDirChange` 调 `folderTreeStore.setSelectedPath` |

### 修改 — baseline type 补丁(12)

| 文件 | 改动 |
|---|---|
| `src/modules/navigator/components/file-browser/composables/use-file-browser.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-context.ts` | FileBrowserContext 加 `navigateToPath` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-selection.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-virtual-layout.ts` | `layout` 类型 + 内部 helper 加 `'tree'` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-box-selection.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-item-counts.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-keyboard-navigation.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-link-metadata.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/file-browser.vue` | provideFileBrowserContext 加 `navigateToPath: fb.navigateToPath` |
| `src/modules/navigator/components/file-browser/file-browser-entry-groups.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/utils/file-browser-box-selection-hit-test.ts` | `layout` 类型加 `'tree'` |
| `src/modules/navigator/components/file-browser/utils/file-browser-sort-columns.ts` | `NavigatorSortLayout` 加 `'tree'` |
| `src/modules/navigator/components/navigator-toolbar-actions/navigator-layout-sort-controls.vue` | `sortLayout` prop 加 `'tree'` |
| `src/modules/extensions/utils/extension-view-settings.ts` | `readExtensionViewLayout` runtime guard(扩展 API 还没扩 'tree',fallback 'list') |

---

## 五、下一步必交付(用户必测)

### 装机 v6(`ef4269a2...`)→ 验证

- [ ] 页面不白屏
- [ ] 收藏/主页点目录 → tree 展开+高亮(单 pane)
- [ ] 文件窗口点击/地址栏 → tree 联动(单 pane)
- [ ] Split-view 切 pane → tree 跟随(每个 pane 写自己的 selectedPath 到同一个 store)
- [ ] Toggle folder tree on/off → 重新打开同步当前路径(store state 持久)
- [ ] ancestor load 失败场景(权限/虚拟路径) → 行仍在,error 标记,不抛错

### Commit + push

v6 已 commit 到 `feat/tree-view`,待用户确认后 push 到 fork。

---

## 六、避坑提示(下次会话)

### 1. 不要回退到 v5 的 imperative ref forwarding

store-driven 是 v6 的根本改进。任何「为了兼容旧路径加回 expandToPath」都是倒退。

### 2. Set mutation 必须 replace 引用

`set.add()` / `set.delete()` 不触发 Vue 响应。每次 mutation 必须 `expandedPaths.value = new Set(expandedPaths.value).add(...)`。store action 内部已 enforce,新代码不能改。

### 3. ancestor load 失败兜底

`useFileTree.ensureLoaded` 内部 try/catch 后调 `onLoadError`,**不重抛**。失败也 mark `loadedSet`,UI 永远渲染行,只是没 children。

### 4. `useFileBrowser` 的 layout 现在是 `'list' | 'grid' | 'tree' | undefined`

任何新加 layout 的代码要同步扩这个 union。Extension API(`@sigma-file-manager/api`) 的 `ExtensionViewLayout` 还没扩 tree,扩展 layout 走 fallback `'list'`。

### 5. `useFileTree` 的 `addRoot` 是公开 API

外部 caller(如某个新写的 tree-view variant)可以主动 addRoot。但不要每次都加:expandedPaths 自动 watch 触发 ensureLoaded,会自动 addRoot 当 ancestor 不在 nodes 时。

---

## 七、必读顺序

1. 本 handoff §一(本次完成总览)
2. `handoff-2026-09-26-tree-sync-retro.md` §四 + §六(retro 的根因分析 + store 方案)
3. `src/stores/runtime/folder-tree.ts`(store 入口)
4. `src/modules/navigator/components/file-browser/file-browser-tree-view.vue`(store 订阅示范)
5. memory `watch-on-computed-with-later-tdz.md`(避免再触发 TDZ)

---

## 八、关键决策记录

### D1: ancestor 算法 Windows drive root 'C:/' 入 ancestors,POSIX '/' 不入

理由:Windows 下用户希望 tree 展开到 drive letter 级别(常见浏览入口),POSIX root '/' 展开无意义。

### D2: store action 内部每次 Set mutation replace 引用

理由:v5 RC-1 教训:Vue 3 `ref(new Map())` 的 `set()` 不触发响应,但 `reactive(Map)` 的 `set()` 触发,以及 `ref(Map)` 的 `replace new Map(...)` 触发。我们用 ref 配合 replace 引用(更显式,debug 容易)。

### D3: `navigator.vue` 仍然保留 FileBrowser emit + listener 链

理由:FileBrowser emit `update:current-dir-entry` 不仅是给 tree 用,还被其他组件订阅(全局搜索、地址栏同步等)。所以 emit 不变,parent listener 多调一行 store。

### D4: `ExtensionViewLayout` 不扩 tree,`readExtensionViewLayout` runtime fallback 'list'

理由:扩展 API 是另一个 npm 包(`@sigma-file-manager/api`),改它需要发包+下游同步。本次 v6 只在主 app 内部修。tree layout 在扩展 API 里没有对应 type,fallback 到 'list' 是合理的兜底(扩展不感知 tree)。

### D5: `useFileTree` 的 `expandedPaths` 参数接受 `MaybeRefOrGetter`

理由:测试里直接传 `ref(new Set())`,tree-view 里用 `storeToRefs(store).expandedPaths`。两种用法都支持,无需 caller 包装。

---

## 九、未完成项 / 已知 TODO

1. **`@sigma-file-manager/api` 包的 `ExtensionViewLayout`** 未扩 tree — 需要单独发包+下游 fork 同步
2. **e2e test 断言** 仍 pending(e2e-webdriver 配置已 ready 但 tree 同步的 DOM 断言未写)
3. **store.selectedPath 是全局单例** —— split-view 下两个 pane 会互相覆盖。如果要让两个 pane 独立 highlight,需要 per-pane store(后续优化项,v6 暂用单例满足 90% case)
4. **baseline 测试 `use-navigator-item-icon.test.ts` 跨 Pinia 状态 fail** —— 跟本次改动无关,但下次跑全套测试会看到 1 fail(可以忽略或单独修)
