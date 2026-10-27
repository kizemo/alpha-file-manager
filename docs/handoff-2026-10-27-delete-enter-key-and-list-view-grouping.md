# 交接文档：删除确认框「回车 = 删除」+ 列表视图按类型分区 + 就地重命名 + 3.6.3 发版（2026-10-27 收尾）

> **本文是自足的接续入口。** 新会话只读本文，即可接手，不需要回放历史对话。
> 上游远程模块的完整背景见 `docs/handoff-2026-10-26-remote-transfer-progress-and-overwrite.md`；
> 项目通用约束见外层仓 `F:\soft\00selfmade\filemanager\AGENTS.md`（**必读**）。

---

## 0. 一分钟版

| 问 | 答 |
|---|---|
| 用户要什么 | ① 永久删除确认框里**直接回车要等于「删除」**（现在回车什么都不会发生）；② 目录窗口的**列表视图**要像缩略图视图那样，在「文件夹 / 图片 / 视频 / 其它文件」之间给出区隔；③ **中途追加**：按 F2 或右键「重命名」（非批量）时**不弹模态框**，直接在文件名上就地编辑，改完后用 ↑/↓ 切文件仍保持可编辑状态，便于手工连续批量改名；④ 优化与测试通过后**直接打包发版**，升级 README、新版本说明、aiec 版本记录，上传 GitHub 与 aiec |
| 这次发哪个版本 | **3.6.3**（3.6.2 已于 2026-10-26 发布）。三块优化合并进 3.6.3（3.6.3 尚未对外发布，合并比拆版本省） |
| 现在卡在哪 | **3.6.3 已经全部发完**：三块优化都通过真机验收，发版提交 `a9fed27a` 已 push 到 `origin/main`，GitHub release `v3.6.3` 与站点 aiec.fun 均已上线并回读校验通过（sha256 `FEE1F2A8…44644B`，20,716,092 B）。**剩下的只有外围事项**：winget PR（须等 3.5.1 的 `#460744` 合并）、安装包代码签名、远程条目的就地改名验证。详见第 13–17 节 |
| 已落盘的东西 | 4 笔 commit（`82d42190` / `605efae8` / `d1c96f70` / `dff656a4`）+ 优化 C 的 13 个源文件（**已通过真机验收，随本次发版提交一并提交**）+ 3.6.3 安装包 / winget 清单 / 站点文件 / 18 个发版文档（**随本次发版提交一并提交**） |
| 仓库状态 | 内层仓 `sigma-file-manager` 分支 `main`，HEAD `a9fed27a`，**已 push**，`origin/main` = `a9fed27a98708c8465b2ecc96f67f4c0a7c67eb5`（用 `gh api .../commits/main --jq '.sha'` 核对一致）；上游 `https://github.com/kizemo/alpha-file-manager.git`；工作区干净 |
| 最容易踩的坑 | ① **真机验收只能用 `tauri build` 产物**，`tauri dev` 的路由页内容区全空白，界面结论一律作废；② 打包前必须停 `alpha-file-manager` 进程**并把 `src-tauri/target/release/alpha-file-manager.exe` 移走**（只杀进程不够，上一轮就是这么被 `os error 5` 卡住的）；③ `eslint .` 是整仓口径，基线 **188 errors**；④ `.cmd` 里 `echo` 不能出现 `>`；⑤ `.ps1` / `.cmd` 必须纯 ASCII（中文要用 `[char]0x....` 拼）；⑥ `app-key.ps1` 只认**裸名 token**（传 `'{F2}'` 会静默不发送）；⑦ WebView2 里 UIA 的 `FocusedElement` 不可信，判「选中」要看状态栏「已选择 N 个项目」；⑧ **会话锁住时 `SendInput` / `SetCursorPos` 全废（err=5），但 `PostMessage` 合成鼠标消息与 `WM_KEYDOWN` 照样能穿透到渲染进程** ⇒ 键盘链路（F2 / 回车 / ↑↓ / Esc）也能在锁屏状态下真机验收；⑨ 文件浏览器右键菜单里**没有文字「重命名」项**，改名入口是 quick-actions 行**第 1 个无名按钮（铅笔）**，要靠 UIA `InvokePattern` 或按坐标点 |

---

## 1. 任务是什么（用户原话）

> 1. 当前选中文件或目录后，选中文件，使用快捷键 shift+del 或者使用鼠标从菜单选择「从驱动器进行删除」，弹出确定窗口（有两个按钮「取消」和「删除」），直接回车，不会删除。希望回车默认映射的是「删除」。
> 2. 在目录窗口的列表视图，希望类似缩略图视图，在文件夹、图片、视频、其它文件等之间，可以有一个区隔，便于在自己需要查找的目录或文件中快速查找和定位。
> 3. 在优化、测试通过之后，直接打包发版，升级 readme 文档、新版本说明、aiec 版本号文件，上传 github 和 aiec。

拆成三件事：

| # | 事项 | 性质 | 归属 |
|---|---|---|---|
| A | 永久删除确认框回车映射 | 前端小改（1 个新模块 + 1 个组件） | 本会话直接写 |
| B | 列表视图按类型分区 | 前端中等改造（虚拟行模型 + 渲染 + 顺序一致性） | 派 worker 并行 |
| C | 3.6.3 发版（打包 + 文档 + 双端上传） | 流程性，有严格顺序铁律 | 本会话做 |

---

## 2. 项目背景与当前状态

### 2.1 目录结构（**两个 git 仓，别搞混**）

| 路径 | 说明 |
|---|---|
| `F:\soft\00selfmade\filemanager` | **外层仓**。只放 `AGENTS.md`、未跟踪的 `docs/blog/`、`docs/marketing/`、`docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md`。**不要在这里做功能改动** |
| `F:\soft\00selfmade\filemanager\sigma-file-manager` | **内层仓**，真正的 Alpha File Manager 项目。所有代码、文档、发版都在这里 |

### 2.2 版本现状

- `package.json` = **3.6.3**；`src-tauri/Cargo.toml`、`src-tauri/default-file-manager-common/Cargo.toml`、`src-tauri/default-file-manager-launcher/Cargo.toml` 三处都 = **3.6.3**（`src-tauri/tauri.conf.json` 的 version 指向 `"../package.json"`，不用改）。**版本号已 bump 完毕，不要再 bump**（重打包要加 `--bump none`）。
- **本次发版目标版本 = 3.6.3**（`release:prepare --bump patch` 会把三处一起 bump，记得用 `npm run sync-version` 或让脚本处理）。
- 3.6.2 已发布：GitHub `kizemo/alpha-file-manager` release `v3.6.2`（非 draft / 非 prerelease）、aiec.fun 站点 `version.json` 已升到 3.6.2、winget 3.6.2 清单四件套已备好。
  - 3.6.2 安装包：`release/Alpha File Manager_3.6.2_x64-setup.exe`，20,671,224 B，sha256 `65B3AF7CE60755D6163A1C4E0ED42C41A7D97FEAD02BD887500FF5E000BFDC89`（GitHub 资产 digest / 站点 `version.json` / winget `InstallerSha256` 四方一致）。
- `release/` 目录里堆了 2.2.0 → 3.6.3 的历史安装包，**都在，别删**（winget 清单要按 sha256 比对这里的实际文件）。
- **3.6.3 的产物**（安装包 sha / 字节数 / winget 清单 / 站点文件）见第 14 节。

### 2.3 项目形态要点

- Tauri 桌面应用（前端 Vue 3 + TS + Pinia，UI 组件库是 **reka-ui**，不是 Radix 直用），Rust 后端在 `src-tauri/`。
- 目录窗口的「文件浏览器」组件群在
  `src/modules/navigator/components/file-browser/`。
- i18n：17 个语言包在 `src/localization/messages/*.json`（**中文的 locale code 是 `ch` 不是 `zh`**）；约定是**只有 `ch.json` 用中文，其余 16 个沿用英文原文**；17 个 key 集合必须完全一致。
  已有 `locale-json-format.test.ts` 要求每个文件 == `JSON.stringify(parsed, null, 2) + '\n'`。
- 根 `CHANGELOG.md`（英文）+ `public/changelog/<15 个 locale>/changelog.md`（每个语言包一份）。

---

## 3. 本轮已完成到哪一步

> 第 4 / 5 节是**已经落地**的定稿方案（代码按它实现并已提交）；第 13 节是中途追加的优化 C 的规格与实现，第 14 节是实测结果与发版资产现状，第 15 节是剩余步骤。

| 事项 | 状态 | 产物 / 证据 |
|---|---|---|
| 定位 A 的根因与实现位置 | ✅ 完成 | 见第 4 节，带文件路径与行号 |
| 定位 B 的实现位置与复用点 | ✅ 完成 | 见第 5 节，带文件路径与行号 |
| A 的方案定稿 | ✅ 完成 | 见第 4.4 节，含可直接抄的代码 |
| B 的方案定稿 | ✅ 完成 | 见第 5 节，含全部一致性要求 |
| **A 的代码 + 单测** | ✅ **已完成并提交** | `utils/permanent-delete-confirm-keyboard.ts`（13 用例）+ `permanent-delete-confirm-dialog.vue`；commit `82d42190` / `605efae8` |
| **B 的代码 + 单测** | ✅ **已完成并提交** | 分区行模型 + `createGroupedListRows()`；commit `d1c96f70` / `dff656a4` |
| **发版文档成因描述修正** | ✅ 完成 | 18 个文件（全仓 grep 旧表述 = No matches found） |
| **真机验收 A（裸回车 = 删除，Case A/B/C/D）** | ✅ **PASS** | `.superpowers\sdd\accept-363-enter-delete.ps1` + 截图 |
| **真机验收 B（列表视图分区，4 条断言）** | ✅ **PASS** | `.superpowers\sdd\accept-363-list-grouping.ps1` + 截图 |
| **优化 C 规格 + 实现 + 单测** | ✅ **代码完成（随本次发版提交一并提交）** | 见第 13 节；16 条新单测；全量 248 files / 2275 tests 全过 |
| **优化 C 门禁** | ✅ 通过 | `eslint .` = 188 errors / 0 warnings（= 基线）；`npm run ts` exit 0 |
| **3.6.3 重打包 + winget 清单 + 站点同步** | ✅ 完成 | 见第 14 节（新 sha / 字节数） |
| **优化 C 真机验收** | ✅ **全部 PASS**（右键路径 / F2 / 回车 / ↓ / ↑ / Esc / 缩略图视图） | 见第 13.6 节 |
| **发版提交 + push + 双端上传** | ✅ **已完成** | commit `a9fed27a` → `origin/main`；GitHub release `v3.6.3` + 站点 aiec.fun 线上均 3.6.3，`release:status` / `release:verify` 版本 / 大小 / sha256 三项一致 |

