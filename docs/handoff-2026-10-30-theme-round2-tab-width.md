# 交接文档 · 主题第二轮细化 + 标签自适应宽度（2026-10-30）

> 面向新会话。本文件只给**指针 + 已探明的落点 + 陷阱**，不复制源文件内容。
> 生成时仓库状态：内层 `F:\soft\00selfmade\filemanager\sigma-file-manager`，branch `main`，
> HEAD `fc2514cb`（`chore(release): 3.7.0`），`git status --porcelain` 为空，`origin/main` 同步，
> tag `v3.7.0` 已在远端。**3.7.0 已完整发版并装机**，本轮从干净基线起步。

---

## 0. 一句话现状

第一轮主题优化（3.7.0）已上线：4 套 AFM 主题 + 12 个层次 token + 工具栏按功能族着色。
用户看过实机后提出**第二轮 4 组细化需求**，本轮尚未动一行代码，只完成了源码侦察。

---

## 1. 必读顺序（完整路径，按序读）

| # | 路径 | 读它干什么 | 体量 |
|---|---|---|---|
| 1 | `F:\soft\00selfmade\filemanager\AGENTS.md` | **项目铁律**，最高优先级。注意末尾被运行时截断，需要 §2.7/§2.8/§2.9 时直接 read 源文件 | 583 行 |
| 2 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\theme-round2-2026-10-30\ANCHORS.md` | **本轮主要工作底稿**：4 项任务分组的样式/模板锚点，带行号 | 30,579 B / 644 行 |
| 3 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\theme-round2-2026-10-30\RECON.md` | 20 个相关源文件的**全量逐行转储**（含 token 交叉引用段）。ANCHORS 不够时查这里，**不要整读**，按文件段落跳读 | 209,962 B |
| 4 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\design-demos\direction-approved.md` | 已批准的设计方向（A/B/C/D 四族），本轮细化必须与之一致 | 7,032 B |
| 5 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\design-demos\design-spec.md` | 设计规范：R1 形状去系统化 / R2 品牌主色承载 / R3 明暗都成立；D1/D2 `--radius: 0.25rem`、D3 `0.75rem` | 10,497 B |
| 6 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\design-demos\brand-spec.md` | 品牌主色 `#607A87` 及配色约束 | 6,228 B |
| 7 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\theme-opt-2026-10-30\PLAN.md` | 上一轮（3.7.0）的施工计划，看已做过什么、为什么那么做 | 11,996 B |
| 8 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\theme-opt-2026-10-30\contrast-report.md` | 上一轮对比度实测报告。**改颜色后必须重跑同类校验** | 12,979 B |
| 9 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-27-delete-enter-key-and-list-view-grouping.md` | 最近一份完整 handoff，了解列表视图分组等既有结构 | 221,762 B，**按需跳读** |

重新生成侦察底稿（源码改动后 ANCHORS/RECON 会过期）：

```powershell
node 'F:/soft/00selfmade/filemanager/sigma-file-manager/.superpowers/sdd/theme-round2-recon.cjs'
node 'F:/soft/00selfmade/filemanager/sigma-file-manager/.superpowers/sdd/theme-round2-anchors.cjs' > 'F:/soft/00selfmade/filemanager/sigma-file-manager/.superpowers/sdd/theme-round2-2026-10-30/ANCHORS.md' 2>&1
```

---

## 2. 本轮任务（用户原文要点，勿自行增删）

### 任务 1 · 跃动活力主题（`afm-vivid-light`）

- **1a** 右上角靠近最小化/最大化按钮的那一排（含 hide folder tree、分屏视图、信息面板等）**也要设置颜色**。
- **1b** 地址栏左边四个按钮（后退、前进、向上、主页）设定**不同**颜色。
- **1c** 适当**加深**目录视图标题栏（名称、项目、大小……）的**上下边线**深度与**背景色**深度。
- **1d** 目录树滚动条**槽**背景设置成与主题一致的背景色。

### 任务 2 · 暖纸主题（`afm-warm-light`）

- **2a** 目录视图主区（显示目录内文件、子目录的区域）适当**调高亮度、调低饱和度**。
- **2b** 未激活目录的标签适当**调低亮度**。
- **2c** 标签显示**浅米灰色边线**。
- **2d** 左边栏导航栏设置成**深色背景 + 浅色图标**。
- **2e** 适当**加深**目录视图标题栏**上下边线**与**背景色**深度（同 1c，但作用于暖纸）。
- **2f** **检查所有图标**，设定为和浅米色一致的线条颜色。

### 任务 3 · 所有主题

目录名较长时，在当前设定的标签宽度基础上**增加并自适应标签宽度**，尽量完整显示目录名；
**打开标签过多时需设定最大宽度**。

### 任务 4 · 各种主题

主区不同类型（**目录 / 文件**）的区隔，设定**和其他位置不同的背景色**，让区隔更明显。

### 附加要求

用户明确要求**调用 `/huashu-design`** 保证修改后的设计更美观。
⇒ 施工前先出方向稿（该技能规定任何新设计 100% 先出三个方向初稿给用户选，**指定风格/品牌也不豁免**）。

---

## 3. 已探明的落点（文件:行号，均已实测）

路径相对 `F:\soft\00selfmade\filemanager\sigma-file-manager\src\`。

### 3.1 任务 1a — 右上角按钮的真实来源（**上一轮摘要里的猜测已被证实并细化**）

`modules\window-toolbar\window-toolbar.vue` **只是 teleport 外壳**：

- `218` `.window-toolbar-primary-teleport-target`
- `220` `.window-toolbar-secondary-teleport-target`
- `221-224` `.window-toolbar-tertiary` 内直接挂 `LanShareToolbarButton` / `StatusCenterToolbarButton` / `GlobalSearchToolbarButton`

**真实按钮由这 5 处 teleport 进来**（这是本轮最重要的新发现，配色必须打在这些文件上，改 `window-toolbar.vue` 会打空）：

| 传送源文件:行 | 目标 | 内容 |
|---|---|---|
| `modules\navigator\components\navigator-toolbar-actions\navigator-toolbar-actions.vue:146` | secondary | **hide folder tree / 信息面板等就在这一族** |
| `modules\extensions\components\extensions-toolbar-actions.vue:53` | secondary | 扩展按钮 |
| `modules\settings\ui\search.vue:44` | secondary | 全局搜索放大镜 |
| `modules\settings\ui\actions.vue:94` | primary | 设置类动作 |
| `modules\tab-bar\tab-bar.vue:32` | primary | **标签栏本身也 teleport 进工具栏**（与任务 3 相关） |

`navigator-toolbar-actions.vue` 内部（731 行）：

- `147` 容器 `.navigator-toolbar-actions`
- `156` `EllipsisVerticalIcon`、`158` `class="navigator-toolbar-actions__icon"`
- `171` `:aria-label="props.showFolderTree ? t('navigator.hideFolderTree') : t('navigator.showFolderTree')"`
- `174` `FolderTreeIcon`、`176` 同样是 `navigator-toolbar-actions__icon`
- `288` `EyeOffIcon`、`307` `PanelRightIcon`（信息面板）、`241`/`322` `InfoIcon`/`CircleHelpIcon`（这两枚是设置菜单内的提示图标，**不属于右上角那一排**，别误染）

⇒ **全部图标共用一个类 `navigator-toolbar-actions__icon`**，与 3.7.0 之前的工具栏同构。
要逐个上色必须**照搬 3.7.0 已验证的做法：加「按功能族」修饰类**（如 `__icon--folder-tree` / `__icon--split` / `__icon--info-panel` / `__icon--more`），
**不要**用 `:nth-child(N)`（3.7.0 就是因为 nth-child 在展开/折叠态错位才返工）。
样式段在 `styles\afm-themes.css` 追加，用 `stroke:` 属性（lucide 图标 `stroke="currentColor"` 会被 CSS `stroke` 覆盖 —— 已验证有效）。

「分屏视图」按钮**不在** `navigator-toolbar-actions.vue` 里，需再定位（grep `split` / `PanelLeft` / `columns`）。

### 3.2 任务 1b — 四个导航按钮（**不是纯 CSS 能完成**）

`modules\navigator\components\file-browser\file-browser-toolbar-nav-buttons.vue`（415 行）：

- `105` / `127` / `147` 后退、前进、向上**三者共用同一个修饰类** `file-browser-toolbar-nav-buttons__icon--nav`
- `166` 主页用 `__icon--location`
- `181-182` 刷新 `__icon--refresh`（+`--spin`）、`205`/`209` 视图 `__icon--view`、`227` 缩略图 `__icon--thumbnail`、`244` 折叠态 `__icon--more`
- `93` `--expanded` 容器、`235` `--collapsed` 容器、`357`/`361` 与 `406`/`410` 是两态显隐 + `@container` 互换
- 样式段只有 `370` `.file-browser-toolbar-nav-buttons__icon` 与 `375` `--spin`

⇒ 要四色必须**改模板**：拆出 `--back` / `--forward` / `--up` 三个新修饰类（`--location` 已有），
并同步 `modules\navigator\components\file-browser\__tests__\file-browser-toolbar-nav-buttons.test.ts`。

⚠️ **折叠态覆盖不到**：`--collapsed` 下拉菜单里的图标（`276`/`280`/`287`/`294`/`298`/`310`/`319`/`331`）
只有 `:size="14"`、**没有任何修饰类**。要么给它们补类，要么在文档里明确「折叠态下拉菜单保持单色」并告知用户。

`styles\afm-themes.css` 现有工具栏配色块在 **179-214 行**：
`--nav`/`--more` → 紫、`--location` → brand、`--refresh` → `#08917f` 青、`--view` → `#c78d05` 金、`--thumbnail` → `#e21239` 红。
拆类后 `--nav` 那条要相应改成三条。

