# Handoff — Sigma Fork: Tree Sync v6.1 (sidebar tree + toolbar toggle)

**会话日期**: 2026-09-26 (8th session)
**承接**: `handoff-2026-09-26-tree-sync-v6.md` — v6 sync 工作了,但用户报告 layout 错(上下堆叠)且 toggle 按钮藏在 dropdown
**本次目标**: 把 tree view 从 file-browser-content 移到 navigator 左侧 sidebar,加独立 toolbar toggle 按钮,默认打开
**下次第一句话**: "从 handoff-2026-09-26-tree-sync-v6-1.md 接力"

---

## 一、本次完成

### 1. Toolbar 加独立 FolderTreeIcon 按钮(从 dropdown 里拿出)

- `navigator-toolbar-actions.vue`:
  - 加 `showFolderTree: boolean` prop
  - 加 `'toggle-folder-tree'` emit
  - 加独立 FolderTreeIcon 按钮,active class 当 `showFolderTree=true`
  - 从 layout dropdown 移除 Tree 选项(dropdown 只剩 list/grid)
  - LayoutType 由 `'list' | 'grid' | 'tree'` 改为 `'list' | 'grid'`

### 2. Navigator 左侧 sidebar tree 面板

- `navigator.vue`:
  - 加 `useDrives()` import,`treeRootPaths = computed(() => drives.value.map(d => d.path))`
  - 加 `showFolderTree = ref(userSettingsStore.userSettings.navigator.showFolderTree ?? true)`,持久化
  - 加 `handleToggleFolderTree`,加 `handleTreeActivate` / `handleTreePreview`
  - 把 panes-container 包进新 ResizablePanelGroup(`.navigator-page__tree-and-panes`),左 panel 是 FileBrowserTreeView(默认 22% 宽,12-40% 范围),右 panel 是 panes
  - 加 CSS:`.navigator-page__folder-tree-panel` / `__folder-tree` / `__panes-section` / `__tree-and-panes`
  - 大小屏两个分支都改了

### 3. file-browser-content.vue 移除 tree view 引用

- 删除 `<FileBrowserTreeView>` 渲染分支
- 删除 `onTreeActivate` 函数
- 删除 FileBrowserTreeView import
- layout prop 类型保持 `'list' | 'grid' | 'tree'`(navigator 可能仍传 'tree',文件内容正常渲染)

### 4. UserSettings 类型扩展 + i18n

- `user-settings.ts`:`UserSettingsNavigator.showFolderTree?: boolean`
- `en.json`:`navigator.showFolderTree` / `navigator.hideFolderTree`

### 5. v6.1 build 出炉

| 文件 | sha256 | 时间 |
|---|---|---|
| `release/Sigma-File-Manager-2.2.0-x64-setup.exe` | `003861ef8b83a908abcae30d33bcd5429668bfb97d41eff4b391bb445cf58236` | 2026-09-26 ~10:30 |

---

## 二、当前 git 状态

- `feat/tree-view` HEAD:本次新 commit(单 atomic commit,见 §三)
- 工作区 clean

---

## 三、测试证据

- `npm run ts` → 0 errors
- `npm run test:unit:run -- src/modules/navigator/` → **257/257 pass**(v6 是 255 + 2 新增)
- `npm run tauri:build` → 成功(vite 4m34s + rust 3m05s ≈ 7.5 分钟)

---

## 四、改动文件清单

| 文件 | 改动 |
|---|---|
| `src/modules/navigator/components/navigator-toolbar-actions/navigator-toolbar-actions.vue` | LayoutType 去 tree,加 showFolderTree prop + toggle-folder-tree emit + FolderTreeIcon 主按钮,layout dropdown 去 Tree 按钮 |
| `src/modules/navigator/pages/navigator.vue` | useDrives + treeRootPaths + showFolderTree state + persist + handleToggleFolderTree/handleTreeActivate/handleTreePreview + 新 ResizablePanelGroup 包 panes-container(tree 在左) + CSS |
| `src/modules/navigator/components/file-browser/file-browser-content.vue` | 移除 FileBrowserTreeView 引用 + onTreeActivate + import |
| `src/types/user-settings.ts` | UserSettingsNavigator 加 `showFolderTree?: boolean` |
| `src/localization/messages/en.json` | navigator 加 showFolderTree / hideFolderTree |

---

## 五、下一步必交付(用户必测)