---

## 4. 优化 A：永久删除确认框「回车 = 删除」

### 4.1 现象与根因

- 入口：选中文件/目录 → `Shift+Del` 或右键菜单「从驱动器进行删除」→ 弹出确认框（按钮：**取消** / **删除**）→ 裸回车**无任何反应**。
- 组件：`src/modules/navigator/components/file-browser/permanent-delete-confirm-dialog.vue`（全文 137 行）。
  - 按钮在第 89–102 行：`<Button variant="outline">取消</Button>` + `<Button variant="destructive">删除</Button>`；
  - `onConfirm()` 在第 55–57 行，只由按钮 `@click` 触发。
- **根因**：对话框用的是 reka-ui 的 `Dialog` / `DialogContent`（`src/components/ui/dialog/index.ts` 第 11 行导出 `DialogContent`，实现在 `src/components/ui/dialog/dialog-content.vue`）。
  reka-ui 打开对话框时把**焦点落在对话框容器本身**，两个按钮都没被激活 ⇒ 裸回车既不点「取消」也不点「删除」，事件被吃掉。
  这与项目里其它确认框的行为一致，不是回归。

### 4.2 定稿方案

新增一个**纯判定模块** + 在组件里挂 **document capture 阶段** 的 keydown 兜底。

判定规则（全部纯逻辑，便于单测）：

1. 只认 `Enter`；
2. `defaultPrevented` 为真 → 不响应（**顺带防止同一事件被多个实例重复确认**）；
3. `isComposing`（输入法组合中）→ 不响应；
4. **焦点落在可交互控件上 → 交给浏览器原生行为**（焦点在「取消」上回车 = 取消；焦点在「删除」上回车 = 原生点击删除）；
   只有焦点**不在**控件上（对话框容器 / body，即用户「刚打开对话框直接回车」的场景）→ 判定为「确认删除」。

为什么用 capture 阶段：`capture` 在 document 上是最早的拦截点，
`event.preventDefault() + event.stopPropagation()` 能保证**应用级的回车快捷键不会同时生效**（否则可能出现「删除 + 打开条目」双动作）。

### 4.3 需要新建的文件

`src/modules/navigator/components/file-browser/utils/permanent-delete-confirm-keyboard.ts`

```ts
// SPDX-License-Identifier: GPL-3.0-or-later
// License: GNU GPLv3 or later. See the license file in the project root for more information.
// Copyright © 2021 - present Aleksey Hoffman. All rights reserved.

/**
 * FORK-MODIFICATION 2026-10-27：永久删除确认框的「回车 = 删除」判定。
 *
 * 背景：reka-ui 的 DialogContent 打开时把焦点落在对话框容器本身，
 * 两个按钮（取消 / 删除）都没被激活 ⇒ 裸回车什么都不发生。
 * 用户要求「回车默认映射的是删除」。
 */

/** 焦点在这些元素上时，回车交给浏览器原生行为处理。 */
export const PERMANENT_DELETE_CONFIRM_INTERACTIVE_SELECTOR
  = 'button, a[href], input, textarea, select, [role="button"], [role="menuitem"]';

export interface PermanentDeleteConfirmEnterContext {
  key: string;
  defaultPrevented: boolean;
  isComposing: boolean;
  /** 焦点当前是否落在可交互控件（按钮 / 链接 / 输入框等）上。 */
  isFocusOnInteractiveControl: boolean;
}

/** 目标元素是否属于「可交互控件」。非浏览器环境下一律当作「不是控件」。 */
export function isPermanentDeleteConfirmInteractiveTarget(target: EventTarget | null): boolean {
  if (!target || typeof Element === 'undefined' || !(target instanceof Element)) {
    return false;
  }

  return target.closest(PERMANENT_DELETE_CONFIRM_INTERACTIVE_SELECTOR) !== null;
}

/** 这次回车是否应该触发「确认删除」。 */
export function shouldConfirmPermanentDeleteOnEnter(
  context: PermanentDeleteConfirmEnterContext,
): boolean {
  if (context.key !== 'Enter') {
    return false;
  }

  if (context.defaultPrevented || context.isComposing) {
    return false;
  }

  return !context.isFocusOnInteractiveControl;
}
```

### 4.4 组件改动（`permanent-delete-confirm-dialog.vue`）

在 `<script setup>` 里：

```ts
import { computed, onBeforeUnmount, onMounted } from 'vue';
import {
  isPermanentDeleteConfirmInteractiveTarget,
  shouldConfirmPermanentDeleteOnEnter,
} from './utils/permanent-delete-confirm-keyboard';

// ...（其余原有逻辑不动）

function handleDocumentKeydown(event: KeyboardEvent) {
  if (!props.open) {
    return;
  }

  const activeElement = typeof document !== 'undefined' ? document.activeElement : null;
  const shouldConfirm = shouldConfirmPermanentDeleteOnEnter({
    key: event.key,
    defaultPrevented: event.defaultPrevented,
    isComposing: event.isComposing,
    isFocusOnInteractiveControl: isPermanentDeleteConfirmInteractiveTarget(activeElement),
  });

  if (!shouldConfirm) {
    return;
  }

  // capture 阶段拦截：别让应用级的回车快捷键同时生效。
  event.preventDefault();
  event.stopPropagation();
  onConfirm();
}

onMounted(() => {
  document.addEventListener('keydown', handleDocumentKeydown, true);
});

onBeforeUnmount(() => {
  document.removeEventListener('keydown', handleDocumentKeydown, true);
});
```

**模板不要改**：按钮顺序（取消 / 删除）、文案、样式全部保持原样。
**不要**额外把焦点自动移到「删除」按钮上 —— 现在的方案已经让「刚打开就回车」= 删除，
移动焦点会让破坏性按钮常驻焦点环，属于额外的危险面，不做。

### 4.5 单测（新建）

`src/modules/navigator/components/file-browser/utils/__tests__/permanent-delete-confirm-keyboard.test.ts`

纯函数用例（不依赖 DOM）：

| 用例 | 输入 | 期望 |
|---|---|---|
| 焦点不在控件上 + Enter | `isFocusOnInteractiveControl: false` | `true`（= 确认删除） |
| 焦点在按钮上 + Enter | `isFocusOnInteractiveControl: true` | `false`（交给原生） |
| 已被处理过的回车 | `defaultPrevented: true` | `false` |
| 输入法组合中 | `isComposing: true` | `false` |
| 其它按键 | `key: 'Escape'` | `false` |

DOM 相关用例（`isPermanentDeleteConfirmInteractiveTarget`）：`<button>` → `true`；`<div>` → `false`；`null` → `false`。
若 vitest 环境不是 jsdom，用 `if (typeof document === 'undefined') return;` 跳过。

### 4.6 验收要点

- `Shift+Del` → 确认框弹出 → **直接回车** → 应当执行删除（不再无反应）。
- Tab 到「取消」再回车 → 仍然是取消（原生行为，不回归）。
- 鼠标点「删除」→ 正常删除。
- 焦点在「删除」按钮上回车 → 正常删除，且**不能触发两次**（`defaultPrevented` 守卫 + 原生 click）。

---

## 5. 优化 B：列表视图按类型分区

### 5.1 可复用的现成资产（不要重新设计）

| 资产 | 路径 | 说明 |
|---|---|---|
| 分组函数 | `src/modules/navigator/components/file-browser/file-browser-entry-groups.ts` | `groupFileBrowserEntries()` 第 21 行 → `{ dirs, images, videos, others }`；`isFileBrowserImageEntry` 第 9 行；`isFileBrowserVideoEntry` 第 15 行；`getFileBrowserGridEntryOrder` 第 50 行；`getFileBrowserVisualEntryOrder` 第 60 行 |
| 区隔标题组件 | `src/modules/navigator/components/file-browser/file-browser-grid-section-bar.vue` | props `{ label, count }`，slot 名 `icon` |
| 网格视图用法 | `src/modules/navigator/components/file-browser/file-browser-grid-view.vue` | `getSectionLabel()` 第 40 行用 `t('fileBrowser.folders'\|'fileBrowser.images'\|'fileBrowser.videos'\|'fileBrowser.otherFiles')`；icon 用 lucide `FolderIcon` / `FileImageIcon` / `FileVideoIcon` / `FileIcon` |
| i18n | `src/localization/messages/*.json` | 上述 4 个 key **17 个语言包都已存在**，**不要新增任何 i18n key** |

### 5.2 涉及的虚拟滚动实现（**最容易踩坑的地方**）