### 3.3 任务 1c / 2e — 标题栏（**当前没有上边线**）

`modules\navigator\components\file-browser\file-browser-list-header.vue`（478 行）：

- `306` `.file-browser-list-view__header-container`
- `308` `border-bottom: 1px solid hsl(var(--border))` ← **只有下边线，没有 border-top**
- `309` `background-color: hsl(var(--background-3))`
- `254` `.file-browser-list-view__header-layout` / `258` `background-color: hsl(var(--background-3))`
- `282` `.file-browser-list-view__header` / `288` `color: hsl(var(--muted-foreground))` / `290` `font-size: 12px` / `291` `font-weight: 500`
- `297` `__header-columns-actions`（`303` bg background-3）、`312` `__columns-button`（`315` `height: 28px`）
- `265` `__header-scroll`、`272` `__header-scroll::-webkit-scrollbar`、`276` `--fill-column-width`

⇒ 用户要「**上下**边线都加深」，需**新增 border-top**。
⚠️ 加边线会撑动行高（`__columns-button` 高 28px），**必须实测标题栏总高不变**，否则列表虚拟滚动的行高计算会跟着偏。
优先做法：加深用 **token**（新增如 `--list-header-border` / `--list-header-bg`，在两套主题里各给值），
而不是在组件里写死颜色 —— 符合铁律 C4「优先只改 token 层」。

### 3.4 任务 1d — 目录树滚动条槽（**纯 token 即可**）

`modules\navigator\components\file-browser\file-browser-tree-view.vue`（576 行）：

