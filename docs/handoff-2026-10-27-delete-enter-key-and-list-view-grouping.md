# 交接文档：删除确认框「回车 = 删除」 + 列表视图按类型分区 + 3.6.3 发版（2026-10-27）

> **本文是自足的接续入口。** 新会话只读本文，即可接手，不需要回放历史对话。
> 上游远程模块的完整背景见 `docs/handoff-2026-10-26-remote-transfer-progress-and-overwrite.md`；
> 项目通用约束见外层仓 `F:\soft\00selfmade\filemanager\AGENTS.md`（**必读**）。

---

## 0. 一分钟版

| 问 | 答 |
|---|---|
| 用户要什么 | ① 永久删除确认框里**直接回车要等于「删除」**（现在回车什么都不会发生）；② 目录窗口的**列表视图**要像缩略图视图那样，在「文件夹 / 图片 / 视频 / 其它文件」之间给出区隔；③ 优化与测试通过后**直接打包发版**，升级 README、新版本说明、aiec 版本记录，上传 GitHub 与 aiec |
| 这次发哪个版本 | **3.6.3**（3.6.2 已于 2026-10-26 发布，版本号从 3.6.2 起 bump） |
| 现在卡在哪 | **代码一行都还没写**。本轮只完成了「定位 + 方案定稿 + 任务书落盘」；随后被工具预算钩子拦死（见第 9 节） |
| 已落盘的东西 | 只有子代理任务书：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\task-list-view-grouping.md`（6,246 B，gitignore 内，不入库） |
| 仓库状态 | 内层仓 `sigma-file-manager` 分支 `main`，HEAD `dd33e5f0`，**工作区干净**，已与 `origin/main` 同步 |
| 最容易踩的坑 | ① **真机验收只能用 `tauri build` 产物**，`tauri dev` 的路由页内容区全空白，界面结论一律作废；② 打包前必须停 `alpha-file-manager` 进程**并把 `src-tauri/target/release/alpha-file-manager.exe` 移走**（只杀进程不够，上一轮就是这么被 `os error 5` 卡住的）；③ `eslint .` 是整仓口径，基线 **188 errors**；④ `.cmd` 里 `echo` 不能出现 `>`；⑤ `.ps1` / `.cmd` 必须纯 ASCII |

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

- `package.json` = **3.6.2**；`src-tauri/Cargo.toml`、`src-tauri/default-file-manager-common/Cargo.toml`、`src-tauri/default-file-manager-launcher/Cargo.toml` 三处都 = 3.6.2（`src-tauri/tauri.conf.json` 的 version 指向 `"../package.json"`，不用改）。
- **本次发版目标版本 = 3.6.3**（`release:prepare --bump patch` 会把三处一起 bump，记得用 `npm run sync-version` 或让脚本处理）。
- 3.6.2 已发布：GitHub `kizemo/alpha-file-manager` release `v3.6.2`（非 draft / 非 prerelease）、aiec.fun 站点 `version.json` 已升到 3.6.2、winget 3.6.2 清单四件套已备好。
  - 3.6.2 安装包：`release/Alpha File Manager_3.6.2_x64-setup.exe`，20,671,224 B，sha256 `65B3AF7CE60755D6163A1C4E0ED42C41A7D97FEAD02BD887500FF5E000BFDC89`（GitHub 资产 digest / 站点 `version.json` / winget `InstallerSha256` 四方一致）。
- `release/` 目录里堆了 2.2.0 → 3.6.2 共 42 个历史安装包，**都在，别删**（winget 清单要按 sha256 比对这里的实际文件）。

### 2.3 项目形态要点

- Tauri 桌面应用（前端 Vue 3 + TS + Pinia，UI 组件库是 **reka-ui**，不是 Radix 直用），Rust 后端在 `src-tauri/`。
- 目录窗口的「文件浏览器」组件群在
  `src/modules/navigator/components/file-browser/`。
- i18n：17 个语言包在 `src/localization/messages/*.json`（**中文的 locale code 是 `ch` 不是 `zh`**）；约定是**只有 `ch.json` 用中文，其余 16 个沿用英文原文**；17 个 key 集合必须完全一致。
  已有 `locale-json-format.test.ts` 要求每个文件 == `JSON.stringify(parsed, null, 2) + '\n'`。
- 根 `CHANGELOG.md`（英文）+ `public/changelog/<15 个 locale>/changelog.md`（每个语言包一份）。

---

## 3. 本轮已完成到哪一步

| 事项 | 状态 | 产物 / 证据 |
|---|---|---|
| 定位 A 的根因与实现位置 | ✅ 完成 | 见第 4 节，带文件路径与行号 |
| 定位 B 的实现位置与复用点 | ✅ 完成 | 见第 5 节，带文件路径与行号 |
| A 的方案定稿 | ✅ 完成 | 见第 4.4 节，含可直接抄的代码 |
| B 的方案定稿 | ✅ 完成 | 见第 5 节，含全部一致性要求 |
| B 的 worker 任务书 | ✅ 已落盘 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\task-list-view-grouping.md` |
| A 的代码 | ❌ **未写** | 下一轮第一个动作 |
| B 的代码 | ❌ **未写** | 下一轮派 worker |
| 单测 | ❌ 未写 | 第 4.5 / 5.4 节给了用例设计 |
| 真机验收 | ❌ 未做 | 第 6 节给了顺序 |
| 3.6.3 发版 | ❌ 未做 | 第 7 节 |

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

## 6. 下一轮执行顺序（照做即可）

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

## 7. 3.6.3 发版流程

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
| 1 | winget PR `microsoft/winget-pkgs#460744`（3.5.1）是否已 MERGED | **本轮始终没确认成功**；同包 ID 不允许同时开两个 PR，所以 3.6.3 的 winget 清单可能仍提不了 |
| 2 | 安装包**代码签名**（需用户自备证书） | 早前提出，仍未决 |
| 3 | 是否要催 winget 版主审 `#460744` | 早前提出，仍未决 |
| 4 | 远程条目的 `copy`/`cut`/`paste` 是否放开 | 产品决定，见 `src/utils/entry-action-policy.ts:70` 的 `REMOTE_PHASE0_BLOCKED_ACTIONS` |
| 5 | 3.6.3 是否也推 winget 清单 | 建议：备好四件套 + `npm run check:winget` 绿，等 PR 通道打开再提 |

---

## 11. 一句话接力 prompt

> 接手 `F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager，Tauri + Vue3 + reka-ui）的 3.6.3 发版任务：先读 `F:\soft\00selfmade\filemanager\AGENTS.md` 与 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-27-delete-enter-key-and-list-view-grouping.md`（本文），按本文第 4 / 5 节定稿的方案完成「永久删除确认框回车=删除」与「列表视图按文件夹/图片/视频/其它文件分区」两项优化的实现与单测，再按第 6 节顺序做真机验收（必须用 `tauri build` 产物，`tauri dev` 路由页内容区全空白、界面结论一律作废），最后按第 7 节发版 3.6.3（README、根 CHANGELOG、15 个语言包 changelog、站点 `version.json`、GitHub release、winget 清单），发版权威流程见 `docs/operations/release-workflow.md`，全程遵守 `AGENTS.md` 的中文输出与各项硬约束。

---

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
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\task-list-view-grouping.md   ← worker 任务书（本轮唯一落盘产物）
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\push-with-fallback.cjs
F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\superseded\                   ← 被挪开的构建产物
E:\Users\Duanyi\.minimax\.minimax\maintenance\tool_budget_hook.py                              ← 本轮的拦截器
E:\Users\Duanyi\.minimax\.minimax\maintenance\task-state\LATEST.md                               ← 自动交接卡
```