| 文件 | 关键位置 | 要点 |
|---|---|---|
| `src/modules/navigator/components/file-browser/utils/file-browser-virtual-rows.ts` | 联合类型第 40 行；`FileBrowserListVirtualRow` 第 18 行；`FileBrowserGridSectionVirtualRow` 第 24 行 | 要新增 `FileBrowserListSectionVirtualRow`（`type: 'list-section'`），并把它加进 `FileBrowserVirtualRow` 联合 |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-virtual-layout.ts` | `GRID_SECTION_HEADER_HEIGHT = 42` 第 48 行；`getRowScrollMarginTop` 第 95 行；`createListRows` 第 178 行；`createGridRows` 第 257 行；`createFileBrowserVirtualRows` 第 312 行；`activeGridSectionRow` 第 419 行 | 见下方四条要求 |
| `src/modules/navigator/components/file-browser/file-browser-list-view.vue` | `visibleRows` 第 433 行（`filter(row => row.type === 'list-entry')`）；行渲染第 465–544 行 | 区隔行要能在 `visibleRows` 里活下来并被渲染；行高用 `row.size` |
| `src/modules/navigator/components/file-browser/file-browser-content-body.vue` | 第 72–78 行 | `layout === 'grid'` → GridView，**`v-else` → ListView（tree 也走 ListView）** ⇒ **tree 布局不能分区** |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-keyboard-navigation.ts` | `getNavigationEntries()` 第 66 行 | 它调 `getFileBrowserVisualEntryOrder(entries, layout())` |
| `src/modules/navigator/components/file-browser/composables/use-file-browser-selection.ts` | 待读 | 范围选择用原始 entries 顺序还是视觉顺序，**必须自己读一遍确认** |

### 5.3 四条硬性要求

1. **区隔行必须进虚拟行模型**：新增 `list-section` 行类型，由 `createListRows`（或新函数）在分组后按序插入，
   每个区隔行有自己的 `start` / `size`，**offset 累加必须与实际渲染高度一致** ——
   虚拟行给的 height 与 CSS 的 min-height 不是同一个数就会滚动错位（本项目历史踩过多次）。
2. **`getRowScrollMarginTop` 必须把 `list-section` 当 list 行**（返回 `virtualContentOffset`），
   否则「滚动到某条目」时区隔行高度没算进去，定位会偏。
3. **视觉顺序与键盘导航、Shift 范围选择必须一致**：
   列表视图显示分组顺序后，`getFileBrowserVisualEntryOrder` 对 `layout === 'list'` **也必须返回分组顺序**；
   若 `use-file-browser-selection.ts` 的范围计算用的是原始 entries 顺序，也要改成视觉顺序，
   否则 Ctrl+Shift / Shift+↑↓ 选出来的范围在画面上横跨不相邻的分组。
   **这是最容易被漏、也最容易被用户直接感知的一条。**
4. **tree 布局保持原样**（不分区、顺序不变）；区隔行**不可被点选**（不进选中集合、右键不弹条目菜单）；
   用户设置的排序**在组内继续生效**（分组只改跨组顺序，不改组内相对顺序）。

> sticky 段头（滚动时顶部吸附当前分区标题，像网格视图那样）：**风险高可以只做随内容滚动的内联区隔标题**，
> 但必须在最终报告里写明「sticky 未做」，不要默默不做。

### 5.4 单测

扩展既有测试文件（**不要新建重复的**）：

- `src/modules/navigator/components/file-browser/composables/__tests__/use-file-browser-virtual-layout.test.ts`
  - 断言 `layout: 'list'` 时产出的行序列里出现 `list-section`；
  - 断言区隔行的 `start` 与前一行 `start + size` 连续；
  - 断言 `layout: 'tree'` 时**不**出现 `list-section`。
- `src/modules/navigator/components/file-browser/__tests__/file-browser-entry-groups.test.ts`
  - 断言 `getFileBrowserVisualEntryOrder(entries, 'list')` 返回分组顺序，且与 list 视图渲染顺序一致。

### 5.5 派 worker

任务书已经写好，直接用：

```
agent_name: worker
prompt: 读取 F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\task-list-view-grouping.md 并严格按其中的内容执行
```

任务书里已包含：目标、已确认事实、硬性要求、约束（不改 Rust / 不碰发版文件 / 不用 PowerShell 改含中文的文件 / lint 与单测口径）、验收报告格式。
**不要自己再写一遍任务书**，避免两份规格漂移。

---

## 6. 下一轮执行顺序（**已执行完毕 —— 保留作历史记录**）

> ⚠️ 本节是**第一轮**的计划（A / B 的实现与提交、两套真机验收、打包），**这些都已完成**。
> 现在的剩余步骤以**第 15 节**为准。

| # | 动作 | 命令 / 说明 |
|---|---|---|
| 1 | **清上下文** | 发 `/new`（本会话上下文已 109k+；工具预算钩子按 turn 计数，新 turn 归零） |
| 2 | 读约定 | `F:\soft\00selfmade\filemanager\AGENTS.md` + 本文 |
| 3 | 落盘优化 A | 写 `utils/permanent-delete-confirm-keyboard.ts` + 改 `permanent-delete-confirm-dialog.vue` + 写单测（4.3 / 4.4 / 4.5） |
| 4 | 派 worker 做优化 B | 任务书见 5.5；同时**自己不要碰** `file-browser-*` 那批文件，避免写冲突 |
| 5 | 跑单测 | `npm --prefix F:\soft\00selfmade\filemanager\sigma-file-manager run test`（**不要 `--reporter=basic`**，本仓库无此 reporter 会直接中止）；按 `Test Files` / `Tests` 两行取结果 |
| 6 | 跑 lint | `npm --prefix F:\soft\00selfmade\filemanager\sigma-file-manager run lint`（= `eslint .` 整仓，基线 **188 errors / 0 warnings**；`eslint src` 基线 **175**）。单文件 `--fix` 通过 ≠ 回到基线，**必须再跑一次全量对照** |
| 7 | 合并审查 | worker 交付后逐项核对第 5.3 节四条硬性要求；`git diff` 人工过一遍 |
| 8 | 提交 | `git -C ... commit -F <提交信息文件>`（用 `-F` 文件形式，别在 PowerShell 里写带引号/反引号的长消息） |
| 9 | **真机验收** | 见下方「打包防锁」小节；**必须用 `tauri build` 产物** |
| 10 | 发版 3.6.3 | 见第 7 节 |

### 打包防锁（上一轮的实证坑）

```powershell
# 1) 停进程
Get-Process alpha-file-manager -ErrorAction SilentlyContinue | Stop-Process -Force
# 2) 把会被链接阶段覆盖的产物移走（只杀进程不够！）
node -e "const fs=require('fs'),path=require('path');const s=path.join(process.argv[1],'src-tauri','target','release','alpha-file-manager.exe');const d=path.join(process.argv[1],'.superpowers','sdd','superseded');if(fs.existsSync(s)){fs.mkdirSync(d,{recursive:true});fs.renameSync(s,path.join(d,'alpha-file-manager.exe.prev'));console.log('moved')}else console.log('absent')" F:\soft\00selfmade\filemanager\sigma-file-manager
# 3) bundle/nsis 里的旧版本验收包也要移出去
#    publish-bundle-to-release.mjs 会把 bundle/nsis/* 覆盖回 release/，
#    会把已发布版本的 sha256 顶掉 → winget 清单变红
```

打包脚本必须**纯 ASCII**、放 `.superpowers/sdd/*.cmd`、**新建文件名**（不要复用/编辑正在运行的那个，
cmd.exe 按字节偏移逐行读，运行中编辑会导致执行错位的片段）、
内部用 `> logfile 2>&1` 重定向到文件（后台跑不要挂 stdout 管道，管道一断进程树跟着死）。

---

## 7. 3.6.3 发版流程（**命令与顺序铁律仍然有效；文档清单的实际改动见第 14.5 节**）

### 7.1 权威文档（先读）

- `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md`（五条命令、五处状态、缓存与回滚语义）
- `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\version-info.md`（`version.json` 字段语义、`history[]` 归档规则）

### 7.2 顺序铁律

1. **`verify-changelog` 必须在升版本号之前跑**（它校验「当前版本有 changelog 条目」）。
2. **`release:prepare --write-notes` 必须在 `release:prepare`（打包）之后跑**（它要读产物大小与 sha256）。

### 7.3 命令

```powershell
$repo = 'F:\soft\00selfmade\filemanager\sigma-file-manager'
npm --prefix $repo run verify-changelog
npm --prefix $repo run release:prepare -- --bump patch --notes-file ./_notes.md
npm --prefix $repo run release:prepare -- --write-notes ./_notes.md
npm --prefix $repo run release:upload
npm --prefix $repo run release:github
npm --prefix $repo run release:status
npm --prefix $repo run release:verify
npm --prefix $repo run check:winget
```

> `release:prepare` 默认会把安装包复制到仓库 `release/`（**不上传**，可以放心跑）。
> 仓库根目录的 `publish-bundle-to-release.mjs` **只复制不发布**。
> 若最后一步「复制到 release/」失败（旧的包被别的进程锁住），**先分清是构建失败还是复制失败**：
> 新包用 `node -e "fs.copyFileSync(a,b)"` 单独补进 `release/` 即可，不必重跑整个打包。

### 7.4 文档清单（3.6.3 要改的文件）