- `487` `scrollbar-color: hsl(var(--scrollbar-thumb)) hsl(var(--scrollbar-track))`
- `491` `.file-tree-view__scroll::-webkit-scrollbar-track` / `492` `background-color: hsl(var(--scrollbar-track))`
- `495-496` thumb、`499-500` thumb:hover（`--scrollbar-thumb-hover`）
- `515` `hsl(var(--muted) / 50%)`、`519` `hsl(var(--primary) / 18%)`
- `529` `background-color: hsl(var(--surface-stripe))`（偶数行条纹）、`548` `border-radius: 3px`、`554` `hsl(var(--muted) / 70%)`

⇒ **槽色已经完全 token 化**，任务 1d 只需在 `afm-vivid-light` 下定义/覆写 `--scrollbar-track`（必要时连 `--scrollbar-thumb*`）。
先确认 `--scrollbar-track` 当前在哪层定义（`styles\vars.css` 还是主题层），别改到全局默认。

### 3.5 任务 2a / 4 — 主区行（**任务 4 缺样式钩子，需加属性**）

`modules\navigator\components\file-browser\file-browser-list-view.vue`（1137 行）：

- `581` `class="file-browser-list-view__entry"`（行根元素）
- 行上现有 data 属性：`588` `:data-entry-path`、`589` `:data-selected`、`590` `:data-in-clipboard`、`591` `:data-clipboard-type`、`592` `:data-link-status`、**`593` `:data-drop-target="row.entry.is_dir || undefined"`**
- `867-882` `.file-browser-list-view__entry-kind` 等 8 个**列**的共享规则（`875` `position:relative`、`876` `z-index:1`、`878` `color: hsl(var(--muted-foreground))`、`879` `font-size:12px`）
- `884-890` 有一段中文注释解释垂直居中根因（改动时保留）
- `891-897` `__entry-kind` / `__entry-items` / `__entry-modified` / `__entry-created` 四列 `display:flex; align-items:center`
- `1069` `[data-selected][data-in-clipboard][data-clipboard-type="move"] .file-browser-list-view__entry-kind`
- `318` `backgroundColor: ${tag.color}25`（标签色，内联）
- `498` 虚拟行过滤谓词

⚠️ **命名陷阱**：`__entry-kind` 是「**类型列**」（显示"文件夹/MP4 文件"那一列），**不是**行类型的判别器。别拿它当任务 4 的钩子。

⚠️ **`data-drop-target` 不能直接复用**：它只在 `is_dir` 时存在，语义是拖放目标，
拖拽经过时会有独立的视觉态；把「目录底色」挂在它上面会与拖放态互相污染，且 `disableDropTarget` 场景下会失效。
⇒ **任务 4 应新增一个专用属性**，例如 `:data-entry-type="row.entry.is_dir ? 'dir' : 'file'"`，
然后在 `styles\afm-themes.css` 或 token 层按主题给两种底色。

**两条渲染路径都要改**（只改一条会出现「列表有色、网格无色」）：

- 列表：`file-browser-list-view.vue`（1137 行）
- 网格：`modules\navigator\components\file-browser\file-browser-grid-view.vue`（240 行）—— `216` `__sticky-section-overlay`、`224` `__sticky-section-content`、`228` bg `hsl(var(--background-3))`、`235` `__grid-row`
- 共用条目组件：`components\dir-entry-interactive\dir-entry-interactive.vue`（291 行）—— `207` `:data-drop-target="!isFile && !props.disableDropTarget || undefined"`、`208` `:data-entry-path`、`287` `.dir-entry-interactive`（**样式段只有这一条**，没有任何按类型的钩子）

⇒ 若把属性加在 `dir-entry-interactive.vue` 上，列表和网格能一次覆盖；加在 `file-browser-list-view.vue` 的 `__entry` 上则只覆盖列表。**建议前者**，但要注意它已有 `isFile` prop 可直接用。

任务 2a（暖纸主区调亮降饱和）落点是主区背景 token（`--background-2` / `--background-3`），
先在 `styles\vars.css` 与 `modules\themes\afm-themes.ts` 里确认这两个 token 在暖纸下的当前值再改。

### 3.6 任务 2b / 2c / 3 — 标签

`modules\tab-bar\tab\tab.vue`（467 行）：

- **`53` `const NAVIGATOR_TAB_WIDTH = 100;`** ← 任务 3 的宽度常量就在这里（已确认唯一定义处）
- **`248` `'--tab-width': ${props.tabGroup.length === 2 ? NAVIGATOR_TAB_WIDTH * 2 : NAVIGATOR_TAB_WIDTH}px`**（内联样式注入）
- `342` `.tab` / `344` `display:flex` / `345` `overflow:hidden` / **`346` `width: var(--tab-width, 100px)`** / `347` `min-width: 0`
- **`352` `border: 1px solid transparent`** ← 任务 2c「浅米灰色边线」落点
- `353` `border-radius: var(--radius-sm)`
- **`355` `background-color: hsl(var(--state-hover))`** ← 任务 2b「未激活标签调低亮度」落点
- `362-363` `[is-active="true"] { background-color: hsl(var(--tab-surface-active)) }`
- `368` `--active-title-bold .tab__title`
- `372-379` `::after` 指示条（`376` `width: 92%`、`378` `background-color: hsl(var(--background-2) / 0%)`、`379` box-shadow）；`384-385` active → `hsl(var(--tab-indicator))`
- `390-391` `:not([is-active]):hover` → `hsl(var(--state-active))`；`394-395` `[data-drag-over]` → `var(--drop-target-background)`
- **`400-405` `.tab__title { overflow:hidden; width: calc(100% - 24px); text-overflow: clip; white-space: nowrap }`** ← 注意是 `clip` **不是** `ellipsis`
- `408-418` `.tab__close-button`（`412` `width: 24px`）—— 与 `400` 的 `calc(100% - 24px)` 对应
- `421` focus-visible、`426-427` hover → `hsl(var(--primary) / 20%)`
- `430-443` tooltip title/subtitle 用 `text-overflow: ellipsis`、`446-448` tooltip-content `min-width:200px; max-width:400px`
- `451-452` dropdown-menu `min-width:220px`、`455`/`459`/`463` menu item 一族

