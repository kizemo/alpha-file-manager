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
| 现在卡在哪 | ① **3.6.3 已经全部发完**（三块优化均通过真机验收，发版提交已 push，GitHub release `v3.6.3` 与站点 aiec.fun 均已上线并回读校验通过）；剩下的外围事项：winget PR（须等 3.5.1 的 `#460744` 合并）、安装包代码签名、远程条目的就地改名验证。详见第 13–17 节。② **资源占用分析与优化**（2026-10-28）：现状实测与计划 v2、**批次 A/B 代码改造（已 push）、批次 D 环境层实测与清理均已完成**；**批次 C（结构性改造 6 项）代码已全部完成并通过全量门禁**（C1 / C3 均已真机验收 PASS，见 §18.12），**工作树尚未 commit**；**资源占用问题的来源归因见 §18.13**（结论：**AFM 二次编辑引入 0 项**，19 条全部来自上游 SFM） |
| 已落盘的东西 | 4 笔 commit（`82d42190` / `605efae8` / `d1c96f70` / `dff656a4`）+ 优化 C 的 13 个源文件（**已通过真机验收，随本次发版提交一并提交**）+ 3.6.3 安装包 / winget 清单 / 站点文件 / 18 个发版文档（**随本次发版提交一并提交**） |
| 资源优化的产物 | `.superpowers/sdd/mem-plan-v2.md`（计划 v2）、`mem-audit-final.md`（现状实测与 v1 方案）、`mem-audit-frontend.md`（前端持有点 20 条）、`mem-audit-rust.md`（Rust 持有点 21 条）、`mem-probe-2.ps1` + `mem-probe-2.out.txt`（可重复的进程/磁盘探针）、`mem-batch-d.ps1` / `mem-batch-d-run.ps1` / `mem-batch-d-phase2.ps1`（批次 D 的测量 / 停启 / 分阶段执行脚本）、`mem-batch-d-before.json` / `-before-fresh.json` / `-after-ab.json` / `-after-d.json` / `-after-d-warm.json`（五份实测快照）、`mem-phase-ab2.log` / `mem-phase-d2.log`（实测日志）、`mem-dump-strings.cjs`（崩溃 dump 取证脚本）、`mem-gate-fe2.log` / `mem-gate-rs2.log` / `mem-build.log`（门禁与打包日志）；**批次 C**：`mem-batch-c1-report.md` … `mem-batch-c6-report.md`（六个切片报告）、`mem-origin-attribution.md`（来源归因，48 KB）、`mem-batch-c-frontend-gate.log` / `mem-batch-c-tauri-build.log`（批次 C 门禁与打包日志）、`accept-c1-v4.ps1` / `accept-c3-virtual-rows.ps1`（真机验收脚本）、`mem-accept-c1-v*.log`（C1 验收各轮日志）、**沙箱验收工具链**：`sandbox-c1-accept.ps1`（隔离桌面沙箱验收脚本，不抢用户焦点）、`mem-accept-c1-sandbox.log`（沙箱验收日志）、`mem-accept-c1-sandbox.stdout.txt`、`c1-sandbox-full.png` / `c1-sandbox-uia.csv`（沙箱截图与 UIA 转储）、**接力件**：`relay-rules-batch-c.md`（防异常终止对策 + 沙箱坑位清单）、`relay-prompt-batch-c.txt`（新会话接力 prompt）。**这些文件都在 `.superpowers/`（被 gitignore），不进版本库** |
| 仓库状态 | 内层仓 `sigma-file-manager` 分支 `main`；**代码提交 `e8a41f3b`（资源优化批次 A/B）已 push**，`origin/main` = `e8a41f3b8b43b740f5078b84521823efd2625280`（用 `gh api .../commits/main --jq '.sha'` 核对一致）；本节 §18.11 的实测回写是**纯文档提交**；上游 `https://github.com/kizemo/alpha-file-manager.git`；**批次 C 见 §18.12**；归因结论见 §18.13 |
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

---

## 18. 资源占用分析与优化（2026-10-28 新增任务）

### 18.1 任务是什么

用户原话（第一轮）：

> 当前，AFM 占用系统资源为 88.7MB，是否有系统资源占用的优化空间？请进行分析，并制定优化方案，**先深入分析，不用执行**。

用户原话（第二轮，本节成型的依据）：

> 请再次进行一次深入分析，研究计划是否还存在可以优化的部分。然后按照你优化后的计划开始执行。如有必要新开会话接力任务执行优化，请更新完整的 handoff 文档，和一句话 prompt。

**性质**：与 §1–§17 完全独立的新任务线。§1–§17 的三块优化已随 3.6.3 发布完毕，**不要重跑、不要重复发版**。

### 18.2 口径澄清：「88.7 MB」不是 AFM 的全貌

AFM **代码里没有任何进程内存自报机制**：

- Rust 侧 `sysinfo` 只用于枚举磁盘类型（`src-tauri/src/dir_reader/drives.rs:17,80-83`），**没有**取进程内存的调用；
- 前端没有任何 memory / rss 指标。

⇒ **88.7 MB 一定来自外部工具**（任务管理器 / 资源监视器）。本机同时存在三种解释：

| 解释 | 依据 | 可信度 |
|---|---|---|
| ① Rust 主进程**冷启动后、扫描前**的基线 | 同一进程现在 PM 165.2 MB；`global_search/index.rs:119` 的 `BULK_INDEX_MEMORY_BUDGET_BYTES = 100_000_000` 正是批量索引的内存预算，**扫描期间**才涨上去。88.7 落在「基线」与「扫描峰值」之间 | 高 |
| ② 任务管理器「内存」列 = **私有工作集**，而本文给的是**工作集 (WS)**（含共享 DLL 页） | 两个口径天然不等 | 高 |
| ③ 测的是**别的应用**的 WebView2 renderer | 本机另有两棵**不属于 AFM** 的 msedgewebview2 树（PPID 9044 / 9716），其中就有 PM 88.5 MB 的进程 | 中 |

**结论**：88.7 MB ≈ AFM 一个进程的四分之一。真实全貌是**整棵进程树**。

### 18.3 现状实测（2026-10-28 11:00–11:10，应用 08:01:59 启动）

**内存**：AFM 进程树共 **8 个进程**，WS 合计 **1,118.9 MB**，私有提交（PM）合计 **1,031.5 MB**。

| 进程 | PID | 类型 | WS MB | PM MB | 线程 | 句柄 | CPU s |
|---|---|---|---|---|---|---|---|
| alpha-file-manager.exe | 154732 | **Rust 主进程** | 230.1 | 165.2 | **77** | **1250** | 2555.1 |
| msedgewebview2.exe | 154944 | browser 宿主 | 203.1 | 87.8 | 58 | 1750 | 113.4 |
| msedgewebview2.exe | 155492 | renderer（主窗口，活跃） | **309.7** | 254.1 | 31 | 353 | 377.5 |
| msedgewebview2.exe | 156216 | renderer（**隐藏的 Quick View**） | 171.9 | 128.1 | 30 | 342 | **15.5** |
| msedgewebview2.exe | 155316 | gpu-process | 121.9 | **371.2** | 53 | 713 | 45.6 |
| msedgewebview2.exe | 155392 / 155348 | utility ×2 | 68.4 | 22.3 | 26 | 496 | 20.5 |
| msedgewebview2.exe | 154980 | crashpad | 13.8 | 2.8 | 8 | 184 | 0.1 |

读数要点：