| 文件 | 改什么 | 约定 |
|---|---|---|
| `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` | 加 `## [3.6.3] - October 2026` 条目 | **英文**，Summary + What changed |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\public\changelog\<15 个 locale>\changelog.md` | 各加 3.6.3 条目 | `ch` 用中文正文，其余 14 个**沿用英文原文**、仅 H1 本地化 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\README.md` | 「What's new in 3.6.2」整段替换成 3.6.3 | 英文 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\README.zh-CN.md` | 「🆕 3.6.2 新增了什么」整段替换成 3.6.3 | 中文 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\_notes.md` | 3.6.3 中文发布说明 | 供 `version.json.notes` 与 GitHub release 正文共用 |
| `E:\办公文件\L1网站\alpha-file-manager\version.json` | 由 `release:prepare` 写；`history[]` 归档 3.6.2 | 见 `docs/operations/version-info.md` |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\README.md` | 状态表加 3.6.3 行 | PR 流程同步 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\manifests\k\kizemo\AlphaFileManager\3.6.3\` | 4 个清单文件 | 从 3.6.2 那套生成（改版本号 / ReleaseDate / URL / sha256） |

### 7.5 双端上传

- **aiec.fun**：站点源目录 `E:\办公文件\L1网站\alpha-file-manager\`；上传脚本
  `E:\办公文件\L1网站\_upload_alpha_files.py`（用 `E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe`，含 paramiko）。
  SFTP 凭据在 `E:\办公文件\L1网站\.vscode\sftp.json` —— **绝不外泄**。
  服务器路径 `/var/www/wordpress/alpha-file-manager/`。脚本会做服务器端 md5+size 逐个校验，再做公网回读。
- **GitHub**：`kizemo/alpha-file-manager` release `v3.6.3`，资产名 `Alpha_File_Manager_3.6.3_x64-setup.exe`。
  查资产 digest 用落盘 JSON + node 解析（PS 下 `gh --jq` 的引号会被打散）：
  ```powershell
  gh api repos/kizemo/alpha-file-manager/releases --paginate | Out-File -Encoding utf8 "$env:TEMP\ghrels.json"
  node -e "let s=require('fs').readFileSync(process.argv[1],'utf8');if(s.charCodeAt(0)===0xFEFF)s=s.slice(1);for(const r of JSON.parse(s).slice(0,4))console.log(r.tag_name,r.published_at,r.assets.map(a=>a.name+' '+a.size+' '+a.digest).join(' | '))" "$env:TEMP\ghrels.json"
  ```
- **push**：GitHub 通道在直连与代理间会翻转，两条都试（`.superpowers/sdd/push-with-fallback.cjs` 已存在）。
  核对远端 SHA **不要用 `git ls-remote`（走代理会挂住）**，用：
  ```powershell
  gh api repos/kizemo/alpha-file-manager/commits/main --jq '.sha'
  ```

---

## 8. 本项目的硬约束（摘要，完整版在 `F:\soft\00selfmade\filemanager\AGENTS.md`）

1. **面向用户的输出一律简体中文**；代码标识符 / 路径 / 命令保持原样。
2. 品牌名固定 **Alpha File Manager**；**不得改** mDNS 服务名、legacy 数据目录名、Tauri `identifier`（`com.sigma-file-manager.app`）。
3. **真机验收只能用 `tauri build` 的打包产物**。`tauri dev` 构建下应用外壳正常但**所有路由页内容区全空白**
   （`RouterView` 挂载却不渲染子组件）⇒ 在 dev 构建上做的界面验收结论**一律作废**。
4. **需要装机实测就直接打包 exe，不要等用户开口**（用户 2026-10-19 明确要求）。
   判定条件是「结论需要真机才能成立」。命令 `npm run tauri:build`。
5. **改任何 `.rs` 前先扫孤立 CR 字节**；`cargo test --lib` 的红线：被测试链接到的代码不得碰
   `tauri::AppHandle` 或 `tauri::async_runtime::*`。
6. **绝不用 PowerShell 的 `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件**
   （PS 5.1 按 ANSI 读无 BOM 的 UTF-8，中文会变乱码并被写回）。
   安全写法：编辑工具，或 `[System.IO.File]::ReadAllText/WriteAllText` + 显式 UTF8，
   或 node 的 `fs.readFileSync/writeFileSync(..., 'utf8')`。写完立刻验 `U+FFFD === 0`。
7. **`.ps1` / `.cmd` 必须纯 ASCII**；PS 5.1 按 ANSI 读无 BOM 文件，中文只能作命令行参数传。
   **`.cmd` 的 `echo` 里不能出现 `>`**（会被当重定向符，3.6.1 那轮在仓库根目录多出一个 `3.6.0)` 垃圾文件）。
   **`.cmd` 里调用另一个 `.cmd` 必须加 `call`。**
8. **`.cmd` 运行期间绝对不能编辑它**（cmd.exe 按字节偏移逐行读，改文件会执行错位的指令片段；
   3.6.2 那轮因此把版本从 3.6.0 顶到 3.7.0，十分钟构建白跑）。
9. **门禁基线用同一条命令量**：`eslint .`（整仓，基线 188）/ `eslint src`（基线 175）。
   用 `git stash push -u` 量 HEAD，别拿旧数字对。
10. **删除文件只能走** `E:\Users\Duanyi\.minimax\.minimax\bin\mavis-trash.cmd -- "<路径>"`。
11. **验收夹具不要放 `%TEMP%`**（`global_search/ignore.rs` 的 `builtin_ignored_paths()` 含 `/AppData/Local/Temp`，
    且 `matches_plain_ignored_path` 对 `/` 开头的模式做 `path.contains(...)` ⇒ 整棵子树被剪掉，命中恒为 0）。
12. **写完检查/校验脚本必须造一份坏数据做阴性对照。**
13. **不要用 `Process.MainWindowHandle` 取本项目主窗口**（会落在 15×15 的托盘窗上）；
    要 `EnumWindows` + 过滤 class 为 `Tauri Window`、可见、面积最大的那个。
14. **量到的是物理像素**：本机 DPI 144（150% 缩放），换算 CSS 逻辑像素要除以 1.5。
15. **UIA 取元素不能只取有名字的**；`InvokePattern.Invoke()` 返回成功 ≠ 点击生效（禁用按钮是静默空操作）。
16. **注入探针要「阳性对照排在前面」**；`SendInput` 的 `INPUT` 结构必须
    `[StructLayout(LayoutKind.Explicit, Size = 40)]`，否则静默失败。
    输入文字首选 `Set-Clipboard` + `app-key.ps1 -Keys 'CTRL+V'`（粘贴前先 Ctrl+A，否则是追加）。
17. **PowerShell 函数只要有返回值，就不能在里面混用 `Write-Output`**（诊断输出会进管道变成返回值的一部分）。

---

## 9. 本轮的环境事故（务必知道）

本轮（会话 `mvs_e65719a4a6c14ca5875383efbfdcc3c8`）在第 20 次工具调用后，
被 `E:\Users\Duanyi\.minimax\.minimax\maintenance\tool_budget_hook.py` 拦死：
`read` / `grep` / `bash` / `task` / `write` 全部返回「预算超限」的拒绝消息，
而每试一次还会让上下文继续涨（87k → 109k），形成死循环。

**判别与解法**：

- 症状是「工具调用被整段拒绝消息顶替」，不是脚本出错；
- 按 turn 分组计数 ⇒ **发一条新消息即可恢复**；
- 上下文已 100k+，**建议直接 `/new`**（新会话 + 清上下文 = 单轮账单最大杠杆）；
- 单轮工具调用控制在 **10 次以内**；要取信息就**一次取齐**（一条命令做完多件事 /
  一次 grep 拿全部匹配 / read 用 `offset`+`limit` / 长结果落盘只回路径+结论）。

**下一次开工时的第一条建议：先 `/new`，再读本文与 `AGENTS.md`。**

---

## 10. 未决 / 待用户决定

| # | 事项 | 状态 |
|---|---|---|
| 1 | winget PR `microsoft/winget-pkgs#460744`（3.5.1）是否已 MERGED | **2026-10-27 实测：仍 OPEN + REVIEW_REQUIRED**（未合并）。同包 ID 不允许同时开两个 PR ⇒ **3.6.3 只备清单、不提 PR** |
| 2 | 安装包**代码签名**（需用户自备证书） | 早前提出，仍未决；本轮安装包 `NotSigned` |
| 3 | 是否要催 winget 版主审 `#460744` | 早前提出，仍未决 |
| 4 | 远程条目的 `copy`/`cut`/`paste` 是否放开 | 产品决定，见 `src/utils/entry-action-policy.ts:70` 的 `REMOTE_PHASE0_BLOCKED_ACTIONS` |
| 5 | 3.6.3 是否也推 winget 清单 | **已定：只备清单**。四件套已生成（`docs/winget/manifests/k/kizemo/AlphaFileManager/3.6.3/`），`npm run check:winget` 全绿（含 "InstallerSha256 matches the local installer (20716092 bytes)"），等 `#460744` 合并后再提 PR |

---

## 11. 一句话接力 prompt

> 接手 `F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager，Tauri 2 + Vue3 + reka-ui）：先读 `F:\soft\00selfmade\filemanager\AGENTS.md`（项目硬约束，**必读**）与 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-27-delete-enter-key-and-list-view-grouping.md`（本文，**唯一权威状态源**：第 0 节速查表、第 13 节优化 C 的规格与**最终验收结论**、第 14 节发版资产现状、第 17.4 节**发布结果**、第 16–17 节经验）。
>
> **3.6.3 已于 2026-10-28 全部发完**（commit `a9fed27a` → `origin/main`；GitHub release `v3.6.3` 与站点 aiec.fun 线上均为 3.6.3，sha256 `FEE1F2A874FA8F89A32F71AA03B6B934C4A8A2E6DB4F23F95D7FF03B1E44644B`，20,716,092 B）⇒ **不要重复发版、不要重跑已完成的验收**。
>
> 下一轮请先跟用户确认做哪一项（候选：① 等 `microsoft/winget-pkgs#460744`（3.5.1）合并后补提 3.6.3 的 winget PR；② 列表视图分区的 **sticky 段头**；③ 远程条目（`renameRemoteEntry`）的就地改名真机验证；④ 虚拟滚动下编辑态行被滚出的边界；⑤ 下一个补丁版本 3.6.4），再按本文第 13–17 节既定的口径动手。发版权威流程见 `docs\operations\release-workflow.md`，全程遵守 `AGENTS.md` 的中文输出与各项硬约束。