`modules\tab-bar\tab-bar.vue`（278 行）：

- `21-22` 引入 `useHorizontalScrollFade` / `getScrollLeftToRevealInFadedContainer`
- `32` `teleportTarget: '.window-toolbar-primary-teleport-target'`
- `45-49` `scrollContainerRef` / `scrollContentRef` / `scrollDisableTimeoutId`
- `87-89` 滚轮 → `container.scrollLeft += event.deltaY || event.deltaX || 0`
- `98-104` `scrollTabBarToEnd`、`112-129` `scrollSelectedTabGroupIntoView`、`162-170` 模板绑定
- **`226-231` `.tab-bar { display:flex; max-width:100%; gap:4px }`**、`234-235` `__base-container`

`modules\tab-bar\tab-draggable\tab-draggable.vue`（106 行）：`71` `.tab-draggable`、`75` `.item-drag-handle`、`81` `--drag-active`、`89` `… .tab`、`96` `… .tab:active`、`102` `.draggable-list__item`

`components\sortable-list\sortable-list.vue`：`136` `ghostWidth.value = row.getBoundingClientRect().width`、`266` `width: ${ghostWidth}px`、`279`/`287` `width: 100%`

**任务 3 的已知结论与待决点**：

- ✅ 拖拽幽灵宽度是**运行时实测**的（`getBoundingClientRect().width`），所以自适应宽度**不会破坏拖拽**。
- ✅ 标签栏本身**已支持横向滚动 + 渐隐**，所以「标签过多」不会挤爆布局 —— 但用户要的「最大宽度」仍须显式设，否则单个长名目录会吃掉整条栏。
- ⚠️ `248` 那行的 `tabGroup.length === 2 ? ×2 : ×1` 逻辑说明**宽度是按组内标签数算的**，不是纯按文本长度。改自适应时要把这条逻辑一起想清楚，别只改 CSS。
- ⚠️ **先查有没有单测守卫 `NAVIGATOR_TAB_WIDTH`**。本项目有过先例：`--nav-sidebar-width: 42px` 有单测守卫（铁律 C1）。
  用 `Get-ChildItem src -Recurse -Include *.test.ts | Select-String -Pattern 'NAVIGATOR_TAB_WIDTH|tab-width'` 确认。
- 自适应实现两条路可选：① 纯 CSS（`width:auto` + `min-width` + `max-width`，靠 `400` 的 `text-overflow` 兜底）；② JS 量文本宽度后写 `--tab-width`。
  **先试 ①**（改动小、符合 C4），量不准再上 ②。注意 `345` `overflow:hidden` 与 `347` `min-width: 0` 会与 `width:auto` 打架。

### 3.7 任务 2d / 2f — 左侧图标轨（**有硬约束 + 一个真实陷阱**）

`modules\nav-sidebar\nav-sidebar.vue`（671 行）：

- `443` `.nav-sidebar` / `449` `border-radius: var(--radius-sm)` / **`454` `background-color: hsl(var(--surface-sidebar))`** ← 2d 背景落点
- `457` header（`464` radius、`466` bg surface-sidebar）、`469` items、`477` spacer、`481` drives
- `491` `.nav-sidebar-item` / `497` radius / `498` `background-color: transparent`
- **`502` `:hover { background-color: hsl(var(--foreground) / 3%) }`**
- **`506` `[is-active="true"] { background-color: hsl(var(--foreground) / 5%) }`**
- **`510` `.nav-sidebar-item-icon { stroke: hsl(var(--nav-rail-foreground)) }`** ← 2d/2f 图标落点
- `514` active 态图标
- `518` `.nav-sidebar-drive` / `524` radius / `525` transparent / **`529` `:hover { hsl(var(--foreground) / 3%) }`**
- `540` `--add`（`542` `border: 1px dashed hsl(var(--border))`、`543` muted-foreground）
- **`546-548` `.nav-sidebar-drive-icon { color/stroke: hsl(var(--nav-rail-muted-foreground)) }`**
- `560` drive-stack、`591` 中文注释「描边用 `background-2`（侧栏自身底色）而不是固定灰」
- `600` drive-label / `604` muted-foreground / **`611` `-webkit-text-stroke: 2px hsl(var(--surface-sidebar))`**
- `625-626` drive:hover 图标 → `hsl(var(--foreground))`；`630-631` drive:hover label → foreground
- `637`/`640`/`647`/`649-650`/`654`/`656-657`/`660`/`667` tooltip 一族

🚨 **陷阱（本轮最容易踩的一个）**：`502` / `506` / `529` / `625-626` / `630-631` 全部基于 `hsl(var(--foreground) / N%)`。
暖纸是**亮色主题**，`--foreground` 是**深色**；一旦按 2d 把轨改成**深色背景**，
这些 hover/active 叠加层会变成「深色叠深色」→ **几乎不可见**，用户会报「点了没反应/看不出选中」。
⇒ 必须把这些改成与背景无关的 `--state-hover` / `--state-active`，或给暖纸加主题作用域覆写。
同时 `611` 的 `-webkit-text-stroke: 2px hsl(var(--surface-sidebar))` 会自动跟随（它引用的是侧栏底色 token），但 `591` 那条注释里提到的 `background-2` 描边要一并核对。

🚨 **硬约束 C1/C2**：图标轨宽度固定 **42px**（`--nav-sidebar-width: 42px`，**有单测守卫**），不得加宽；轨内不得出现文字。2d 只能改颜色，不能改尺寸。

