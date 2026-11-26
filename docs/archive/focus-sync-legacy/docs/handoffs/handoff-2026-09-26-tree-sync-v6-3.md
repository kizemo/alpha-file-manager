# Handoff — Sigma Fork: Tree Sync v6.3 (fix depth collapse + click split)

**会话日期**: 2026-09-26 (10th session)
**承接**: `handoff-2026-09-26-tree-sync-v6-2.md` — v6.2 装机后用户报告 2 个严重 bug
**本次目标**: 修复树深度崩溃 + 重复节点;拆分 chevron / row click 行为
**下次第一句话**: "从 handoff-2026-09-26-tree-sync-v6-3.md 接力"

---

## 一、本次完成

### 1. 修复树深度崩溃 + 重复节点(root cause)

**Bug**: 用户 v6.2 装机后所有目录都缩进到同一级(depth 0),`办公文件` 出现两次,`002内衣项目` 也以独立 root 出现。

**Root cause**(`use-file-tree.ts` ensureLoaded):
- 旧逻辑:任何不在 nodes 树里的 path → `addRoot(path)`,作为新的 root 节点
- v6.1 在 `FileBrowserTreeView` 里把 `selectedPath` 的 ancestors 加进 `dynamicRoots`,触发 rebuildRoots 把这些 ancestors 作为新 root 插入
- v6.2 `setSelectedPath` 把 selectedPath 自身加入 `expandedPaths` → `useFileTree` watch 触发 `ensureLoaded('E:/办公文件/002内衣项目')` → addRoot → 这个 path 变成独立 root(depth 0)
- 后果:`E:/办公文件` 既是 `E:/` 的 child(depth 1)又是独立 root(depth 0);`002内衣项目` 也是

**Fix**(`use-file-tree.ts` ensureLoaded v6.3):
- 如果 path 不在 tree,向上 walk parent 链找到最近的已存在祖先
- 加载那个祖先的 children(让 path 作为 child 被发现)
- 只有"完全没有 ancestor 在 tree 里"时才 addRoot(真正的全新 root,如用户导航到未挂载的 drive)
- 这保证 ancestors 永远作为 child 出现,深度正确

### 2. 移除 v6.1 dynamicRoots ancestors watcher

`FileBrowserTreeView` 之前在 `dynamicRoots` ref 里加 selectedPath 的 ancestors,导致 `rebuildRoots` 把它们当新 root。v6.3 简化:`rootPaths` 直接来自 props(就是 drives),不再有任何动态添加。树结构完全由 `useFileTree` 的 walk-up 策略保证正确。

### 3. 拆分 chevron click 和 row click

**用户要求**: 点 chevron → 展开/收缩(不跳转);点 row 主体(包括 name)→ 跳转

**Fix**(`file-browser-tree-view.vue` template + script):
- 新增 `onChevronClick(row)`:`@click.stop="onChevronClick(row)"` 只调 `toggleExpanded`
- 新增 `onRowClick(row)`:row 主体 `@click="onRowClick(row)"` 只 emit `activate`(parent navigate)
- 不再在 row click 里 toggle expand——`setSelectedPath` 已经自动展开 ancestors,无需手动
- chevron 加 `.file-tree-row__chevron` class,hover 背景 + pointer cursor,让用户能识别它是独立 affordance

### 4. 测试覆盖

- `use-file-tree.test.ts` 测试仍 9/9 pass(行为兼容,只是 internals 改了)
- `file-browser-tree-view.test.ts`:
  - "clicking a directory row emits `activate` (name click → navigate only)"
  - "clicking the chevron toggles expandedPaths without emitting `activate`"

### 5. v6.3 build

| 文件 | sha256 | 时间 |
|---|---|---|
| `release/Sigma-File-Manager-2.2.0-x64-setup.exe` | `559a11d23628e53d7859c293de08c6cc11907f7061d3cb1efdbfec919302a04a` | 2026-09-26 ~12:20 |

---

## 二、当前 git 状态

- `feat/tree-sidebar-v6-1` HEAD:本次新 commit(单 atomic commit)
- 工作区 clean

---

## 三、测试证据

- `npm run ts` → 0 errors
- `npm run test:unit:run -- src/modules/navigator/` → **256/256 pass**(v6.2 是 255 + 1 新 chevron click 测试)
- 单跑 `file-browser-tree-view.test.ts` → 6/6
- `npm run tauri:build` → 成功

---

## 四、改动文件清单

| 文件 | 改动 |
|---|---|
| `src/modules/navigator/composables/use-file-tree.ts` | 重写 `ensureLoaded`:用 walk-up + load-parent 替代 addRoot;新增 `loadChildrenOfNode` 内部 helper;新增 `parentDirectoryOfPath` 内部 helper |
| `src/modules/navigator/components/file-browser/file-browser-tree-view.vue` | 移除 dynamicRoots ancestors watcher;rootPaths 直接 from props;`onClick` 拆成 `onChevronClick` + `onRowClick`;chevron icon 加 class + stop click;CSS 加 chevron hover |
| `src/modules/navigator/components/file-browser/__tests__/file-browser-tree-view.test.ts` | 测试改为:row click 仅 emit activate;chevron click 仅 toggle 不 emit |