## 12. 涉及文件完整路径清单

### 12.1 优化 A

```
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\permanent-delete-confirm-dialog.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\utils\permanent-delete-confirm-keyboard.ts        ← 新建
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\utils\__tests__\permanent-delete-confirm-keyboard.test.ts   ← 新建
F:\soft\00selfmade\filemanager\sigma-file-manager\src\components\ui\dialog\index.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\components\ui\dialog\dialog-content.vue
```

### 12.2 优化 B

```
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-entry-groups.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-list-view.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-grid-view.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-grid-section-bar.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-content-body.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\utils\file-browser-virtual-rows.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-virtual-layout.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-keyboard-navigation.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-selection.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-context.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\__tests__\use-file-browser-virtual-layout.test.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\__tests__\file-browser-entry-groups.test.ts
```

### 12.3 发版与配置

```
F:\soft\00selfmade\filemanager\AGENTS.md                                                                 ← 项目硬约束（必读）
F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md
F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\version-info.md
F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md
F:\soft\00selfmade\filemanager\sigma-file-manager\README.md
F:\soft\00selfmade\filemanager\sigma-file-manager\README.zh-CN.md
F:\soft\00selfmade\filemanager\sigma-file-manager\_notes.md
F:\soft\00selfmade\filemanager\sigma-file-manager\public\changelog\<15 个 locale>\changelog.md
F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\release-publish.mjs
F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\verify-changelog.mjs
F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\publish-bundle-to-release.mjs
F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\sync-version.js
F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\check-manifest.mjs
F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\README.md
F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\manifests\k\kizemo\AlphaFileManager\3.6.2\   ← 3.6.3 的模板
F:\soft\00selfmade\filemanager\sigma-file-manager\src\localization\messages\*.json                            ← 17 个语言包
E:\办公文件\L1网站\alpha-file-manager\version.json
E:\办公文件\L1网站\_upload_alpha_files.py
E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe
E:\办公文件\L1网站\.vscode\sftp.json                                                                       ← 凭据，绝不外泄
```

### 12.4 本轮产物与工具

```
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\task-list-view-grouping.md   ← B 的 worker 任务书
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\handoff-363-tail.md          ← 本次追加到本文的第 12.5–16 节片段（gitignore 内）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\push-with-fallback.cjs
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\superseded\                   ← 被挪开的构建产物
E:\Users\Duanyi\.minimax\.minimax\maintenance\tool_budget_hook.py                              ← 本轮的拦截器
E:\Users\Duanyi\.minimax\.minimax\maintenance\task-state\LATEST.md                               ← 自动交接卡
```

### 12.4.1 锁屏「无输入注入」验收脚本（2026-10-28 新增）

```text
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\probe-desktop.ps1                     ← 锁屏性质判定（ASCII）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename-v4.ps1      ← 无输入注入首版
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename-v5.ps1      ← 右键路径全链路 PASS
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename-v6.ps1      ← F2 / 回车 / Esc
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename-v7.ps1      ← 链式改名（干净夹具）13/0
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename-v8.ps1      ← 缩略图视图 6/0
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename-v5.log      ← 以上四支的日志
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\shots\v5-*.png , v6-*.png , v8-*.png    ← 截图
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-backup\user-settings.json.pregrid.bak ← v8 前后的设置备份
```

### 12.5 优化 C（就地重命名）涉及文件完整路径

**新增（3 个，未提交）**

```
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\utils\file-browser-inline-rename.ts                      ← 新建（纯逻辑）
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\utils\__tests__\file-browser-inline-rename.test.ts      ← 新建（16 用例）
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-inline-rename-input.vue                ← 新建（输入组件）
```

**修改（10 个，未提交）**

```
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-selection.ts          ← 改名状态机核心（+214 行）
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-dialogs.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-context.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-focus.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser-keyboard-navigation.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\use-file-browser.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\composables\__tests__\use-file-browser-dialogs.test.ts
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-list-view.vue
F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\components\file-browser\file-browser-grid-card.vue
```