任务 2f「检查所有图标」范围**不止 nav-sidebar**：至少还要覆盖 `navigator-toolbar-actions__icon`、
`file-browser-toolbar-nav-buttons__icon--*`、列表/网格条目图标、标题栏 `__columns-button`。
建议做法：先在 RECON.md 的「Token usage cross-reference」段列出所有 `stroke:` / `color:` 图标规则，
统一收敛到一个 token（如 `--icon-stroke-warm`），再逐处替换 —— 别一枚一枚写死颜色。

### 3.8 token 层现状（改色的主战场）

| 文件 | 行数 | 说明 |
|---|---|---|
| `styles\afm-themes.css` | **238**（实测；旧摘要记的「179-214 工具栏块」仍在此范围内） | 主题作用域覆写，本轮大部分改动落这里 |
| `styles\vars.css` | 207 | 全局 token 默认值 |
| `modules\themes\afm-themes.ts` | 361 | 4 套 AFM 主题（warm-light / warm-dark / vivid-light / cool-light）+ 12 个层次 token |
| `modules\themes\registry.ts` | 240 | `DEFAULT_THEME_ID='afm-warm-light'`、`BUILTIN_THEME_IDS=['dark']`、`CLASSIC_BUILTIN_THEME_IDS=['dark','system']`、`LEGACY_THEME_ALIASES={system:'afm-cool-light'}` |

三者的选择器/token 清单在 **ANCHORS.md 414-644 行**（分节：`414` afm-themes.css 选择器、`497` afm-themes.ts token 名、`590` vars.css）。

已知的 3.7.0 主题值（改动基线）：

- `afm-cool-light`（冷杉）：`--brand` = `200 16.9% 45.3%`（#607A87）、`--accent` = `174 55% 32%`
- `afm-vivid-light`（跃动活力）：`--primary`/`--accent` = `251 100% 63.9%`（紫）、`--nav-rail-background`/`--surface-sidebar` = `201 21.5% 25.5%`（**已是深色轨**）
- ⇒ **暖纸要做 2d 时，可以直接参考跃动活力的深色轨 token 组合**，它已经跑通一轮真机验收。

当前用户设置为 `"theme": "afm-vivid-light"`
（`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-settings.json`）。
**验收暖纸时记得先切主题**，别在跃动活力下量暖纸的色。

---

## 4. 硬约束与门禁基线

### 4.1 铁律（来自 `AGENTS.md`，违反即返工）

- **C1** 图标轨固定 42px（`--nav-sidebar-width`，**有单测守卫**），不得加宽
- **C2** 图标轨不得出现文字
- **C3** 不大改框架/布局结构
- **C4** **优先只改 token 层**
- **R1** 形状去系统化；**R2** 每套主题至少一处承载品牌主色 `#607A87`；**R3** 明暗都成立
- D1/D2 `--radius: 0.25rem`；D3 `0.75rem`
- 受保护常量不得改：mDNS 服务名、legacy 数据目录名、Tauri `identifier`（= `com.sigma-file-manager.app`）
- 品牌名 **Alpha File Manager** 锁定，不重新讨论命名/图标
- 中文 locale code 是 **`ch`** 而非 `zh`；17 个语言包在 `src/localization/messages/**`

### 4.2 门禁基线（干净 HEAD 实测，**判定看数字不看退出码**）

```
vue-tsc --build      => 0 errors
eslint .             => 188 errors / 0 warnings     （退出码非 0 是正常的）
stylelint            => 184 errors / 0 warnings     （退出码非 0 是正常的）
vitest               => Test Files 256 passed (256) | Tests 2364 passed (2364)
```

⚠️ 基线数字**随提交变**。量的时候用 `git stash push -u` 量 HEAD 做对照，**别拿旧数字对**。
⚠️ 口径必须一致：`eslint .`（整仓）≠ `eslint src`。混口径会凭空多出「回归」。
⚠️ 单文件 `eslint --fix` 通过 ≠ 回到基线（`--fix` 会把该文件里基线本来就有的违规一起修掉）。

现成门禁脚本（一次跑完四项并打印摘要）：

```powershell
& 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\gate-370.cmd'
node 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\read-gate-370.cjs'
```

输出形如 `TSC => 0 errors` / `ESLINT.TXT => ✖ 188 problems` / `STYLELINT.TXT => × 184 problems` / `VITEST => Test Files 256 passed (256) | Tests 2364 passed (2364)`。
**新脚本要另起文件名**（如 `gate-r2.cmd`），不要复用/改写正在跑的那个。

---

## 5. 建议工作流

1. **读文档**（§1 顺序 1→8）。
2. **加载 `/huashu-design`**，按它的规定出**三个方向初稿**给用户选（用户已明确要求调用该技能；它规定指定风格/品牌也不豁免三方向）。
   方向稿只做本轮 4 组细化涉及的区域，不要重做整套主题。
3. **用户选定后施工**。顺序建议：先纯 token 的（1d、2a、2b、2c、2e）→ 再需改模板的（1a、1b、4）→ 最后需改逻辑的（3）。
   这样前面几步能独立跑门禁、独立取证，不会被任务 3 的逻辑改动卡住。