---

## 五、下一步必交付(用户必测)

装机 v6.3(`559a11d2...`)→ 验证:
- [ ] 树结构正确显示,所有目录按 depth 正确缩进(不再平铺)
- [ ] 不再有重复节点(同一个 path 不出现两次)
- [ ] 点 chevron `>` / `v` → 只展开/收缩,不跳转
- [ ] 点 row 主体(目录名 / 文件名)→ 跳转(激活 pane 切到该路径)
- [ ] 选中路径的 chain 仍然自动展开
- [ ] split-view 下激活 pane 仍同步

---

## 六、避坑提示(下次会话)

### 1. 不要重新引入 dynamicRoots

`FileBrowserTreeView` 不应该有"动态 root 列表"。`rootPaths` 是稳定 prop,只来自 drives。祖先加载完全由 `useFileTree.ensureLoaded` 的 walk-up 策略处理。

### 2. ensureLoaded 不要 addRoot 任何已知 path

之前的"找不到就 addRoot"是 bug root cause。改成"找不到就 walk-up 找祖先加载祖先"。只有 path 完全不可达时才 addRoot。

### 3. chevron / row click 拆分不能合并

不能为了"少代码"把 click handler 合在一起。用户期望:icon 是 expand-only,row body 是 navigate-only。这是 UX 语义,不是 implementation detail。

### 4. parentDirectoryOf 不能引入跨盘逻辑

`parentDirectoryOf('C:/Users')` 应该是 `'C:/'`,不是 `'/'`。当前实现用 `^[A-Z]:\/?$` 检测 Windows drive root,Linux root 是 `'/'`,两者都正确 break。

### 5. onLoadStart 仍要 markLoadError false

如果 user 之前 expanded 一个失败的目录,retry 时先 markLoadError false 才合理。这是 v6.2 已经定下的语义,保留。

---

## 七、必读顺序

1. 本 handoff §一(本次完成)
2. `handoff-2026-09-26-tree-sync-v6-2.md`(上轮 + 用户反馈)
3. `src/modules/navigator/composables/use-file-tree.ts` 的 ensureLoaded
4. `src/modules/navigator/components/file-browser/file-browser-tree-view.vue` 模板 + script

---

## 八、关键决策记录

### D1: ensureLoaded 用 walk-up 而非 addRoot

之前的 addRoot 把 ancestor 当独立 root 是 tree 深度崩溃的 root cause。改成 walk-up:祖先是已经在 nodes 里的 path(比如 drive root),加载它的 children 让目标 path 出现。祖先永远是 child,depth 自然正确。

### D2: 移除 dynamicRoots 概念

v6.1 引入的 dynamicRoots ref 让 ancestors 进入 rootPaths 是错的。v6.3 完全删除这个 ref,rootPaths 只来自 props(就是 drives)。任何深度处理交给 useFileTree 的 ensureLoaded。

### D3: chevron 是独立 affordance

之前 chevron 和 row 共享一个 click handler,用户反馈"chevron click 应该只展开不跳转"。拆分成 `onChevronClick` + `onRowClick`,chevron 加 `@click.stop` 阻止冒泡到 row click。同时给 chevron 加 visible affordance(hover bg, cursor:pointer)让用户发现它可点。

### D4: row click 只 emit activate,不 toggle expand

之前的 v6.2 row click 既 emit activate 又 toggle expand。v6.3 只 emit activate,expand 由 `setSelectedPath` 的 replace-ancestors policy 自动处理(navigator 调 store.setSelectedPath(activeTab.path) 时自动展开 chain)。这样点同名目录不会 toggle collapse(用户期望"open this folder"而不是 toggle)。

### D5: 不重写 useFileTree 整个 API

keep API 兼容(`rows`, `loadedPaths`, `ensureLoaded`, `ensureAncestorsLoaded`, `addRoot`),只改 ensureLoaded 内部。v6.1/v6.2 的所有 useFileTree 测试不需重写。

---

## 九、已知 TODO

1. **chevron click 在 row hover 时不够明显** — chevron hover background 是浅灰,可能不够显眼。未来可加 active state。
2. **file vs directory emit 区分** — 当前 emit('activate') 不区分文件/目录;未来可以加 `preview` emit 给文件,父级决定是否 quick view
3. **tree 节点 scrollIntoView** — 选中节点不一定在视口内
4. **sidebar 宽度持久化** — 当前 22% hardcoded
5. **Extension API ExtensionViewLayout** 仍 fallback 'list'
6. **baseline test isolation 1 fail** — use-navigator-item-icon.test.ts 跨 Pinia 状态,与本次无关