**优化 C 的工具脚本（`.superpowers\sdd\`，gitignore 内）**

```
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\spec-inline-rename.md            ← 规格
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\inline-rename-implementation.md  ← 实现说明
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-363-inline-rename.ps1      ← 真机验收脚本（第 1 轮，**已被 v5–v8 取代**）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\patch-363-inline-docs.cjs          ← 18 个发版文档批量补写（已跑成功，幂等）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\release-363-inline.cmd            ← 第三轮打包（--bump none）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\run-gates-363.ps1                  ← 门禁（结果写 gates-363.log）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\gen-winget-363.cjs                ← winget 3.6.3 清单生成
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\append-handoff-results.cjs        ← 追加第 13/14 节（**未运行**，本文已手工补齐，可不用）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\msg-363-release.txt               ← 提交信息（**待补优化 C 条目 + 新 sha/字节数**）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\pin-workspace.cjs                ← 钉/还原 user-workspaces.json
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\app-key.ps1                      ← 裸 SendInput 注入（含 40 字节 INPUT）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\app-sendkeys.ps1                 ← WScript.SendKeys 注入
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\afm-accept.ps1                   ← 找主窗口 + 截图
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\click-webview-point.ps1          ← webview 内点击
```

---

## 13. 优化 C（用户中途追加的第 3 项需求）：F2 / 右键「重命名」就地编辑

### 13.1 用户原话

> 增加一个优化要求：使用快捷键 F2 或右键选择文件重命名（非批量重命名），不要打开弹窗，直接在目录窗口（列表或缩略图视图）的文件名上进行修改，且修改后，可以通过键盘上下切换文件，切换后，保留可以编辑文件名的状态，便于手动批量修改文件名。

规格落盘：`.superpowers\sdd\spec-inline-rename.md`；实现说明：`.superpowers\sdd\inline-rename-implementation.md`。
交付形态：13 个文件，**336 insertions / 13 deletions**。

### 13.2 分流规则（唯一入口在 `use-file-browser-selection.ts` 的 `startRename()`）

| 入口 | 链路 | 结果 |
|---|---|---|
| F2 | `src/stores/runtime/shortcuts.ts:386`（`id:'rename'`, `key:'F2'`, scope navigator, conditions `inputFieldIsActive:false / dialogIsOpened:false / dirItemIsSelected:true`）→ `src/modules/navigator/pages/navigator.vue:1576` 的 `registerHandler('rename', …)` → `pane.startRename(activePaneSelection[0])` | list/grid ⇒ 就地编辑；tree ⇒ 模态框 |
| 右键「重命名」 | `file-browser-actions-menu.vue` → `file-browser-context-menu.vue` `handleAction` → `ctx.onContextMenuAction('rename')` → `use-file-browser-selection.ts` 的 `handleContextMenuAction` `case 'rename'` → `startRename(entries[0])`（**只处理单选**） | 同上 |

- `renameState` 增加 `mode: 'dialog' | 'inline'` 与 `isChainActive`。
- `canEditInline()` = 当前 layout 是 `list` 或 `grid`；**tree / 批量重命名 / 首页仪表盘侧栏仍走模态框**。
- `isRenameDialogOpen`（`use-file-browser-dialogs.ts`）= `isActive && mode === 'dialog'` ⇒ 就地编辑时**不会**同时冒出模态框。
- **未新增后端命令 / i18n key / 依赖**；`file-browser-rename-dialog.vue` 一行未动。

### 13.3 两条「连续改名」路径（最容易被误读，务必分清）

**路径 ①（批量改名主路径）：输入框内按 ↑/↓**

`file-browser-inline-rename-input.vue` 的 `handleKeydown` → `getInlineRenameMoveIntent(event.key)` → `ctx.inlineRename.move(direction, draft)`
→ `use-file-browser.ts:805` 的 `move`：**先** `selection.submitInlineRename(currentName, 'move')`（提交当前名字），成功后再 `keyboardNav.navigateDown()` / `navigateUp()`
→ 落点回调 `onNavigationTargetApplied`（`use-file-browser-keyboard-navigation.ts:97`）→ `handleInlineRenameNavigationTarget` 把落点条目切进编辑态
⇒ **改一个 → 按 ↓ → 下一个直接可编辑**。

**路径 ②（提交/取消之后用 ↑/↓ 切条目）**

`applyNavigationTarget()` → `options.onNavigationTargetApplied?.(entry)` → 同一个 `handleInlineRenameNavigationTarget`。
⇒ 依赖「落点条目被选中」时触发的导航回调；**这条路径要求 DOM 焦点仍在列表行上**（方向键是行级 `@keydown`，见 `file-browser-list-view.vue:598` / `file-browser-grid-card.vue:365`）。

> ⚠️ **已知风险（必须由真机验收判定，尚未实测）**
> `submitInlineRename` 成功路径调用 `requestFocusEntryAfterRefresh(parentDir, newPath, { shouldFocus: false })`（**刻意不抢焦点**——路径 ① 下焦点要交给下一个输入框）。
> 若输入框卸载后 DOM 焦点掉到 `body`，则**路径 ② 收不到方向键**。
> **若真机验收在此步失败，修法是让「enter / blur 提交」路径恢复焦点到被改名的行**（`shouldFocus: true`），**不要**删掉 chain 逻辑、也不要改成把 `shouldFocus: false` 一刀切。

**状态机**（`utils/file-browser-inline-rename.ts` 的 `getInlineRenameChainStateAfterCommit` / `…AfterCancel`）

| 触发 | 结果 | 编辑态 | 连续模式 |
|---|---|---|---|
| 成功（renamed / unchanged） | `enter` / `move` | 退出 | **保持开** |
| 成功 | `blur` | 退出 | 结束 |
| 失败 | `enter` / `move` | **留在编辑态** | 结束 |
| 失败 | `blur` | 退出 | 结束 |
| Esc | — | 退出 | 结束 |

其它已确认事实：

- 连续模式在「**鼠标点条目**」时结束（`onEntryMouseDown` 包了 `endInlineRenameChain()`）；键盘导航走 `selectEntryByPath`，不会自己打断自己 ⇒ **验收时 commit 与 ↓ 之间不能插入鼠标点击**。
- 非法字符正则 `/[<>:"/\\|?*\x00-\x1f]/`、`lastIndexOf('.') > 0` 才算扩展名 —— 与 `file-browser-rename-dialog.vue` 语义一致（后端判定未动）。
- 草稿放在**输入组件本地 ref**（列表行有 `v-memo` 缓存，父级受控值会被吞）；`memoKey` 必须带编辑态标记。
- inline 成功后**不清选区**（清了就找不到当前条目，↑/↓ 落点会跳回首项），改用 `requestFocusEntryAfterRefresh(parent, newPath, { shouldFocus: false })` 按新路径重新选中。
- 网格卡片里 `<input>` 嵌在 `<button>` 内（HTML 语义无效嵌套）—— **HTML 语义确实无效，但实测可用**：缩略图视图的就地编辑已在真机验收通过（第 13.6 节 v8）。

### 13.4 真机验收第 1 轮失败的根因（**是验收脚本的问题，不是产品缺陷**）

脚本 `.superpowers\sdd\accept-363-inline-rename.ps1`，日志 `.superpowers\sdd\accept-363-inline-rename.log`，截图 `.superpowers\sdd\shots\inline-363-0{1,2,3}-*.png`。
8 条断言失败，三个真实原因：

1. **按键路径错**：脚本用 `app-sendkeys.ps1`（WScript.SendKeys）发 F2，未验证能送达。已验证可用的是 `app-key.ps1` 的裸名 SendInput 路径。
   ⚠️ `app-key.ps1` **只认裸名 token**（`F1`–`F12`/`ENTER`/`ESC`/`TAB`/`DELETE`/`UP`/`DOWN`/`LEFT`/`RIGHT`/`PAGEUP`/`PAGEDOWN`/`HOME`/`END`/`INSERT`/`A`–`Z`，可加 `CTRL+`/`SHIFT+`/`ALT+` 前缀）。
   传 `'{F2}'`（带花括号）会落到 UNKNOWN TOKEN 分支**静默不发送**。
   `app-sendkeys.ps1` 才是 SendKeys 语法（`'{DOWN}'` / `'+{DOWN}'`）。
2. **阳性对照判据错**：用了 `AutomationElement.FocusedElement`，它在 WebView2 里报的是应用自定义标题栏按钮（「最小化」），**不能**当「条目已聚焦」的判据。
   但**截图证明点击其实成功了**（首行已高亮选中、右侧详情面板显示 `aaa-first.txt` / 32 B / txt/plain / 完整路径）。
   ⇒ 改用**状态栏**判据：`已选择 {count} 个项目`（i18n key `fileBrowser.selectedItems`，`src/localization/messages/ch.json:1138`）。
3. **失败时没有诊断输出** ⇒ 已补 DOM 焦点 / 状态栏原文 / 前 40 个 Text-Edit-Button 元素的 `[CT] "name" @x,y w×h`。

> 这三条都是**验收脚本**的问题。但结论不能靠推理代替实测 —— 必须重跑证实。

### 13.5 锁屏挡住了注入：改走「无输入注入」路径（`v4`–`v8`）

2026-10-28 00:03 起本机会话被锁（`LogonUI` 在跑、`GetForegroundWindow()=0`、`OpenInputDesktop` err=5、`SendInput` n=0/err=5、`SetCursorPos` 失败）⇒
**`app-key.ps1` / `app-sendkeys.ps1` 这一整套 SendInput 路径全部不可用**。

实测结论（`.superpowers\sdd\probe-desktop.ps1`）：

| 手段 | 锁屏下 | 结论 |
|---|---|---|
| `SendInput`（真实键鼠） | n=0 / err=5 | ❌ 不可用 |
| `SetCursorPos` / 真实鼠标点击 | 失败 | ❌ 不可用 |
| `PostMessage` 合成鼠标（`WM_MOUSEMOVE` + `WM_LBUTTONDOWN/UP`、`WM_RBUTTONDOWN/UP`） | **有效** | ✅ 能触发选中、能开右键菜单 |
| `PostMessage` `WM_KEYDOWN` / `WM_KEYUP`（VK 码） | **有效** | ✅ 能触发 F2 / 回车 / ↑↓ / Esc |

⇒ **键盘链路不必等解锁**：把 `WM_KEYDOWN(0x0100)` 直接发给 `Chrome_RenderWidgetHostHWND`（主窗口的 render widget），
渲染进程就会派发正常的 `keydown`，走的是产品代码里同一条路径。
（`WM_CHAR` 依旧会被静默丢弃 —— 它要靠 `TranslateMessage` 配合真实按键才会生成。）

五支新脚本（全部纯 ASCII、必须用 **Windows PowerShell 5.1** 跑）：

| 脚本 | 覆盖 | 结果 |
|---|---|---|
| `.superpowers\sdd\accept-363-inline-rename-v4.ps1` | 右键菜单 → 就地编辑器（首版，触发点未打通） | 定位到根因 |
| `.superpowers\sdd\accept-363-inline-rename-v5.ps1` | 右键路径全链路（定位菜单弹窗 HWND + `InvokePattern` + `WM_KEYDOWN` 提交） | **PASS（落盘改名成功）** |
| `.superpowers\sdd\accept-363-inline-rename-v6.ps1` | F2 / 回车 / Esc | 14 项断言 13 PASS |
| `.superpowers\sdd\accept-363-inline-rename-v7.ps1` | 链式改名（干净夹具） | **13 PASS / 0 FAIL** |
| `.superpowers\sdd\accept-363-inline-rename-v8.ps1` | 缩略图（grid）视图 | **6 PASS / 0 FAIL** |

### 13.6 最终验收结论（**全部 PASS**，打包产物 `D:\Program Files\Alpha File Manager\alpha-file-manager.exe` 3.6.3）

| 场景 | 断言 | 结果 |
|---|---|---|
| 右键菜单改名 | quick-actions 第 1 个无名按钮（铅笔）→ 出现 `[Edit]`、**没有**「重命名项目」对话框 → 回车后文件系统上是新名字 | ✅ |
| F2 | 打开的是文件名上的输入框、不是对话框；输入框内容 = 当前文件名 | ✅ |
| 回车 | 新名字落盘、旧名字消失、编辑器关闭 | ✅ |
| ↓ | 编辑器**仍在**，装的是下一个条目的名字；上一步的新名字**已落盘** | ✅ |
| ↑ | 编辑器仍在，装的是上一个条目的名字 | ✅ |
| Esc | 编辑器关闭，该条目**没有**被改名 | ✅ |
| 缩略图视图 | 与列表视图一致：F2 → 输入框 → 回车落盘 | ✅ |

> v6 里有 1 条 FAIL（「↓ 之后应是 `bbb-second.txt`」）是**夹具污染**造成的假阴性 ——
> 上一轮测试已经把 `aaa-first.txt` 改名，目录里残留 4 个文件，↓ 的相邻条目自然不是它。
> v7 用**干净夹具**（只有 3 个文件）重跑，13 项断言全绿。

---

## 14. 实测结果与发版资产现状（3.6.3）

### 14.1 门禁（`.superpowers\sdd\run-gates-363.ps1`；结果只写 `gates-363.log`，stdout 为空）

| 项 | 结果 |
|---|---|
| `npm run test` | **248 files / 2275 tests 全过**（TEST_EXIT=0） |
| `npm run ts`（vue-tsc） | **TSC_EXIT=0**（类型检查只有它能抓，`npm run test` 不做类型检查） |
| `eslint .` | **188 errors / 0 warnings / 1049 files** = 基线（ESLINT_EXIT=1 是 eslint「有错即 1」的语义，**不是回归**） |

### 14.2 安装包（第三轮打包，`.superpowers\sdd\release-363-inline.cmd`，`--bump none`）

| 项 | 值 |
|---|---|
| 路径 | `F:\soft\00selfmade\filemanager\sigma-file-manager\release\Alpha File Manager_3.6.3_x64-setup.exe` |
| 大小 | **20,716,092 B** |
| sha256 | **`FEE1F2A874FA8F89A32F71AA03B6B934C4A8A2E6DB4F23F95D7FF03B1E44644B`** |
| 静默安装 | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，ProductVersion = **3.6.3** |
| 作废的前两轮 | D258E57A…2B22 / 20,670,028 B |

### 14.3 winget 3.6.3 清单

- 目录：`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\manifests\k\izemo\AlphaFileManager\3.6.3\`（4 个文件，由 `.superpowers\sdd\gen-winget-363.cjs 3.6.3 2026-10-27` 从 3.6.2 克隆并替换真实 sha 生成）。
- `npm run check:winget` → **All local checks passed**（含 "InstallerSha256 matches the local installer (20716092 bytes)"）。
- `docs/winget\README.md` 的 3.6.3 章节已更新（sha / 字节数 / 「三处改动」表述）。
- **只备清单，不提 PR**（见第 10 节第 1 条）。

### 14.4 站点（aiec.fun 源目录 + **线上已生效**）

`E:\办公文件\L1网站\alpha-file-manager\` 已含：
- `Alpha_File_Manager_3.6.3_x64-setup.exe`（20,716,092 B）
- `version.json`（version = 3.6.3，sha 前缀 `fee1f2a874fa8f89`，history = 18，**未重复归档**）
- `_release-notes-v3.6.3.md`（1,404 B，**已含「就地重命名」**）

### 14.5 发版文档（18 个文件已改，**已随 `a9fed27a` 提交**）

- `_notes.md`：首行加「；重命名改成在文件名上就地编辑」+ 新增「**就地重命名：**」段落。
- 根 `CHANGELOG.md` + 14 个 `public/changelog/<loc>/changelog.md` 的 3.6.3 段：Summary 改为「Three things / 三件」并追加 `**Added: renaming happens in place.**` / `**新增：重命名就地编辑。**`。
- `README.md` 追加 `### ✏️ Renaming happens in place`；`README.zh-CN.md` 追加 `### ✏️ 重命名就地编辑`。
- 脚本：`.superpowers\sdd\patch-363-inline-docs.cjs`（幂等 + 换行感知：先归一为 LF 再按文件原 EOL 写回，校验无 U+FFFD / 无 BOM）。
- **语言包惯例**：除 `ch`（中文正文）外，其余 13 个语言包正文一直是英文（仅 H1 标题本地化），3.6.2 亦如此 ⇒ 本轮沿用该惯例。历史版本存在翻译漂移，属既有状态，不在本次范围。

### 14.6 用户数据：**已还原到验收前快照**（2026-10-28）

| 项 | 值 |
|---|---|
| `user-workspaces.json` | **已还原**：2,350 B，sha256 `1F586B6AF91A7812E55A10B1054F5BE352B7C5D024D06BA49E21CA49E7C2557C`（= 验收前快照） |
| 还原后的工作区 | 7 个真实目录（`E:/办公文件/000临时事项`、`E:/办公文件/G1销售管理/00数据汇总分析/售后报表`、`G:/music`、`F:/` 等），**无夹具路径** |
| `user-settings.json` | 4,188 B；`navigator.layout.type` = `{"name":"list","title":"listLayout"}`（v8 试缩略图时临时改成 grid，测完已改回） |
| `navigator.folderSettings` | 无任何 `afm-` / `lab` / `fixture` 残留条目 |
| 夹具目录 | `C:\Users\Duanyi\afm-inline-lab` **已用 `mavis-trash.cmd` 移入回收站**（`Test-Path` = False） |
| 遗留进程 | `alpha-file-manager` 进程数 **0** |

> **还原顺序铁律不变**：**先杀 `alpha-file-manager` 进程 → 再 `node .superpowers\sdd\pin-workspace.cjs restore`**。
> 应用退出时会把内存里的统计 / 工作区状态**写回**真实文件 ⇒ 夹具或 pin 状态会被再写回去一次。

---

## 15. 剩余步骤（**已全部完成**，本节留作发版流程记录）

| # | 动作 | 命令 / 要点 | 状态 |
|---|---|---|---|
| 1 | 读约定 + 本文 | `F:\soft\00selfmade\filemanager\AGENTS.md` + 本文（第 0 节速查表先看） | ✅ |
| 2 | 优化 C 真机验收 | `accept-363-inline-rename-v5/v6/v7/v8.ps1`（**锁屏下用 `PostMessage` + `WM_KEYDOWN`**，见第 13.5 节） | ✅ 全部 PASS |
| 3 | 还原工作区 + 清夹具 | 先杀进程 → `node .superpowers\sdd\pin-workspace.cjs restore` → 核对 2,350 B / sha `1F586B6A…557C` → `mavis-trash.cmd -- "$env:USERPROFILE\afm-inline-lab"` | ✅ 已完成 |
| 4 | 补提交信息 | `.superpowers\sdd\msg-363-release.txt` —— 已写入优化 A / B / C 三条 + 实测验收结论 + sha `FEE1F2A874FA8F89A32F71AA03B6B934C4A8A2E6DB4F23F95D7FF03B1E44644B` + 字节数 20,716,092 | ✅ |
| 5 | 发版提交 | `git -C F:\soft\00selfmade\filemanager\sigma-file-manager add -A` → `git -C … commit -F F:\…\.superpowers\sdd\msg-363-release.txt`（**信息必须走文件**） | ✅ `a9fed27a`（42 files / +1600 / −102） |
| 6 | push | `git -C … -c http.proxy= push origin main`；核对远端 SHA 用 `gh api repos/kizemo/alpha-file-manager/commits/main --jq '.sha'`（**不要用 `git ls-remote`**） | ✅ 远端 = `a9fed27a98708c8465b2ecc96f67f4c0a7c67eb5` |
| 7 | 双端上传 | `npm --prefix <repo> run release:upload`（aiec.fun）→ `npm --prefix <repo> run release:github`（GitHub release `v3.6.3`，资产名 `Alpha_File_Manager_3.6.3_x64-setup.exe`）→ `release:status` → `release:verify` | ✅ 站点 + GitHub 均已上线，版本 / 大小 / sha256 三项一致 |
| 8 | 汇报 | 简体中文；**必须写明**：① **sticky 段头未做**（列表视图分区是随内容滚动的内联标题）；② winget PR 仍待 `#460744`（3.5.1）合并，3.6.3 只备清单未提 PR；③ 安装包**无代码签名**；④ workspace 已回滚到验收前快照；⑤ 缩略图视图的就地编辑**已**做真机验证 | ✅ 已汇报 |

### 15.1 发版顺序铁律（再强调一次）

1. **`verify-changelog` 必须在升版本号之前跑**（它校验「当前版本有 changelog 条目」）。
2. **`release:prepare --write-notes` 必须在 `release:prepare`（打包）之后跑**（它要读产物大小与 sha256）。
3. `release:prepare` **内部已含 `tauri build`** ⇒ **发版只构建一次**，不要再单独跑 `npm run tauri:build`。
4. 版本号**已经是 3.6.3**（package.json + Cargo 三处）⇒ 万一要重打包，**必须 `--bump none`**，否则版本号会被顶掉（3.6.0 → 3.7.0 的事故就是这么来的）。
5. `publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 下所有历史安装包逐个 `rmSync` + 复制到 `release/`；**只要 `release/` 里任何一个旧包被锁住就整步 exit 1**（而 `tauri build` 早已成功）⇒ 分清「构建失败」和「复制失败」，新包单独 `fs.copyFileSync` 补进 `release/` 即可。

---

## 16. 本轮新增的经验（下一轮直接用，别再踩）

### 16.1 脚本与编码

- **`.ps1` / `.cmd` 必须纯 ASCII**（PS 5.1 按 ANSI 读无 BOM 文件）。中文 needle 一律用 `[char]0x....` 拼；本轮已在 `.ps1` 里写进中文直字，**是本轮自己踩的坑**。
- **`.cmd` 的 `echo` 里不能出现 `>`**（会被当重定向符，本项目已在仓库根目录留下过 `3.6.0)` 垃圾文件）；`.cmd` 调用另一个 `.cmd` 要加 `call`；**`.cmd` 运行期间绝对不能编辑**（cmd.exe 按字节偏移逐行读 ⇒ 会执行错位的片段，3.6.2 那轮因此把版本顶到 3.7.0）；要改就**新建文件名**重跑。
- **绝不用 PowerShell 的 `Get-Content -Raw` / `Set-Content` 改含中文的源文件**；用编辑工具或 node `fs.readFileSync/writeFileSync(..., 'utf8')`，写完立刻验 `U+FFFD === 0` 且无 BOM。

### 16.2 注入与 UIA 取证

- **裸键（F1–F12 / ENTER / ESC / 方向键）一律走 `app-key.ps1`**（裸名 token + 40 字节 `INPUT` + 前台激活）；**SendKeys 语法（`'{DOWN}'` / `'+{DOWN}'`）才走 `app-sendkeys.ps1`**。
  ⚠️ `app-key.ps1` 传带花括号的 token 会**静默不发送**（UNKNOWN TOKEN 分支）。
  ⚠️ **单修饰键 + 非字母主键**（`Shift+Delete`）的 Shift 到不了 WebView2 ⇒ 永久删除验收走**右键菜单顶部图标条的垃圾桶**。
- **注入脚本的输出不要 `| Out-Null`** —— `foreground MATCH=` / `down F2 -> 1` 是唯一能分辨「注入没进去」和「功能真坏了」的证据。
- **注入探针必须「阳性对照排在前面」**：先做一个必然有可见后果的动作，确认状态变了，再验目标。
- **WebView2 里 `AutomationElement.FocusedElement` 不可信**（会报应用自定义标题栏按钮）⇒ 判「选中」看**右侧详情面板**或**状态栏「已选择 N 个项目」**。
- **UIA `BoundingRectangle` 是物理像素**：本机屏 3840×2160 / DPI 150%（虚拟 2560×1440），PrintWindow 截图 3862×2110；CSS 逻辑像素要 ÷1.5。
- **列表视图行的可访问名是复合串**（「文件名 — 大小 日期」）⇒ 用前缀 `-like ($Name + ' *')`，精确 `-eq` 必然落空；取**面积最大**的候选（同名有两个候选：侧栏 107×26 的 `ControlType.Text` 与列表行 2512×96 的 `ControlType.Button`；缩略图视图则是 479×129 的卡片按钮；点行中心**能选中**）。
- **`InvokePattern.Invoke()` 返回成功 ≠ 点击生效**（禁用按钮是静默空操作）。
- **应用启动会弹「新功能」更新日志模态框**，把整个文件列表挡住 ⇒ 验收脚本必须先关掉它（关闭按钮名「关闭」= U+5173 U+95ED），否则「找不到卡片」会被误判成页面没加载。
- **应用日志被独占锁着** ⇒ 用 `File.Open($p,'Open','Read','FileShare.ReadWrite')` + `StreamReader` 读；日志时间戳是 **UTC**（比本地少 8 小时）。

- **会话锁住时不要死磕 SendInput**：本项目实测 `SendInput` n=0/err=5、`SetCursorPos` 失败，但 **`PostMessage` 合成鼠标消息与 `WM_KEYDOWN` 能穿透到 WebView2 渲染进程** ⇒ 键盘链路（F2 / 回车 / ↑↓ / Esc）可以在锁屏状态下真机验收。`WM_CHAR` 依旧被丢弃。
- **文件浏览器右键菜单里没有文字「重命名」项**：12 个 `MenuItem` 全是「打开方式… / Shell 扩展 / 在终端中打开 Alt+T / 压缩 / 快速查看 Space / 打印 Ctrl+O / 复制路径到剪贴板 / 解除文件锁定 / 彻底删除 / 局域网共享 / 属性 Alt+Enter / 添加到收藏」；改名入口是 quick-actions 行**第 1 个无名按钮（铅笔）**（文件浏览器场景插槽为空 ⇒ 铅笔排第一）。**按名找「重命名」永远找不到**。
- **无名按钮用 UIA `InvokePattern` 最稳**：`PostMessage` 点它的屏幕坐标在本项目里不稳定（菜单会先被关掉、元素 `BoundingRectangle` 变 `NaN`、int cast 会保留旧值 ⇒ 反复点同一点）。**每次尝试前重新打开菜单**。
- **UIA `ValuePattern.SetValue` 在 WebView2 里会触发 `input` 事件** ⇒ 组件本地 `draft` 会更新，回车提交的就是新名字（实测落盘正确）；读回值等于期望值。
- **应用退出时会把内存状态写回真实文件** ⇒ 验收后必须**先杀进程再 restore**，否则夹具会被再写回去一次（本项目踩过，代价是真实数据被夹具顶掉一次）。

### 16.3 构建与门禁

- **后台长编译**：`.cmd` 内部用 `> logfile 2>&1` 重定向到文件，**不要挂 stdout 管道**（`Tee-Object` / `| Select-String`）—— 管道一断进程树跟着死，2026-10-23 因此白跑十分钟。
- **cargo 不在 PATH**：`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`；MSVC 环境靠 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`。
- **`cargo test --lib` 的隐形红线**：任何被测试链接到的代码都不能碰 `tauri::AppHandle` 或 `tauri::async_runtime::*` ⇒ 业务模块要提供 `*_blocking` 同步内核入口，调用方自己选执行器。
- **`npm run test` 不做类型检查**（esbuild 转译）⇒ 类型错误只有 `npm run ts`（vue-tsc）能抓。
- **`vitest --reporter=basic` 在本仓库不存在**，会 `Failed to load custom Reporter from basic` 直接中止 ⇒ 跑全量用默认 reporter，按 `Test Files` / `Tests` 两行取结果。
- **门禁基线必须用同一条命令量**：`eslint .` = 188 / `eslint src` = 175；`eslint .` 的数字要**干净检出**那个数（`eslint` 不读 `.gitignore`，工作副本里的 `.superpowers/` 会被一起 lint）。
- **`run-gates-363.ps1` 的结果只写 `gates-363.log`**（stdout 为空），日志是混合编码（`Out-File utf8` + `*>>` utf16le）⇒ 用 node 分别按 utf16le / utf8 解码后 grep；eslint 明细在 `eslint-all.json`。
- **`$ErrorActionPreference = 'Stop'` 会把原生命令的 stderr 变成终止错误** ⇒ 门禁脚本调 `npm` / `npx` 前后要显式切回 `Continue`。