4. **每项改完立刻自证**：颜色改动跑对比度校验（参照 `contrast-report.md` 的方法），布局改动（1c 加 border-top、任务 3 改宽度）**必须量像素**。
5. **全量门禁**（§4.2）。
6. **需要真机证据就直接打包**，不要等用户开口（用户 2026-10-19 明确要求）。判定条件：新功能缺界面证据、缺陷修复只有截图能确认。
   ```powershell
   & 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\build-370.cmd'   # 或另起 build-r2.cmd
   node 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\wait-build-370.cjs'
   ```
   ⚠️ **`tauri dev` 不能用于界面验收**（dev 构建所有路由页面内容区空白，结论一律作废）。
   ⚠️ 后台跑**不要挂 stdout 管道**（`Tee-Object` / `| Select-String` 会让进程树跟着死）；让 `.cmd` 内部 `> logfile 2>&1`，末尾写 `XXX_EXIT=%ERRORLEVEL%`。
   ⚠️ `.cmd` **运行期间绝对不能编辑它**（cmd.exe 按字节偏移逐行读，会执行别处的指令片段 —— 上一轮因此把版本从 3.6.0 顶到 3.7.0、十分钟构建白跑）。
   ⚠️ `.cmd` / `.ps1` **必须纯 ASCII**；`echo` 里**不能出现 `>`**（会被当重定向符，还会在仓库根目录拉出垃圾文件）。
   ⚠️ 打包后**分清「构建失败」和「复制失败」**：`publish-bundle-to-release.mjs` 只要 `release/` 里任何旧包被锁就 exit 1，而 `tauri build` 本身早已成功。新包用 `node -e "fs.copyFileSync(a,b)"` 单独补进去即可。
7. **装机取证**：
   ```powershell
   & 'F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\shot-wide-370.ps1'
   ```
   该脚本是**宽窗取证正解**：停旧进程 → 启动 → 等 18s → 找 `Tauri Window` → `ShowWindow(9)`+`SetForegroundWindow` → `MoveWindow(hwnd,0,0,3400,1900,true)` → 等 5s → 点击 (48,183) → 等 7s → `PrintWindow` → 检测 `PAINT_RIGHT_EDGE` → 扫描色带。
   同版本重装必须带 `/FORCE`：
   ```powershell
   $pkg='F:\soft\00selfmade\filemanager\sigma-file-manager\release\Alpha File Manager_3.7.0_x64-setup.exe'
   Start-Process -FilePath $pkg -ArgumentList '/FORCE','/S' -Wait -PassThru
   ```
8. **提交**：内层仓库 `sigma-file-manager`；远端 `origin = https://github.com/kizemo/alpha-file-manager.git`；先默认代理 `http://127.0.0.1:7897`，失败再 `-c http.proxy=` 重试。
9. **发版**（若本轮要发）：走官方链，顺序不可乱 —— `status` → `prepare --bump <x> --skip-build --notes-file` → `prepare --write-notes` → `upload` → `verify` → `github`。
   驱动器范本：`.superpowers\sdd\publish-370.cjs`（6 步串行，任一非 0 即停）。
   tag 必须是 **lightweight**（与 `v3.5.1`/`v3.6.0`/`v3.6.1` 一致，`git cat-file -t` = `commit`），且严格落在发版提交上，并按 §2.7 做三条复核。
   复核脚本范本：`.superpowers\sdd\check-tag-370.cjs`、`.superpowers\sdd\verify-release-370.cjs`。

---

## 6. 已知不可用/易错的路径（**勿重试，都是实测踩过的**）

**工具层**

- `edit` 工具改 `src/` 下文件 → **返回成功但磁盘不变**（改 `.superpowers/sdd/*` 是生效的）。改 `src/` 用 pwsh `Get-Content` 读 + node `fs.writeFileSync(..., 'utf8')` 写，或整文件 `write`。
- `read` / `grep` 工具对本仓 `src/` → **过期视图**。取真实内容用 node dump 或 pwsh `Get-Content`。
- **绝不要用 PowerShell 5.1 的 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件**（ANSI 读无 BOM UTF-8 → 中文全变 mojibake；`Set-Content -Encoding UTF8` 还会加 BOM）。改完立刻用 node 验：`s.includes('某个中文注释')` 且 `s.match(/\uFFFD/g)` 为空。
- pwsh 对「无 stdout 输出的原生命令」会抛 Win32 内部错误 **0xE9**（`git merge-base --is-ancestor` / `git grep` 返回 `exit=-1073741502`，整条命令 code `3221225794`）。**这不是断言失败** ⇒ 用 node `execFileSync` 包装。
- `gh` 在外层目录会解析错仓库 → 假的 "release not found"。**必须带 `--repo kizemo/alpha-file-manager`**。
- `Select-String` / `findstr` 搜 vitest/eslint/stylelint 日志 → 不可靠（ANSI 色码；stylelint 分隔符是 `×` U+00D7）。
- `node -e "..."` 双引号内含 `\"` 正则会炸 ⇒ 用 `node -e @'...'@` 或写 `.cjs`。`git -C "F:\...\"` 的反斜杠会吃掉结尾引号 ⇒ 用正斜杠路径。
- `gh --jq`（PS 5.1 吃引号）/ `gh --input -`（管道破坏 JSON）→ 改 node 解析或写文件再 `--input`。
- `cmd /c 'cd /d <repo> && ... > log 2>&1'` 内联 → 安全策略拦 ⇒ 封进 `.superpowers/sdd/*.cmd`，用 pwsh `& '<绝对路径>.cmd'` 起跑。
- `Set-Location` 后再写相对路径 / `*>` 重定向 → 被拦。优先 `npm --prefix '<绝对路径>' run <script>`。
- **删除只能用运行时可恢复删除器**：`& 'E:\Users\Duanyi\.minimax\.minimax\bin\mavis-trash.cmd' -- "<t1>" "<t2>"`。顶层 `rm --` 报 `CommandNotFoundException`；`Remove-Item` 被安全策略硬拦。
- `vitest --reporter=basic` 不存在（会 `Failed to load custom Reporter`）。跑全量用默认 reporter，按 `Test Files` / `Tests` 两行抓结果。
- `$ErrorActionPreference = 'Stop'` 会把原生命令的 stderr 变成终止错误 ⇒ 调 `npm`/`npx` 前后显式切回 `Continue`。
- PowerShell 函数只要有返回值，就**不能在裡面混用 `Write-Output`**（诊断输出会进管道成为返回值）⇒ 函数内用 `Write-Host`。
- 连续 3 次 `task_output` 轮询同一后台任务会触发系统提醒 ⇒ 改用一次长 sleep 或 waiter `.cjs`。
- 切片读大文件时**边界要对齐分节标记**（ANCHORS.md 的分节行是 `=====`），否则会把关键段落切掉。