装机 v6.1(`003861ef...`)→ 验证:
- [ ] 应用打开,**默认左侧显示 directory tree sidebar**(显示所有 drives)
- [ ] Toolbar 右上第一个图标是独立的 FolderTreeIcon(active 高亮)
- [ ] 点击 toolbar FolderTreeIcon → 左侧 sidebar tree 消失/出现
- [ ] 关闭 app 再启动 → sidebar tree 状态保持
- [ ] 点击 tree 里某个 drive → file browser 右侧切到那个 drive
- [ ] 点击 tree 里某个子目录 → file browser 右侧切到该目录,展开路径
- [ ] 点击 tree 里文件 → quick view 打开
- [ ] Layout dropdown 只剩 list/grid 两选项
- [ ] 切换 split-view → 左侧 sidebar tree 仍可见

---

## 六、避坑提示(下次会话)

### 1. 不要回退 v6 store 改造

`useFolderTreeStore` + `useFileTree` consumer 模式是 v6 核心。sidebar tree 直接消费 store(`storeToRefs`),不需要任何 ref forwarding。

### 2. CSS 嵌套 ResizablePanel

navigator 现在有 3 层 ResizablePanel(outer info panel, mid tree+panes, inner split-view panes)。reka-ui 支持嵌套但要小心 key 稳定性,否则 size 会重置。

### 3. drives 异步加载

`useDrives()` 在 onMounted 才启动 initialFetch。treeRootPaths 初始空数组,sidebar 显示空,几秒后 drives 加载完自动填充。

### 4. showFolderTree 持久化

`userSettingsStore.set('navigator.showFolderTree', next)` — 通过 UserSettingsPath 模板类型自动推导。直接 read `userSettings.userSettings.navigator.showFolderTree` 即可(可能 undefined,所以 `?? true`)。

### 5. ancestor load 失败兜底

跟 v6 一样:loadChildren 抛错被 try/catch 捕获,row 仍渲染(只是没 children)。UI 永远不白屏。

---

## 七、必读顺序

1. 本 handoff §一(本次完成)
2. `handoff-2026-09-26-tree-sync-v6.md` §一(v6 基础)
3. `src/modules/navigator/pages/navigator.vue`(sidebar tree 容器)
4. `src/stores/runtime/folder-tree.ts`(store 入口)
5. memory `watch-on-computed-with-later-tdz.md`(避免再触发 TDZ)

---

## 八、关键决策记录

### D1: tree root = drives(不是 currentPath)

v6 把 rootPaths 设为 `[currentPath.value]`(file-browser-content 传)。v6.1 改为 `drives.value.map(d => d.path)`(navigator 传)。理由:
- tree 是全局视图,显示所有 drives 入口
- 当前路径的 ancestor 由 store.expandedPaths 自动管理(不需要 root 包含)
- 用户切换到任何 path,store.setSelectedPath 自动展开 ancestor

### D2: tree 与 layout dropdown 解耦

之前 layout dropdown 有 Tree 选项(切换 file browser 内容显示树)。现在 tree 是 sidebar(独立 toggle 按钮),layout 只控制 list/grid。理由:
- 职责单一:tree 是导航视图,layout 是内容视图
- 隐藏 sidebar 后 layout 仍是 list/grid,符合 Windows Explorer 习惯

### D3: ResizablePanel min/max 12-40%

默认 22%(占屏幕 1/4),最小 12%,最大 40%。提供足够空间又不会侵占文件浏览器。

### D4: 文件 emit `preview` 而非 `activate`

`handleTreeActivate` 调 `navigateToPath`,`handleTreePreview` 调 `quickViewStore.toggleQuickView`。file-browser-tree-view 当前 emit 只有 `activate`,需要补 `preview` emit + 文件 vs 目录的 click 区分(类似 sidebar-tree-preview handoff §一轮次 4 描述)。**注意:这个还没改**,目前 tree-view 文件点击也会调 navigateToPath 到文件所在目录。需要后续补充。

---

## 九、已知 TODO

1. **file-browser-tree-view emit preview** — 当前只 emit `activate`,文件点击走 navigateToPath(到父目录)。需要补 `preview` emit + 文件/目录 click 区分
2. **tree 节点滚动到视口** — 当前选中节点不一定在视口内,需要 scrollIntoView
3. **持久化 sidebar 宽度** — 当前 22% 是 hardcoded,resize 后会重置
4. **Extension API `ExtensionViewLayout`** 仍 fallback 'list'(v6 已记录)
5. **baseline test isolation 1 fail** — `use-navigator-item-icon.test.ts` 跨 Pinia 状态失败(v6 已知,与本次无关)