### 16.4 本仓特有的两个「以为坏了其实是环境问题」

- **`SFM_E2E_DATA_DIR` 只在 debug 构建生效**（`src-tauri/src/app_dirs.rs` 的覆盖是 `#[cfg(debug_assertions)]`）⇒ release 产物上做真机验收且不想动用户真实数据，只能「**备份真实文件 → 灌夹具 → 验收 → 恢复**」。
- **验收夹具不要放 `%TEMP%`**（`global_search/ignore.rs` 的 `builtin_ignored_paths()` 含 `/AppData/Local/Temp`，且 `matches_plain_ignored_path` 对 `/` 开头的模式做 `path.contains(...)`）⇒ 整棵子树被剪掉，命中恒为 0。本轮夹具放在 `C:\Users\Duanyi\afm-inline-lab\`。
- **`PrintWindow` 会返回陈旧帧**（渲染线程刷爆时）⇒ 判后台任务的运行状态要看**文件系统**（文件数是否停止增长），不要看屏幕。

---

## 17. 本轮（2026-10-28）新增的经验与最终状态

### 17.1 锁屏下的「无输入注入」验收配方（可复制）

1. **杀旧实例 → 建干净夹具 → 起应用**（用 `tauri build` 装出来的 exe，不是 `tauri dev`）。
2. **找 HWND**：`EnumWindows` + `GetWindowThreadProcessId` 过滤 PID → class 必须等于 `Tauri Window` → 取**面积最大**的可见窗口（`Process.MainWindowHandle` 会落到 15×15 的托盘窗上）。
3. **找 render widget**：该窗口的 `Chrome_RenderWidgetHostHWND` 子窗口；本机 render widget 的客户区原点恰好 = 屏幕原点（rect `0,0 3840x2088`），但仍要用 `ScreenToClient` 换算。
4. **合成鼠标**：`WM_MOUSEMOVE` → `WM_LBUTTONDOWN(0x0201, wParam=1)` → `WM_LBUTTONUP(0x0202)`；右键换 `0x0204/0x0205` + `wParam=2`。`WM_CONTEXTMENU(0x007D)` 无效。
5. **合成键盘**：`WM_KEYDOWN(0x0100, wParam=VK)` + `WM_KEYUP(0x0101)`。VK：F2=`0x71`、回车=`0x0D`、↑=`0x26`、↓=`0x28`、Esc=`0x1B`。
6. **阳性对照**：左键点行 → 状态栏出现「已选择 1 个项目」才算注入到位。
7. **UIA 取值**：`ValuePattern.SetValue` 写新名 → 读回校验 → `WM_KEYDOWN(回车)` 提交 → 落盘核对文件名。

### 17.2 产品侧结论

- 优化 C **在打包产物上全部走通**：右键路径、F2、回车、↑↓ 链式改名、Esc、缩略图视图。
- 「连续改名」的语义实测确认：`↓` 会**先提交当前条目**再把编辑目标移到下一个条目，所以可以一个接一个地改。
- 网格卡片里 `<input>` 嵌在 `<button>` 内虽然 HTML 语义无效，但 Chromium 能正常渲染与编辑 ⇒ **不构成缺陷**。

### 17.3 仍未做 / 待后续

- **sticky 段头未做**（列表视图分区标题随内容滚动，不是吸顶）。
- **winget PR 未提**：`microsoft/winget-pkgs#460744`（3.5.1）仍 OPEN + REVIEW_REQUIRED，同包 ID 不能同时开第二个 PR ⇒ 3.6.3 只备清单。
- **安装包无代码签名**（`NotSigned`），需用户自备证书。
- 远程条目（`renameRemoteEntry`）的就地改名**未做真机验证**（本轮夹具全是本地文件）。
- 虚拟滚动下**编辑态行被滚出可视区会被回收**，输入框消失但 `renameState` 仍留着 entry —— 已知边界，本轮未处理。