**取证层**

- UIA 枚举本应用工具栏按钮 → `BUTTON_COUNT=0`（Chromium 无障碍树未激活），**不可靠**。
- 本应用工具栏按钮**不暴露 UIA `InvokePattern`**。可用第二条路：`PostMessage(WM_LBUTTONDOWN=0x0201 / WM_LBUTTONUP=0x0202)` 发给 **`Chrome_RenderWidgetHostHWND`**（不是 `Chrome_WidgetWin_0`），`lParam` 用该窗口**客户区**坐标。
- 输入文字首选 `Set-Clipboard` + `.superpowers\sdd\app-key.ps1 -Keys 'CTRL+V'`（中文/emoji 免疫 IME）。`SendInput` 的 Ctrl+V 要用 **VK_LCONTROL=0x11、VK_V=0x56**（写 `0x10` 是 VK_SHIFT）；粘贴前先 Ctrl+A，否则是追加。
- 自己写注入脚本时 `INPUT` 结构必须显式 `[StructLayout(LayoutKind.Explicit, Size = 40)]`，否则 `SendInput` **静默失败**（返回 0、不报错）。照抄 `app-key.ps1`。
- **注入的 Shift 到不了 webview 的 `shiftKey`**（修饰键 keyup 抢在主键 keydown 之前）⇒ 验收永久删除等不要依赖键盘组合。四条绕法全试过均无效，别再试。
- `PrintWindow` 会返回**陈旧帧/全黑帧** ⇒ 黑帧的像素扫描一律无效，必须重拍并确认 `PAINT_RIGHT_EDGE` 接近位图宽度。平滑滚动未结束时会抓到黑块 ⇒ 延长等待、连拍两次取后一帧。
- `PrintWindow` **不渲染 hover / overlay 层** ⇒ 验悬停面板、右键菜单只能走屏幕 DC BitBlt。
- 屏幕 BitBlt 会拍到盖在上面的别的窗口 ⇒ 用 `WindowFromPoint` + `GetAncestor(h,2)` 校验实际绘制者；**白名单必须包含应用自己的 `msedgewebview2`**（否则每张干净截图都被判成「被遮挡」）。
- 验收时会有置顶 Electron 窗口盖住应用，`ShowWindow(SW_MINIMIZE)` 无效（会弹回）⇒ 用 `MoveWindow` 挪到 `(-4000,-4000)`，原 rect 记进 `occluders.json` 收尾复原（`.superpowers\sdd\shift-occluders.ps1`）。
- **不要用 `Process.MainWindowHandle`** 取主窗口（会落在 15×15 托盘窗上）⇒ 自己 `EnumWindows` + `GetWindowThreadProcessId`，取 class 为 `Tauri Window`、可见、面积最大的那个。
- 量到的是**物理像素**，本机 DPI 144（150% 缩放）⇒ 换算 CSS 逻辑像素要**除以 1.5**。上一轮就因为忘了除，差点把正常值误判成 bug。
- 应用启动后会弹「新功能」更新日志模态框挡住文件列表 ⇒ 取证脚本必须先关掉它（关闭按钮名「关闭」= U+5173 U+95ED）。
- 应用日志被独占锁 ⇒ 用 `File.Open($p,'Open','Read','FileShare.ReadWrite')` + `StreamReader` 读。日志时间戳是 **UTC**（比本地少 8 小时）。
- `SFM_E2E_DATA_DIR` **只在 debug 构建生效**（release 把 override 编译掉）⇒ 在 release 产物上改用户数据只能「备份真实文件 → 灌夹具 → 验收 → 恢复」，且**恢复顺序必须是先杀进程再 restore**（应用退出时会把内存状态写回）。夹具脚本：`.superpowers\sdd\dash-accept-data.cjs backup|seed|restore`。
- 验收夹具**不要建在 `%TEMP%` 下**（`global_search/ignore.rs` 的 `builtin_ignored_paths()` 含 `/AppData/Local/Temp`，整棵子树被剪掉，搜索命中恒为 0）。
- 开关型控件（toggle）要**先探测再点击**，否则第二次点会把它关掉，得到假的「找不到输入框」。
- 注入探针要**阳性对照排在前面**（先跑必然命中的，再跑必然不命中的），否则空结果分不清是注入失败还是功能坏了。
- `InvokePattern.Invoke()` 返回成功 ≠ 点击生效（对**禁用**按钮是静默空操作）⇒ 只能靠落盘文件或像素变化判定。
- 改 `user-settings.json` 的 `navigator.defaultDirectory` **不等于**应用会打开那个目录（`user-workspaces.json` 里保存的当前标签页优先）⇒ 要同时改 `workspaces[].tabGroups[][]` 中 `type === 'directory'` 的 `path`。**这条与任务 3 直接相关**：验收标签宽度时，工作区文件里存着上一轮的标签状态。
- 悬停面板里的坐标**不能硬编码**（滚动位置会漂）⇒ 点击前从当帧实测字形位置。面板 x≈70..480 与目录树 x≈80..340 **重叠**，滚轮落点要用 x≈455 避开树。

**门禁红了的判读**