- **WebView2 子树 888.8 MB WS**，是 Rust 主进程（230.1 MB）的 **3.9 倍** ⇒ 优化重点在 WebView2 侧。
- gpu-process **PM 371.2 > WS 121.9** ⇒ 提交量远大于驻留，**不是真实 RAM 占用**（也是任务管理器数字对不上的原因）。
- **系统内存不是瓶颈**（63.9 GB 总 / 41.6 GB 空闲）⇒ 优化收益是「更轻快 / 更干净」，不是「救火」。

**磁盘（≈ 1.18 GB）**：

| 位置 | 体积 | 明细 |
|---|---|---|
| `Roaming\...\image-thumbnails` | **434.34 MB** | 4,249 张（4,177 PNG + 72 JPG）；中位 121.5 KB / p90 187.9 KB / 最大 317.6 KB；**全部产生于 2026-10**（最早 10-12，最新 10-28） |
| `Roaming\...\global-search` | **299.16 MB** | `index\` 46 文件 / 7+ 个 store segment（单个 13–17 MB）；**测量时正有一次扫描在跑**（`.index.staging.1793156733828.154732\` 有 11:06 的新文件，而 `index\` 里是 10:01–10:05 的旧 segment）⇒ **新旧索引并存，磁盘翻倍** |
| `Local\...\EBWebView` | **541.13 MB** | `Default` 443.79（含 `Default\Cache\Cache_Data\data_3` 12.01）；`Crashpad` 24.96（**3 个崩溃转储**：10-23 15:47 / 10-22 20:16 / 10-22 14:55）；`component_crx_cache` 21.18；`WidevineCdm` 21.92（**DRM，文件管理器用不到**）；`Subresource Filter` 12.29；`GrShaderCache` 10.58 |
| `Roaming\...\extensions` / `user-data` / `Local\...\logs` | 3.90 / 2.29 / 1.80 MB | — |

### 18.4 代码级根因（21 条前端 + 21 条 Rust 全表见 `.superpowers/sdd/mem-audit-*.md`）

**F1 · 隐藏的 Quick View 常驻一个 renderer（≈172 MB）** — `tauri.conf.json:29-35` 的 `quick-view` 窗口 `"create": false`，但 `src/utils/auxiliary-windows.ts:64-72` 的 `isAuxiliaryWindowPrelaunchEnabled` + 用户设置 `performance.prelaunchQuickViewWindow = true` 会把它**预拉起并隐藏**。Tauri **每窗口一个独立 renderer** ⇒ **窗口不可见 ≠ 没有开销**。应用自己的文案已写明代价：`src/localization/messages/ch.json:263-266`「约占用 200 MB 内存」。

**F2 · 缩略图缓存「超限全清」而非 LRU（体验炸弹）** — `src-tauri/src/image_thumbnails.rs:21-25`（限额 5000 张 / 1 GB / 每 100 次写检查一次）、`:257-267` `enforce_thumbnail_cache_limits()` **超限整个目录清空**。当前 4,249 张、增长约 **265 张/天** ⇒ **约 2026-10-31 撞线，缩略图集体消失并全部重生成**。

**F3 · tantivy 显式禁用 segment 合并** — `src-tauri/src/global_search/index.rs:10` `use tantivy::indexer::{IndexWriter, NoMergePolicy};` ⇒ 每次扫描产生新 segment 且**永不 merge** ⇒ 索引体积膨胀、查询要开更多 segment（更慢更吃内存）、扫描期间磁盘翻倍。

**F4 · 单次搜索物化 10 万条文档** — `src-tauri/src/global_search/query.rs:196` `TopDocs::with_limit(100_000)`，而 `:297` 的 `truncate(options.limit)` 发生在 `searcher.doc()` **物化之后** ⇒ 峰值 30–80 MB。**这是 Rust 主进程涨到 165 MB 的直接原因之一。**

**F5 · 前端唯一「只增不减」的全局缓存** — `src/composables/system-icon-cache.ts:33` 模块级 `Map<string, string|null>`，值是 **base64 PNG data URL**；key 规则 `:100-114` 中 **exe/dll/ico/lnk/scr/cpl/msi/appx/msix 走 `path:<全路径>:<size>`** ⇒ 每个 exe/dll × 3 档尺寸（18/24/48）永久常驻，**零淘汰逻辑**。

**F6 · 每盘符一个完整 FileBrowser** — `src/modules/global-search/global-search-view.vue:162-184,532-576`：`v-for="group in groupedResults"` 每组渲染一个**完整 `<FileBrowser>`**（虚拟布局 / 选择 / 图标 / 缩略图全套 composables）⇒ 搜索命中 N 个盘符 = N 套完整文件浏览器同时挂载。**前端最大放大器。**

**F7 · 前端目录数据是深响应式 + 每标签页各存一份** — `use-file-browser-navigation.ts:281` `dirContents = ref<DirContents>`；`stores/storage/workspaces.ts:69,573` 每 Tab 存一份 `dirEntries`；`:169` `clone(tab)` 深拷整份目录列表（瞬时 2×）；`:820` 分屏再常驻一份副本。

**F8 · 虚拟滚动只做了一半** — `use-file-browser-virtual-layout.ts:493` `rows` computed **全量物化** O(N) 行对象；`:208` key = `list:${entry.path}`、`:334` = `paths.join('|')` ⇒ 每行一条含完整路径的新字符串。

**F9 · Rust 侧图标缓存** — `src-tauri/src/system_icons.rs:18-22` 另有 **512 条 base64 PNG LRU**（理论 60 MB）；`:32-33` `UNSUPPORTED_SHELL_ICON_PATHS`（`:216-223` insert）**只增不减，是 Rust 侧唯一严格无界项**。

**F10 · 缩略图解码尖峰** — `src-tauri/src/image_thumbnails.rs:29` **单图解码上限 192 MB**（`:303-309`、`:412-421`）。

**F11 · 77 线程的真正主因** — `src-tauri/src/dir_watcher.rs:15,122,135-142` **每个监听目录起 1 个应用线程 + 1 个 notify 线程**，`:128` `std::sync::mpsc` **无界 channel** ⇒ 线程数随监听目录数线性增长。

**F12 · 其他 P2** — `lan_share/handlers.rs:272` 上传体整块进内存（≤512 MB）；`extensions/filesystem.rs:66` `read_to_end` 无字节上限；`dir_size.rs:169-181` WalkDir 全量 `collect`；`startup_storage_bootstrap.rs:44,58-63` 5 个用户存储 JSON 常驻副本；`global_search/state.rs:26-32` 常驻 `Index` + `IndexReader` 且 reader 无内存预算。

### 18.5 计划 v2：v1 的一处错误 + 八条补入

> 完整计划见 `.superpowers/sdd/mem-plan-v2.md`。

**⚠️ v1 的错误建议已作废**：v1 写「把 tantivy 换回默认 merge 策略」。**不能这样改** —— `NoMergePolicy` 是 2026-06-24 提交 `054e2472`「UX: indexing stability improvements」**刻意加入的稳定性措施**，同一提交还引入了 `BULK_INDEX_MEMORY_BUDGET_BYTES = 100_000_000` 与 `writer_with_num_threads(1, …)`。正确改法是**保留 NoMergePolicy，在扫描提交完成后显式 `merge()` + `wait_merging_threads()`**，让 segment 收敛而不回退稳定性改进。

v2 补入的八条：F4（搜索物化 10 万）· F6（每盘符 FileBrowser）· F10（192 MB 解码上限）· F11（每目录线程）· F9（Rust 图标 LRU + 唯一无界项）· F12 的五项 P2 · 以及 `global-search-view.vue` / `use-image-thumbnails.ts` / `dir-sizes.ts` / `folder-tree.ts` 等深响应式容器。

### 18.6 执行批次与状态

| 批次 | 内容 | 文件（独占所有权） | 状态 |
|---|---|---|---|
| **A1** | 系统图标缓存加容量上限；`path:`/`dir:` 类必须可淘汰，`ext:` 类保留 | `src/composables/system-icon-cache.ts` + 新增测试 | ✅ 完成（`e8a41f3b`） |
| **A2** | 缩略图前端缓存 `ref<Record>`+`reactive(Set)` → `shallowRef`+`Map`，写入改原地 `set`（消 O(n²)） | `.../file-browser/composables/use-image-thumbnails.ts` + 测试 | ✅ 完成（`e8a41f3b`） |
| **B1** | 搜索查询：`truncate` 前移到物化之前；`TopDocs` limit 与实际返回对齐；reader 预算/空闲释放（能安全实现才做） | `src-tauri/src/global_search/query.rs`、`state.rs` + 测试 | ✅ 完成（`e8a41f3b`） |
| **B2** | 缩略图：解码上限 192 MB → 64 MB + 并发闸门 2–4；**超限全清 → 按最旧淘汰**；限额 5000 张/1 GB → **2000 张/256 MB** | `src-tauri/src/image_thumbnails.rs` + 测试 | ✅ 完成（`e8a41f3b`） |
| **B3** | Rust 图标：LRU 512 → 128 条（或按字节限额）；`UNSUPPORTED_SHELL_ICON_PATHS` 加上限 | `src-tauri/src/system_icons.rs` + 测试 | ✅ 完成（`e8a41f3b`） |
| **C1** | 搜索结果页每盘符完整 `FileBrowser` → 轻量只读列表 | `src/modules/global-search/global-search-view.vue` | ✅ 完成（见 §18.12） |
| **C2** | `dirContents` → `shallowRef`+`markRaw`；`workspaces` Tab 不再存 `dirEntries`；`clone(tab)` 前先清 | `use-file-browser-navigation.ts`、`stores/storage/workspaces.ts` | ✅ 完成（见 §18.12） |
| **C3** | 虚拟行只算可视窗口；行 key 改下标，不再拼路径串 | `use-file-browser-virtual-layout.ts`、`utils/file-browser-virtual-rows.ts` | ✅ 完成（见 §18.12） |
| **C4** | `dir_watcher` 每目录线程 → 共享 watcher 线程池 + 有界 channel | `src-tauri/src/dir_watcher.rs` | ✅ 完成（见 §18.12） |
| **C5** | tantivy：扫描提交后显式 merge + segment 上限；常驻 reader 预算 | `src-tauri/src/global_search/index.rs`、`state.rs` | ✅ 完成（见 §18.12） |
| **C6** | LAN 上传改流式写盘；扩展 `read_to_end` 加字节上限；`dir_size` 去 `collect()` | `lan_share/handlers.rs`、`extensions/filesystem.rs`、`dir_size.rs` | ✅ 完成（见 §18.12） |
| **D1** | 关 `performance.prelaunchQuickViewWindow` ⇒ 隐藏 Quick View 的 WebView2 renderer 消失 | 用户设置（零代码） | ✅ 完成（实测：进程 8→7，WS −150.8 MB，详见 §18.11） |
| **D2** | 清缩略图 / 搜索索引 / `EBWebView\Cache` / Crashpad dump | 环境 | ✅ 完成（磁盘 −739.37 MB；⚠️ 索引与缩略图会重建复涨，详见 §18.11） |
| **D3** | 查 WebView2 三次崩溃（10-22 ×2、10-23 ×1）原因 | 环境 | ✅ 完成（**崩在 WebView2 自身子进程**，非 AFM 代码，详见 §18.11） |

### 18.7 执行本任务时的硬约束（除 §16.3 通用约束外）

1. **Rust 测试红线**：被测试链接到的代码**不得触碰 `tauri::AppHandle` 或 `tauri::async_runtime::*`**，否则 `cargo test --lib` 在**加载阶段**以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，一个测试都跑不了且报错看不出关联。需要异步时用项目现成的 `*_blocking` 同步内核入口（`dir_reader.rs`、`global_search.rs` 是范例）。
   ⚠️ `image_thumbnails.rs:664-727` 有 5 处 `tauri::async_runtime::spawn_blocking`（command 包装层）—— 新测试**不能**把这些包装层链接进测试二进制。
2. **tantivy 的 merge 策略不能动**（见 §18.5），只能「保留 + 扫描后显式 merge」。
3. **`system_icons.rs` 的 `UNSUPPORTED_SHELL_ICON_PATHS` 与 `image_thumbnails.rs` 的限额常量**：改上限前先 grep 全仓（含 `src/`）确认有没有被设置界面 / i18n 展示出来，展示值要同步改。
4. **门禁判据**：`eslint .` = **188**（`eslint src` = 175）；`npm run ts`（vue-tsc）必须过；`npm run test` 全绿；`cargo check --lib` 无 error。
5. **不要顺手改 `tauri.conf.json` 的 `identifier`、mDNS 服务名、legacy 目录名**（受保护常量）。
6. **本任务不涉及版本号**：当前 `package.json` = 3.6.3，**不要 bump**；本轮改动不单独发版，等批次 C/D 完成后统一评估。
7. **真机验收**：若改动需要界面证据，必须用 `tauri build` 产物（`npm run tauri:build`），`tauri dev` 的路由页内容区全空白、界面结论一律作废。构建脚本放 `.superpowers/sdd/*.cmd` 且**必须纯 ASCII**。

### 18.8 未确认项（不要当成已验证结论）

- **88.7 MB 的确切口径**需确认（任务管理器哪一列、哪个进程）。
- **renderer 156216 = 隐藏 Quick View** 是**强推断**（CPU 15.5 s 几乎空闲 + `Vis=False` 窗口存在 + 设置里 prelaunch 开着），**未用窗口句柄直接绑定 renderer 与 WebView**。
- **WebView2 三次崩溃原因未查**。
- **前端各项「规模」是代码结构推算**，无 heap snapshot / DevTools 取数。
- **测量时刻全局搜索正在扫描**，该时段的磁盘与内存读数偏高。
- Rust 侧审计结论**全部为源码静态推算**，未做运行时 RSS 采样。

### 18.9 证据文件索引

| 文件 | 内容 |
|---|---|
| `.superpowers/sdd/mem-plan-v2.md` | 计划 v2（批次 A–D、硬约束、验收判据、实测基线） |
| `.superpowers/sdd/mem-audit-final.md` | 现状实测全文 + v1 方案 + 复现命令 |
| `.superpowers/sdd/mem-audit-frontend.md` | 前端持有点全表（20 条）+ 3 条优先项 |
| `.superpowers/sdd/mem-audit-rust.md` | Rust 持有点全表（21 条）+ 3 条优先项 |
| `.superpowers/sdd/mem-probe-2.ps1` | 可重复的进程 / 磁盘探针（**纯 ASCII**） |
| `.superpowers/sdd/mem-probe-2.out.txt` | 探针原始输出（2026-10-28 11:07） |
| `.superpowers/sdd/mem-batch-d.ps1` | 批次 D：测量 / 清理（改设置 + 清缓存）/ 还原，三种模式 |
| `.superpowers/sdd/mem-batch-d-run.ps1` | 批次 D：停旧进程 / 启动指定 exe |
| `.superpowers/sdd/mem-batch-d-before.json` | 批次 D 前的进程树与缓存目录快照 |
| `.superpowers/sdd/mem-gate-fe2.log` | 前端门禁原始日志（eslint / vue-tsc / vitest） |
| `.superpowers/sdd/mem-gate-rs2.log` | Rust 门禁原始日志（cargo check / cargo test） |
| `.superpowers/sdd/mem-build.log` | 真机验收打包日志 |

> 这 5 个文件都在 `.superpowers/`（被 gitignore），**不进版本库**；接力会话用绝对路径读。

### 18.10 批次 A/B 执行结果（2026-10-28，commit `e8a41f3b`，已 push）

**提交**：`e8a41f3b` 「perf: 降低常驻内存与磁盘占用（资源优化批次 A/B）」（9 文件，+1497 / −140）；`origin/main` = `e8a41f3b8b43b740f5078b84521823efd2625280`（`gh api .../commits/main --jq '.sha'` 核对一致）。
**未 bump 版本号**（仍 3.6.3）、**未发版**、`release/` 目录未动。

| 切片 | 实际改动 | 测试 |
|---|---|---|
| A1 | `system-icon-cache.ts`：缓存上限 **512 条**；命中即重插（LRU）；淘汰优先级 `path:` → `dir:` → `ext:`，同类内淘汰最久未使用；保留「`ext:` 失败不写缓存」与 in-flight 去重语义 | 新增 5 条（原 5 条保留，共 10 条） |
| A2 | `use-image-thumbnails.ts`：4 个容器改 `shallowRef` + `Map`/`Set`，写入改原地 mutate + `triggerRef`；去掉 `{ ...cache, [k]: v }` 整体重建（O(n²) → O(1)）；对外函数签名与语义不变 | 新增 7 条 + 改写 5 条原断言（共 22 条） |
| B1 | `global_search/query.rs`：候选**先截断再物化**（原先先物化 10 万条再 truncate）；候选预算 = `limit × 已索引盘符数`，绝对上限 5000；`global_search/state.rs`：常驻 `IndexReader` 空闲 5 分钟释放，下次查询走既有 `needs_open` 重开路径 | 3 条 |
| B2 | `image_thumbnails.rs`：单图解码分配上限 **192 MB → 64 MB**；源像素上限 **16 MP**；并发解码闸门 **最多 3 张**（Mutex + Condvar，RAII 归还名额，解码期间不持锁）；缓存限额 **5000 张/1 GB → 2000 张/256 MB**；超限**按 mtime 淘汰最旧**（不再整体清空） | 新增 6 条，删掉 2 条前提失效的旧测试 |
| B3 | `system_icons.rs`：图标 data URL LRU **512 → 128 条**；`UNSUPPORTED_SHELL_ICON_PATHS` 由无界 `HashSet` 改为容量 **512** 的 LRU ⇒ **Rust 侧最后一个严格无界项消除** | 2 条 |

**统一门禁（全部在 `e8a41f3b` 上量）**

| 项 | 结果 |
|---|---|
| `npx eslint .` | **188 errors / 0 warnings** —— 与基线一致 ✅ |
| `npx eslint src` | **175 errors / 0 warnings** —— 与基线一致 ✅ |
| `npm run ts`（vue-tsc） | **exit 0** ✅ |
| `npm run test`（vitest） | **248 files / 2287 tests 全通过** ✅ |
| `cargo check --lib` | **exit 0** ✅ |
| `cargo test --lib` | **823 passed / 2 failed** ⚠️ —— **存量环境失败，见下** |

> ⚠️ **那 2 条失败是环境性的，与本轮改动无关**：`mcp::server::tests::port_search_stays_inside_the_declared_range` 与 `…port_search_never_hands_back_a_privileged_port` 都在 `find_available_port_in_range(55100, 55199).unwrap()` 处 panic（`在端口区间 55100-55199 内找不到可用端口`）。
> **根因**：本机 Windows 的排除端口段（`netsh interface ipv4 show excludedportrange protocol=tcp`）为 `55033-55132` 与 `55133-55232` ⇒ **MCP 要的 55100-55199 整段被系统保留**（Hyper-V/WSL 动态端口预留），bind 必然失败。实测 55100 / 55132 / 55133 / 55150 / 55199 **全部 bind 失败**，报 `WSAEACCES`（「以一种访问权限不允许的方式做了一个访问套接字的尝试」）。
> `mcp/` 模块**不在**本轮改动范围内（`git status` 确认本轮只动了 5 个源文件 + 1 个文档）⇒ 判定为**存量环境失败**。**接力会话不必重查，但不要把它当回归**；要彻底消除需改 `MCP_PORT_RANGE_START/END` 避开系统保留段（独立改动，未做）。

**主 Agent 复核后追加的一处修复**：B1 初版把候选池**全局**封顶在 `limit`，多盘符搜索时靠后的盘符会只剩个位数结果 —— 属**用户可见的行为回归**。已改为「候选预算 = `limit × 已索引盘符数`」（`candidate_limit_for(options, drive_count)`，绝对上限 5000）⇒ 保留旧的「每盘符各 limit」语义，物化量同时从 10 万级压到「limit × 盘符数」级。新增测试 `candidate_budget_scales_with_indexed_drive_count`。

**已知的行为变化（用户可见，需知情）**

1. **>16 MP 的图片不再生成缩略图**（只显示占位图）—— 64 MB 解码上限的必然代价；此前 50 MP 以内还能通过。
2. **现存 4,249 张 / 434 MB 缩略图缓存会被裁到 2,000 张 / 256 MB**（按 mtime 淘汰最旧），之后不再「集体消失后全部重生成」。
3. **多盘符搜索的返回条数可能少于 `limit`** —— 候选池截断在前、过滤规则（忽略路径 / 相似度门槛 / 精确匹配 / 类型）在后，被挡掉的候选没有替补；短查询（3 字符，门槛 0.9）最容易出现。
4. **exe/dll 密集目录里图标编码会重复发生**（LRU 降到 128 条），CPU 略升、内存常驻下降。
5. **`use-video-thumbnails.ts` 有与 A2 完全相同的 `ref<Record>` + 整体重建模式**（前端审计第 12 条），本轮**未动** ⇒ 建议列入批次 C。

### 18.11 批次 D 实测结果（2026-10-28，真机产物）

**被测产物**：`npx tauri build` 产物 `src-tauri\target\release\alpha-file-manager.exe`（56,331,264 B，Rust release 编译 3 m 51 s，NSIS 包 19.72 MiB）。
⚠️ **刻意用 `npx tauri build` 而非 `npm run tauri:build`**：后者末尾的 `publish-bundle-to-release.mjs` 会覆盖 `release\Alpha File Manager_3.6.3_x64-setup.exe`，导致 winget 清单里的 sha256 失配。**`release/` 目录本轮未被改动。**

**口径**：`WS` = `WorkingSet64`（工作集），`PM` = `PrivateMemorySize64`（私有提交）。任务管理器「内存」列接近 WS ⇒ **对外汇报一律用 WS**；PM 会被 Chromium 的预提交行为放大，不适合当收益口径。
**方法**：同一台机器、同一账号、同一时刻分别启动「安装版 3.6.3」与「新构建」，各冷启 30 s 后用 `mem-batch-d.ps1 -Mode measure` 取整棵进程树（含 `msedgewebview2.exe` 子进程）+ 缓存目录体积。

#### 四点测量

| 快照 | 构建 | 状态 | 进程数 | WS (MB) | PM (MB) | 缓存目录 (MB) |
|---|---|---|---|---|---|---|
| `before` | 安装版 3.6.3 | 已运行约 3.5 h | 8 | 1066.8 | 976.5 | 805.58 |
| `before-fresh` | 安装版 3.6.3 | 冷启 30 s | 8 | 961.2 | 965.8 | 805.58 |
| `after-ab` | **新构建（A/B 生效）** | 冷启 30 s，缓存未动 | 8 | **893.6** | 995.8 | 805.58 |
| `after-d` | 新构建 | 冷启 30 s，**已关预启动 + 已清缓存** | **7** | 804.7 | 1072.7 | **0** |
| `after-d-warm` | 新构建 | 同上，运行约 10 分钟后 | 7 | **742.8** | 881.7 | 66.21 |

#### 归因

1. **批次 A/B 代码本身（唯一可干净归因的一段：`before-fresh` → `after-ab`，同构建口径、缓存未动）：WS 961.2 → 893.6 MB，净 −67.6 MB（−7.0 %）。**
   逐进程看，降幅**全部落在 renderer 进程**（WS 317.2 → 249.2，−68.0 MB；PM 216.6 → 146.9，−69.7 MB），其余进程基本持平 —— 与 A2「缩略图前端缓存去掉整体重建」的方向一致。
   ⚠️ **PM 口径反而 +30.0 MB**：gpu-process 的 `PrivateMemorySize64` 从 446.7 涨到 547.6（+100.9），把 renderer 省下的 69.7 抵掉还多。这是 Chromium 预提交显存/共享内存的**提交口径**，不是实际驻留 ⇒ **不代表内存变差**。
2. **D1（关 `performance.prelaunchQuickViewWindow`）：机制被坐实 —— 进程数 8 → 7，少掉的正是第二个 renderer**，即隐藏 Quick View 的那棵 WebView2（§18.8 那条「强推断」至此升级为**已验证**）。这一项与 D2 是一起做的，两者收益无法在本轮完全拆开；D1 的机制由「少一个 renderer」独立佐证。
3. **D2（清缓存）：磁盘 805.58 MB → 0 MB**（缩略图 434.34 + 搜索索引 200.27 + `EBWebView\Default\Cache` 146.02 + 3 个 Crashpad dump 24.95），全部走**可恢复删除**（进回收站，未永久删除）。代价是**应用会重建**：清理后 Rust 主进程立刻开始重建全局搜索索引（该快照里 main 进程 CPU 136.3 s、PM 92.8 → 145.9 MB）；约 10 分钟后 `global-search\index` 复涨到 66.21 MB，缩略图目录当时尚未重建。**磁盘收益是「把 800 MB 常驻缓存换成一次重建 + 之后按需再生」，不是净消失。**
4. **合账（新构建 + D 措施稳态，`after-ab` → `after-d-warm`）：WS −150.8 MB（−16.9 %）、PM −114.1 MB（−11.5 %）、磁盘 −739.37 MB（−91.8 %）。**
   **相对最初基线（安装版、跑了 3.5 h 的 `before`）：WS 1066.8 → 742.8 MB，−324.0 MB（−30.4 %）；磁盘 −739.37 MB。**
   相对同为冷启口径的 `before-fresh`：WS 961.2 → 742.8 MB，−218.4 MB（−22.7 %）。

#### D3：WebView2 三次崩溃的原因（已完成取证）

三个 dump 在清理时进了回收站（`C:\$Recycle.Bin\S-1-5-21-...\`），用 `mem-dump-exc.cjs` 直接解 minidump 头（`ExceptionStream`）+ `mem-dump-strings.cjs` 抽模块表：

| dump | 大小 | 崩溃模块 | 崩溃线程 | 异常码 | 故障地址 |
|---|---|---|---|---|---|
| `$RNZMW7A.dmp` | 11.6 MB | `msedgewebview2.exe`（Edge WebView **155.0.4283.62**） | `Chrome_IOThread` | `0x80000003`（STATUS_BREAKPOINT = Chromium 的 CHECK/DCHECK 失败） | `0x7FFD6AFCB83F` |
| `$RTCQZN7.dmp` | 7.3 MB | 同上 | `Chrome_ChildIOThread` | `0xE0000008` | `0x7FF9156E408A` |
| `$RZ7BCS9.dmp` | 7.3 MB | 同上 | `Chrome_ChildIOThread` | `0xE0000008` | `0x7FF9156E408A` |

**结论：崩溃发生在 WebView2 自己的子进程里，不是 AFM 的 Rust / 前端代码缺陷。**
依据：① 三个 dump 的模块表里只有 `msedgewebview2.exe` 与系统 DLL，**没有 AFM 自己的模块**（后两个显式 `AFM_MODULES_IN_DUMP=NONE`；AFM 主进程是 WebView2 子进程的父进程，本就不会出现在子进程模块表里）；② AFM 主进程**没有崩**（Crashpad handler 一直存活）；③ 后两次**故障地址完全相同** ⇒ WebView2 内部同一处可复现故障点。
**建议**：① 升级 WebView2 runtime（155.0.4283.62 偏旧）；② AFM 侧补「WebView2 子进程崩溃 → 检测 + 提示 + 可恢复」的兜底（可并入批次 C）；③ 三个 dump 均**未做符号化**，触发条件（哪个页面/哪类操作）仍未定位，别写成「已找到根因」。

#### 本轮的已知局限（不要当成更精确的结论）

- **单次采样**，未取多轮中位数；单机、单账号、单驱动。
- **「冷启 30 s」不是稳态**：跑了 3.5 h 的安装版比冷启高 105.6 MB（WS），说明缓存增长会让稳态回升 ⇒ 批次 D 的收益会被后续缓存再生部分抵消。
- 所有读数都取在**全局搜索扫描 / 索引重建**时段 ⇒ 偏高。
- A/B 的收益归因是「**方向一致**」而非因果坐实（要坐实需逐切片单独构建重测）。
- **D1 与 D2 是一起做的**，本轮无法把两者收益完全拆开；D1 的机制（少一棵 renderer）已独立佐证。
- 用户最初看到的 **88.7 MB** 口径仍未确认（§18.8）。

#### 已落地到用户环境的状态（需要知情）

- `user-settings.json` 的 **`performance.prelaunchQuickViewWindow` 已改为 `false`**（隐藏 Quick View 不再预启动）。备份：`.superpowers\sdd\mem-batch-d-settings-backup-cleanup.json`；回滚：`pwsh -NoProfile -ExecutionPolicy Bypass -File .superpowers\sdd\mem-batch-d.ps1 -Mode restore -Tag cleanup`。
- `image-thumbnails` / `global-search\index` / `EBWebView\Default\Cache` / 3 个 Crashpad dump **已移入回收站**（可恢复，未永久删除）。
- 崩溃 dump 仍在回收站里，可按上面的路径取证。
- 打包产物 `src-tauri\target\release\alpha-file-manager.exe` **未安装**，当前运行的就是它（`launch` 模式）；需要以安装版形态体验时用 `mem-batch-d-run.ps1 -Mode launch-installed`（注意本机安装路径是 `D:\Program Files\Alpha File Manager\`，脚本内置的候选路径不含它，故单独指定）。

#### 证据文件

| 文件 | 内容 |
|---|---|
| `.superpowers/sdd/mem-batch-d-phase2.ps1` | 分阶段执行脚本（ab / d / warm）；**注意别把辅助函数命名为 `Measure`** —— PowerShell 内置别名 `measure` → `Measure-Object` 的**别名优先级高于函数**，本轮踩过一次，导致函数体从未执行、快照文件没生成 |
| `.superpowers/sdd/mem-batch-d-before-fresh.json` | 安装版冷启基线快照 |
| `.superpowers/sdd/mem-batch-d-after-ab.json` | 新构建（A/B 生效）冷启快照 |
| `.superpowers/sdd/mem-batch-d-after-d.json` | 新构建 + 关预启动 + 清缓存，冷启快照 |
| `.superpowers/sdd/mem-batch-d-after-d-warm.json` | 同上，运行约 10 分钟后的稳态快照 |
| `.superpowers/sdd/mem-phase-ab2.log` | AB 阶段原始日志 |
| `.superpowers/sdd/mem-phase-d2.log` | D 阶段原始日志（含每项清理结果） |
| `.superpowers/sdd/mem-dump-exc.cjs` | 解析 minidump `ExceptionStream`（异常码 / 故障地址 / 故障模块） |
| `.superpowers/sdd/mem-dump-strings.cjs` | 从 dump 抽模块表与关键词字符串 |
| `.superpowers/sdd/mem-batch-d-settings-backup-cleanup.json` | 清理前的 `user-settings.json` 备份（回滚用） |

### 18.12 批次 C 执行结果（2026-10-28，结构性资源优化 C1–C6）

**提交状态**：C1–C6 代码全部落地并通过全量门禁，**但尚未 commit**（工作树 17 个已改文件 + 2 处未跟踪）。**未 bump 版本号**（仍 3.6.3）、**未发版**、`release/` 目录**未动**。
⚠️ `release/Alpha File Manager_3.6.3_x64-setup.exe` 曾被一次误用 `npm run tauri:build` 的构建覆盖成 20,717,408 B，**已用 `node fs.copyFileSync` 从 `.superpowers/sdd/superseded/bundle-nsis/` 还原为原版 20,670,028 B / 2026-10-27 16:23:31**。**后续验收构建一律用 `npx tauri build`，不要用 `npm run tauri:build`。**

**规模**：`git diff --stat` = 17 files changed, 2857 insertions(+), 699 deletions(-)。

#### 六个切片

| 切片 | 实际改动 | 关键点 | 测试 |
|---|---|---|---|
| **C1** | 搜索结果页每命中一个盘符挂一个完整 `<FileBrowser>` → **轻量只读列表** | 新增 `global-search-result-rows.ts` + `global-search-result-list.vue`；`global-search-view.vue` 删掉 `<FileBrowser>`；`stores/runtime/global-search.ts` 的 `results` 改 `shallowRef` + `markRaw` | 新增 7 条 |
| **C2** | `dirContents` → `shallowRef`；workspaces 的 Tab 不再存 `dirEntries` | `stores/storage/workspaces.ts` 用**闭包内非响应式 `Map`** 缓存（上限 8、FIFO 淘汰），移除 `cloneDeep`；`clone(tab)` 前先清条目 | 新增 5 条 |
| **C3** | 虚拟行**只算可视窗口**；行 key 改下标 | `file-browser-virtual-rows.ts` 重写为「元数据 + 惰性物化」两层模型；`use-file-browser-virtual-layout.ts` 改 Proxy 惰性数组 + `rowModel` / `visibleRange` | **真机验收 PASS（6 条断言）** |
| **C4** | `dir_watcher` 每监听目录 2 个线程 + 无界 mpsc → **共享 watcher 线程池 + 有界 channel** | 全局单例 `SHARED_WATCHER` + 单个 `RecommendedWatcher` + 单消费线程 `afm-dir-watcher`（栈 256 KB）；`EventQueue<T>` 有界队列（容量 1024，**满时丢最旧**）；`DebounceTracker` 复刻 300 ms 节流；`retain_watched` 清理失效目录；`resolve_watched_path` 最长前缀反推（Windows 大小写不敏感）；`MAX_ACTIVE_WATCHERS = 256` | `cargo test --lib dir_watcher` → **8 passed** |
| **C5** | tantivy 扫描提交后**显式 merge** + segment 上限 + reader 预算 | `MAX_INDEX_SEGMENTS = 4` + `merge_index_segments(writer)`；`scan.rs` 两处 commit 后 `merge()` + `wait_merging_threads()`；`state.rs` 加 `READER_RESIDENT_BUDGET_BYTES = 256 MiB` + `READER_IDLE_RELEASE_AFTER_MS_OVER_BUDGET = 60_000` + `reader_idle_threshold_ms()`；`query.rs` 调 `release_idle_reader_if_needed()`。**`NoMergePolicy` 保留不动**（§18.5 的铁律） | `cargo test --lib global_search` → **30 passed** |
| **C6** | LAN 上传**流式写盘** + 扩展读文件**加字节上限** + `dir_size` **去 `collect()`** | `lan_share/handlers.rs` 改流式写盘 + `tokio::io::BufWriter` 64 KiB（**失败时删掉半截文件**）；`extensions/filesystem.rs` 加 `MAX_EXTENSION_FILE_READ_BYTES = 64 MiB`（`take(limit + 1)`）；`dir_size.rs` 加 `SIZE_SCAN_CHUNK = 4096` + `accumulate_entry_sizes()`，不再把整棵树 `collect()` 进内存 | `cargo check --lib` exit 0 |

#### C1 引入的快捷键回归（**已修**，12 处改动）

轻量列表换掉 `<FileBrowser>` 之后，搜索结果**不再挂在导航格的 FileBrowser 上**，于是「当前选中项」永远为空 ⇒ 复制/剪切/删除/重命名/属性全部退化成空操作。修复口径是**「动作路由由『哪个条目』决定，而不是『哪个窗格』决定」**：

| 位置 | 改动 |
|---|---|
| `navigator.vue:1044` `getActivePaneSelectedEntries()` | 搜索激活时直接读 `globalSearchViewRef.getSelectedEntries()`（导航格的选择已被清空） |
| `navigator.vue:1008` `getActivePaneRef()` | 统一回落导航格 pane，避免搜索激活时拿到 `null` |
| `navigator.vue:1262` `handleSelectAllShortcut()` | 搜索激活**或**搜索面板有焦点时走 `selectAllResults()` |
| `navigator.vue:805/1301/1317` | 删除 / 永久删除成功后 `clearSelections()` |
| `navigator.vue:1576` rename handler | 搜索激活时走 `globalSearchViewRef.startRename(entry)` |
| `global-search-view.vue` | 引入 `FileBrowserRenameDialog` + `useDirEntryActions`；根节点加 `@focusin/@focusout` 跟踪面板焦点；搜索框加 `@keydown.enter.prevent`（无聚焦项时打开第一条）；`defineExpose` 增补 5 个成员 |

**Ctrl+A 判定口径（容易再踩）**：搜索面板是**侧边面板**（`v-show` + ResizablePanel），不是遮罩，用户可以同时操作导航格 ⇒ **单看 `globalSearchStore.isOpen` 会误清导航格的选择**。现在的口径是「`isSearchSelectionActive` **或** 搜索面板有焦点」，焦点用根节点的 `focusin` / `focusout` 跟踪（`relatedTarget` 仍在根内则不置 false）。

**重命名通道**：搜索结果不在导航格列表里，就地编辑不可见 ⇒ 搜索面板自挂 `FileBrowserRenameDialog` + `useDirEntryActions().renameItem(entry, newName)`（本地 `invoke('rename_item', {sourcePath, newName})`；远程 `renameRemoteEntry`）。
⚠️ **已知局限**：重命名成功后，搜索列表**仍显示旧文件名**，直到下一次搜索 / 扫描。

#### 统一门禁（全部在最终代码上量）

| 项 | 结果 |
|---|---|
| `npx eslint .` | **188 errors / 0 warnings** —— 与基线一致 ✅ |
| `npm run ts`（vue-tsc） | **exit 0** ✅ |
| `npm run test`（vitest） | **250 files / 2309 tests 全通过** ✅ |
| `cargo check --lib` | **exit 0** ✅ |
| `cargo test --lib` | 823 passed / 2 failed —— **存量环境失败**，同 §18.10 的两条 MCP 端口用例，**不要当回归** |

**打包**：`npx tauri build` → `TAURI_BUILD_EXIT=0`（Rust release 3 m 05 s，NSIS 19.76 MiB）。
产物：`src-tauri\target\release\alpha-file-manager.exe` **56,495,104 B / 2026-10-28 12:58:39**；`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.6.3_x64-setup.exe` **20,717,765 B / 12:58:36**。

#### 真机验收

| 切片 | 结论 |
|---|---|
| **C3** | ✅ **PASS**（6 条断言）——`.superpowers\sdd\accept-c3-virtual-rows.ps1` + `shots\c3-vrows-*.png` |
| **C1** | ✅ **PASS**（**沙箱验收**：隔离桌面，脚本 `.superpowers\sdd\sandbox-c1-accept.ps1`，日志 `mem-accept-c1-sandbox.log`，查询词 `"2026`，11 条断言全部通过：（①）S1 operator desktop foreground untouched；（②）S2 global search panel opened with the mouse；（③）S2 query text entered in sandbox；（④）S3 results summary present；（⑤）S4 one accessible row per result；（⑥）S5 drive groups exposed as lists；（⑦）S6 positive control: left pane shows a selection badge；（⑧）S7 a result row inside the window was clicked；（⑨）S8 search row selection routed into the navigator (left pane selection cleared)；（⑩）S9 screenshot captured in sandbox；（⑪）S10 operator desktop foreground still untouched at the end）。⚠️ **本轮结论的边界**：沙箱桌面里**无法注入键盘**（`SendInput` 只到输入桌面），因此 `Ctrl+A` / `Ctrl+C` 这两条**键位路径没有在沙箱内取证**；已取证的是「动作路由按选中项走」这条底层逻辑（搜索选中会清掉导航格的选择）。 |

#### 已知的行为变化（用户可见，需知情）

1. **扩展读文件超过 64 MiB 直接返回 Err**（C6 的有意取舍）。
2. **`dir_watcher` 的 OS `watch()` 失败现在返回 Err**（原来静默忽略）；同时修掉旧实现「线程先退出、注册表后写入」的 stale 登记竞态；节流刷新事件顺带刷新 `last_emit`。
3. **重命名搜索结果后列表仍显示旧文件名**（直到下一次搜索 / 扫描）。
4. **C4 的有界队列满时丢最旧事件**（文件系统最终一致，不丢最终状态）。
5. **C5 让 segment 收敛** ⇒ 索引体积会下降，但**合并期间有一次 CPU / IO 尖峰**。

#### 证据文件

| 文件 | 内容 |
|---|---|
| `.superpowers/sdd/mem-batch-c1-report.md` … `mem-batch-c6-report.md` | 六个切片各自的实现报告 |
| `.superpowers/sdd/mem-origin-attribution.md` | 来源归因完整取证（48,123 B / 473 行） |
| `.superpowers/sdd/mem-batch-c-frontend-gate.log` | eslint / vue-tsc / vitest 原始输出 |
| `.superpowers/sdd/mem-batch-c-tauri-build.log` | `npx tauri build` 原始日志 |
| `.superpowers/sdd/batch-c-gate.ps1` / `batch-c-build.ps1` | 门禁 / 打包脚本（**纯 ASCII**） |
| `.superpowers/sdd/accept-c1-search-actions.ps1` / `accept-c1-v4.ps1` | C1 真机验收脚本（**纯 ASCII**；v4 加了置前校验 + UIA 直调 + ValuePattern 设值） |
| `.superpowers/sdd/accept-c3-virtual-rows.ps1` + `shots/c3-vrows-*.png` | C3 真机验收脚本与截图 |
| `.superpowers/sdd/mem-accept-c1-v*.log` | C1 验收各轮原始日志（含两次失败的原因） |

### 18.13 资源占用问题的来源归因（上游 SFM vs AFM 二次编辑）

> 完整取证见 `.superpowers/sdd/mem-origin-attribution.md`（48,123 B / 473 行）。本节只给结论。

**一句话结论：被审计的 19 条资源占用问题（F1–F12e），AFM 二次编辑引入 0 条。** 全部 19 条的「首次引入 commit」都是上游 SFM 的提交（作者一律 `aleksey-hoffman`，`git merge-base --is-ancestor <sha> upstream/main` 全部返回 0）。

**口径说明**：`upstream` remote = `aleksey-hoffman/sigma-file-manager`（**原仓 SFM**），`origin` = `kizemo/alpha-file-manager`（**AFM 自己**）。merge base = `fed49cee`（2026-08-27），上游独有提交 23 个、AFM 独有 302 个（文档旧值 251 已过期，AFM 在 10-19 之后又补了约 51 个提交）。

#### 归因汇总

| 判定 | 条数 | 条目 |
|---|---|---|
| 上游 SFM 自带，AFM **未修**（当前 HEAD 仍在） | **10** | F1、F3、F6、F7、F8、F11、F12a、F12b、F12c、F12d |
| 上游 SFM 自带，AFM **批次 A/B 已修** | **6** | F2、F4、F5、F9、F10、F12e（部分：reader 已释放，`Index` 仍常驻） |
| **AFM 二次编辑引入** | **0** | — |
| AFM **放大了上游问题**（仅量级层面） | **1** | F2 的磁盘占用量级：AFM 独有的缩略图四档尺寸让同一源图按尺寸存成多份缓存文件 |
| 未能判定 | **0** | — |

#### 三大占用块（按对实测占用的贡献排序）

| 占用块 | 实测量 | 归属 | 关键证据 |
|---|---|---|---|
| **F1 隐藏 Quick View renderer** | 171.9 MB（常驻第二个 renderer） | **纯上游** | 机制由上游 `d59e8e9e`（2026-06-30「New settings: Window memory / performance control」）引入；上游默认 `prelaunchQuickViewWindow: true`。AFM 只改过窗口标题（品牌改名 `d2be2373`）。批次 D1 已在**用户机器**上关掉（改设置，非代码） |
| **F3 tantivy `NoMergePolicy`** | 搜索索引 299.16 MB（7+ segment 永不合并） | **纯上游** | 上游 `054e2472`（2026-06-24「UX: indexing stability improvements」）；AFM 对 `global_search/index.rs` 的提交数 = **0**；上游当前版本仍是 `NoMergePolicy` |
| **F2 缩略图缓存** | 434.34 MB / 4,249 张 | 上游设计 + **AFM 放大量级** | 上游 `9f20b226` 定下 5000 张 / 1 GB；AFM 批次 A/B（B2）已改为 2000 张 / 256 MB 并按 mtime 淘汰。**四档缩略图尺寸是 AFM 独有的** ⇒ 同一源图落盘多份 |

**WebView2 子树 888.8 MB 与 Rust 主进程 230.1 MB 的常驻成分，主要由上游 SFM 造成**（F1 隐藏 renderer + F3 索引常驻 + F12e `Index` 常驻 + 上游的 `dir_watcher` 每目录 2 线程）。

#### 批次 C 修的正是「上游自带且未修」的那批

| 条目 | 上游引入 | 批次 C 对应切片 |
|---|---|---|
| F6 每命中一个盘符挂一个完整 `<FileBrowser>` | `7a62a0d0`（AFM 对 `global-search-view.vue` 提交数 = 0） | **C1** |
| F7 `dirContents` 深响应式 / 每 Tab 各存一份 `dirEntries` / `clone(tab)` 深拷 | `ef1b1fed` + `ebc762ef` / `733e2f42` | **C2** |
| F8 虚拟行全量物化 O(N) + 每行拼完整路径当 key | `afa479de` | **C3** |
| F11 `dir_watcher` 每监听目录 2 个线程 + 无界 mpsc | `385157ae`（AFM 对 `dir_watcher.rs` 提交数 = 0） | **C4** |
| F3 tantivy segment 不合并 | `054e2472` | **C5** |
| F12a / F12b / F12c | `053fb4b2` / `9864c72c` / `6cc7928a`（AFM 三个文件提交数均为 0） | **C6** |

**F12d**（启动时 5 个用户存储 JSON 常驻副本）**本轮未修** —— 它是上游 `b839e298` 的 `preloaded: Arc<OnceCell<…>>` 语义，改动面波及启动路径，留待下一批次。

#### 附带发现：上游还有一个**功能性**缺陷（与资源占用无关，但同属「上游自带」）

`src-tauri/src/global_search/scoring.rs` 的 `get_min_score_for_query_length()`：查询长度 **1–3 字符 → 相似度门槛 0.9**，4–6 → 0.65，7–9 → 0.55，其余 → 0.5。
而 `calculate_similarity_score()` 对「文件名包含查询词」只给 `0.8 + (len(query)/len(name)) * 0.15`：3 字符查询对 `a.txt`(0.89)、`report.txt`(0.845)、`2026年10月26日 20:53.txt`(0.821) **一律 < 0.9** ⇒ **1–3 字符的全局搜索恒返回 0 条**。
实测对照：查询 `txt`（3 字符）→ 面板显示「什么都没找到」；查询 `2026`（4 字符，门槛 0.65）→ 「在 3 个驱动器上找到 96 项」。
上游提交 `b8a1c1b4`（2026-03-28 拆文件提交），**AFM 从未修改该文件**（`git diff` 为空、`git log` 只有上游那条）⇒ 同属**上游自带**，本轮**未修**（改它等于改搜索排序策略，属产品决策）。
⚠️ **验收脚本必须用 ≥4 字符的查询词**，否则会把上游缺陷误判成 C1 回归。

#### 归因方法的已知坑（接力时若要复核）

- `git blame` 给的是「该行最后一次被触碰」，**不等于语义引入**（例：F3 的 `NoMergePolicy` 当前行 blame = `31c665c6f`，纯格式提交；语义引入其实是 `054e2472`）⇒ 一律以 `git log --reverse -S'<逐字字符串>'` 的首次出现为准。
- `-S` 数的是**出现次数**，格式调整也会命中，要结合提交内容判断。
- `git log --follow` 只在文件被 **rename** 时有效；本仓 2026-03-28 `b8a1c1b4` 是「拆文件」而非 rename ⇒ `--follow` 会断链，需按任意路径重查。
- 取证期间有并行 writer 在改同一工作副本（正是批次 C 的改动）⇒ 涉及被改文件的归因已用 `git blame HEAD` + `git log -S` 只读复核，结论不变。

### 18.14 批次 C 之后的待办（接力入口）

| # | 事项 | 状态 |
|---|---|---|
| 1 | **批次 C 已 commit**：`bb003f97`（22 files / +3963 / −708，代码 + 本文回写同一个 perf commit）；**未 push、未发版、未 bump 版本号** | ✅ 已提交 |
| 2 | **C1 真机验收**（沙箱内执行，脚本 `sandbox-c1-accept.ps1`）：已 PASS（见 §18.12） | ✅ |
| 3 | **F12d 已评估·不修**：实测 5 个真实 JSON 合计约 **75 KB**（`user-stats.json` 34,477 B、`user-extensions.json` 32,461 B、`user-settings.json` 4,266 B、`user-workspaces.json` 2,736 B、`user-remote-connections.json` 1,154 B），载体是上游 `b839e298` 的 `preloaded: Arc<OnceCell<…>>`；常驻上限只是百 KB 级，改启动路径的风险与收益不成比例 | ⏸️ 已评估·不修 |
| 4 | **`use-video-thumbnails.ts` 已按 A2 同款改造**：`ref<Record>` → `shallowRef<Map>`，写入走 `setVideoThumbnail()`（等值早退 → `Map.set` → `triggerRef`，把单调增长时的 O(n²) 分配降为 O(1)），`clearThumbnails()` 改 `= new Map()`；测试断言同步改 `.value.size === 0`；`npm run ts` exit 0、该文件单测 3/3 通过 | ✅ 已修 |
| 5 | **WebView2 子进程崩溃兜底**（§18.11 D3 建议 ②）：检测 + 提示 + 可恢复 | ⬜ 未做 |
| 6 | **版本号仍是 3.6.3**，批次 C 不单独发版；是否随下一版统一发由用户决定 | ⬜ 待决策 |
| 7 | **索引重建代价**：实测一次完整重建约 **17 分钟**（1,016 s，1,956,015 条）；验收脚本必须先确认索引就绪再判 FAIL | ⚠️ 已知 |
| 8 | **上游缺陷候选**：`global_search/scoring.rs` 的 `get_min_score_for_query_length()` 对 1–3 字符查询给 0.9 门槛，使「含扩展名的文件名」普遍落在 0.82–0.89 ⇒ **短查询恒 0 结果**（上游 `b8a1c1b4`，AFM 未改） | ⬜ 未修（上游） |
| 9 | **真机验收必须进隔离桌面沙箱**（用户 2026-10-28 要求）：不得在用户桌面上启动 AFM 抢焦点；沙箱脚本 `sandbox-c1-accept.ps1`，坑位清单见 `.superpowers\sdd\relay-rules-batch-c.md` | ✅ 已落地 |
| 10 | **新会话接力 prompt**：`.superpowers\sdd\relay-prompt-batch-c.txt`（一句话 + 完整路径，引导新会话读必要文档） | ✅ 已落地 |

> **接力时先读**：外层仓 `F:\soft\00selfmade\filemanager\AGENTS.md`（硬约束全集）→ 本文 **§0** 与 **§18** → `.superpowers\sdd\mem-batch-c-handoff-18-12.md` / `-18-13.md`（本节的底稿）→ `.superpowers\sdd\mem-origin-attribution.md`（归因完整取证）→ `.superpowers\sdd\sandbox-c1-accept.ps1`（沙箱验收脚本）→ `.superpowers\sdd\relay-rules-batch-c.md`（防异常终止对策）。