### 17.4 3.6.3 发布结果（2026-10-28）

| 环节 | 结果 |
|---|---|
| 发版提交 | `a9fed27a` —— `release: 发版 3.6.3 —— 永久删除确认框回车即生效；列表视图按类型分区；重命名就地编辑`（42 files / +1600 / −102） |
| push | `origin/main` = `a9fed27a98708c8465b2ecc96f67f4c0a7c67eb5`（`gh api repos/kizemo/alpha-file-manager/commits/main --jq '.sha'` 核对一致） |
| GitHub release | `v3.6.3` 已创建：https://github.com/kizemo/alpha-file-manager/releases/tag/v3.6.3 ；资产 `Alpha_File_Manager_3.6.3_x64-setup.exe`，20,716,092 B，sha256 `FEE1F2A874FA8F89A32F71AA03B6B934C4A8A2E6DB4F23F95D7FF03B1E44644B` |
| 站点 aiec.fun | SFTP 上传 + cache-busting 回读通过：线上版本 3.6.3，源站包 20,716,092 B，边缘包 20,716,092 B（`CF-Cache-Status=HIT`），sha256 一致 |
| `release:status` | exit 0 —— package.json 3.6.3 / 安装包 20,716,092 B / 站点源 version.json 3.6.3，sha 三处相同 |
| `release:verify` | exit 0 —— **版本号 / 大小 / sha256 三项均一致（源站与边缘都已生效）** |
| winget | 3.6.3 清单四件套已备（`docs/winget/manifests/k/kizemo/AlphaFileManager/3.6.3/`），`npm run check:winget` 全绿；**未提 PR**（`microsoft/winget-pkgs#460744`（3.5.1）仍 OPEN + REVIEW_REQUIRED，同包 ID 不能同时开第二个 PR） |
| 日志 | `.superpowers\sdd\release-363-github.log`、`.superpowers\sdd\release-363-status-verify.log`（站点上传日志在 `release:upload` 的 stdout 里） |

> **仍未做 / 待后续**（不阻塞本次发版）：sticky 段头、安装包代码签名、winget PR、远程条目的就地改名真机验证、虚拟滚动下编辑态行被滚出的边界。