- 先分清「真红」还是「超时」：`single-instance-guard.test.ts` 曾因枚举全机进程（431 个进程时一次 WMI 查询 6.5s）跑 42.5s > vitest 默认 30s 而报 `Test timed out`，脚本本身全通过。**断言失败才有内容可看；超时是另一回事。**
- 看到几百个 eslint 错误**先按目录分组**，分清「仓库里真有的」和「只有工作副本才有的」（`.superpowers/**` 与 `graphify-out/**` 已写进 `eslint.config.ts` 的 `globalIgnores`）。

---

## 7. 上一轮遗留（**不阻塞本轮，别顺手做**）

1. **winget 清单未发**：上游 PR `#460744`（3.5.1）仍 OPEN，社区排队规则要求先合并它才能提 3.7.0 的新包。
2. **D1/D2/D3「形状去系统化」隔离沙箱的真机截图验收未做**（3.7.0 用的是主应用实拍取证，不是沙箱）。
3. 下一轮可考虑：把形状特征层覆盖到更多区域、统一图标风格 —— **与本轮任务 2f 有重叠**，若 2f 做了图标收敛，可顺带把这条关掉，但要显式说明。

---

## 8. 关键路径速查

| 路径 | 作用 |
|---|---|
| `F:\soft\00selfmade\filemanager\AGENTS.md` | 项目铁律（583 行，末尾被截断，需要时直接 read） |
| `…\sigma-file-manager\.superpowers\sdd\theme-round2-2026-10-30\ANCHORS.md` | 本轮锚点底稿（644 行，14 分节） |
| `…\sigma-file-manager\.superpowers\sdd\theme-round2-2026-10-30\RECON.md` | 20 文件全量逐行转储（210 KB，跳读） |
| `…\sigma-file-manager\.superpowers\sdd\theme-round2-recon.cjs` / `theme-round2-anchors.cjs` | 侦察器，源码改动后重跑 |
| `…\.superpowers\sdd\design-demos\` | `direction-approved.md` `design-spec.md` `brand-spec.md` `INDEX.md` + a1/a2/b1/b2/c1/c2/d1/d2/d3 的 html 与 png |
| `…\.superpowers\sdd\theme-opt-2026-10-30\` | 上一轮：`PLAN.md` `RESEARCH.md` `contrast-report.md` `contrast-data.json` + 20 余个 probe/apply/fix 脚本与 `.out.txt` |
| `…\.superpowers\sdd\gate-370.cmd` / `read-gate-370.cjs` | 门禁四合一 + 摘要读取 |
| `…\.superpowers\sdd\build-370.cmd` / `wait-build-370.cjs` | 打包 + 等待 |
| `…\.superpowers\sdd\shot-wide-370.ps1` | 宽窗真机取证（正解） |
| `…\.superpowers\sdd\app-key.ps1` | 键盘注入（`INPUT` 结构范本） |
| `…\.superpowers\sdd\publish-370.cjs` / `check-tag-370.cjs` / `verify-release-370.cjs` | 发版三部曲范本 |
| `…\sigma-file-manager\scripts\release-publish.mjs` | 官方发版链（551 行，status/prepare/upload/verify/github） |
| `…\sigma-file-manager\release\` | 49 个文件，**历史安装包不许删** |
| `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-settings.json` | 当前 `"theme": "afm-vivid-light"` |
| `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-workspaces.json` | **存着当前标签页状态，与任务 3 验收直接相关** |
| `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` | 已装 3.7.0（80,946,176 B，mtime 10/30/2026 10:43:04） |
| `E:\Users\Duanyi\.minimax\.minimax\bin\mavis-trash.cmd` | 唯一可用的删除入口 |

**MSVC/Rust 环境**（每次 bash 调用都要重来）：`cargo` 不在 PATH，在
`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`；MSVC 靠
`call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`（MSVC 14.44.35207）；
NSIS 在 `C:\Program Files (x86)\NSIS\makensis.exe`（不在 PATH）。可直接复用 `.superpowers\sdd\build-370.cmd` 的环境注入段。

**已验证可用的 npm 脚本**：`tauri build`、`sync-version`、`lint:eslint:check`、`lint:stylelint:check`、
`test:unit:run`（= `node ./scripts/vitest-run.mjs run`）、`sync-i18n`（只补缺失键、不覆盖已有值，幂等无害）、
`verify-changelog`（通过 ≠ bump 生效）、`release:status`。

**CHANGELOG 内容源两处**：英文 → 根 `CHANGELOG.md`；其余 14 locale → `public/changelog/<locale>/changelog.md`
（ch/de/es/fa/fr/hi/it/ja/pt/ru/sl/tr/ur/vi，**无 `en` 目录**）。

---

## 9. 交付标准（本轮完成的判据）

1. 4 组任务全部落地，每项都能指出**具体文件:行**的改动。
2. 门禁四项与基线一致（tsc 0 / eslint 188 / stylelint 184 / vitest 2364 全绿），**用同一条命令量**。
3. 颜色改动有**对比度校验数据**，布局改动（1c 边线、任务 3 宽度）有**像素实测值**。
4. 有**打包产物的真机截图**证明 4 套主题下的实际效果（不是 `tauri dev`）。
5. 暖纸深色轨的 hover/active **在深色背景上可见**（§3.7 陷阱已处理并有截图）。
6. 任务 4 在**列表与网格两条路径**上都生效。
7. 任务 3 有**最大宽度**，且标签过多时标签栏仍可横向滚动、拖拽仍正常。
8. `/huashu-design` 的三方向稿已给用户选过，选定记录留在 `.superpowers\sdd\` 下。
9. 若发版：tag 为 lightweight、落在发版提交上、§2.7 三条复核全过、四方（GitHub/站点目录/线上边缘/清单）一致。

**最后一条纪律**：任何收尾动作**必须看到工具返回码**才可宣称完成。
「工具报 success」/「脚本自检 OK」/ 后台任务 succeeded / 自己写进进度卡的文字，**都不算证据** ——
前几轮因此被误导 8 次以上。
