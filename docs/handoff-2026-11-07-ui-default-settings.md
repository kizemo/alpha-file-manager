# 交接卡 · 2026-11-07 「视觉效果 / 样式设置」默认指标重置为附图 + 上一轮收尾

> 新会话请先读本文件，再按需读：
>
> - `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-07-reveal-shortcut-target.md`
>   （上一轮交接卡：未提交改动清单、门禁数字、发版待办、踩坑清单）
> - `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\work-2026-11-07-reveal-shortcut-target.md`
>   （上一轮完整排查记录：§10 根因、§11 两个分支的真机验收）
> - `F:\soft\00selfmade\filemanager\AGENTS.md`（项目硬约束 + 踩坑沉淀，**改代码前必读**）
>
> 本卡目标：把附图里的「视觉效果 / 样式设置」指标变成**应用默认设置**（点「重置为默认」即回到这组值，新装软件首次启动也用这组值），
> 并把上一轮遗留的收尾（eslint 基线对照 / 发版决定 / 提交）一并做掉。

---

## 1. 背景

- 内层仓 `F:\soft\00selfmade\filemanager\sigma-file-manager`（Alpha File Manager / Sigma File Manager），基线 **3.12.1** 已发版。
- 外层仓 `F:\soft\00selfmade\filemanager` 只放 `docs/`、`AGENTS.md`、`.superpowers/`。
- **上一轮任务（「打开文件所在目录」对快捷方式失效）已闭环**：代码、门禁、打包、两个分支的真机验收全部通过，**不要重做**。
  但改动**未提交**：HEAD 仍是 `b49fbd31`，11 个文件处于 modified 状态（清单见上一轮交接卡 §8）。
- 用户 2026-11-07 20:13 追加了本卡的新需求，并明确「新开会话继续执行」。

## 2. 本轮目标

### 2.1 新需求（UI 默认设置）

把附图（**规格图，不是现状截图**）里的「视觉效果 / 样式设置」指标设为**应用的默认设置**：

1. 设置页点「重置为默认」按钮 ⇒ 回到附图这组值；
2. **新装软件**首次启动 ⇒ 直接就是附图这组值。

附图原件（规格来源，已另存一份到外层仓）：

- 原始附件：`E:\Users\Duanyi\.minimax\.minimax\v2\assets\2026\11\07\20-13-10-047-asset_20261107-201310-047_76dd8c2a3482_c5edb52f-image.png`
- 副本（**下一会话优先读这份**，避免附件目录轮转后找不到）：
  `F:\soft\00selfmade\filemanager\.superpowers\sdd\ui-defaults-spec-20261107.png`

### 2.2 上一轮未完成任务（本轮一并收尾）

3. `eslint .` 全量基线对照（**用 `git stash push -u` 量 HEAD**，基线数字随提交变）。
4. 发版决定：建议 **3.12.2** 补丁版；发版前把 `package.json` / `src-tauri/Cargo.toml` / `tauri.conf.json` 三处版本号对齐。
5. 提交上一轮 11 个文件的改动（**用户未明确要求前不要擅自提交**）。

## 3. 目标值 vs 当前默认值（逐项核对到代码行）

默认值全部在 `src/stores/storage/user-settings.ts`；本页对照**已逐条 grep 核实**。

### 3.1 视觉效果（`settings.visualEffects.*`，见 `src/localization/messages/ch.json:314-351`）

| 附图项 | 存储键 | 当前默认 | 附图目标 | 是否要改 |
|---|---|---|---|---|
| 视觉效果（总开关） | `infusion.enabled` | `true` | 开 | 否 |
| 对所有页面使用相同的设置 | `infusion.sameSettingsForAllPages` | `true` | 开 | 否 |
| 应用空闲或最小化时暂停背景视频 | `infusion.pauseVideoWhenIdle` | `true` | 开 | 否 |
| 应用空闲或最小化时暂停横幅视频 | `homeBannerPauseVideoWhenIdle` | `true` | 开 | 否 |
| 叠加模糊 | `infusion.pages.<page>.blur` | **64**（通用页）/ **32**（home 页） | **0px** | **是** |
| 叠加不透明度 | `.opacity` | **5** | **0%** | **是** |
| 叠加媒体对比度 | `.mediaContrast` | 100 | 100% | 否 |
| 叠加媒体亮度 | `.mediaBrightness` | 100 | 100% | 否 |
| 叠加噪点 | `.noise` | **5** | **0%** | **是** |
| 叠加噪点缩放 | `.noiseScale` | 0.5 | 0.5 | 否 |
| 叠加混合模式 | `.mixBlendMode` | `'normal'` | 正常 | 否 |
| 混合对象 | `.blendTarget` | `'ui'` | 界面 | 否 |
| 混合填充色 | `.blendTint` | `'#3b82f6'` | 附图未显示（混合对象=界面时不生效） | 否（存疑，见 §7） |
| 叠加背景 | `.background` | 内置背景媒体 | 附图显示「后台管理器 + 刷新」 | 否（能力问题，非数值） |

### 3.2 样式设置（`settings.visualFilters.*`，`ch.json:368-372`）

| 附图项 | 存储键 | 当前默认 | 附图目标 | 是否要改 |
|---|---|---|---|---|
| 亮度 | `visualFilters.brightness` | 100 | 100% | 否 |
| 对比度 | `visualFilters.contrast` | 100 | 100% | 否 |
| 对话框背景模糊 | `visualFilters.dialogOverlayBlur` | 8 | 8px | 否 |

**⇒ 结论：真正要改的只有 6 个数值** —— `blur`（64/32 → 0）、`opacity`（5 → 0）、`noise`（5 → 0），
分别在「通用页」和「home 页」两套默认里各改一次。

## 4. 改哪里（精确指针）

| 位置 | 现状 | 动作 |
|---|---|---|
| `src/stores/storage/user-settings.ts:308-323` `createDefaultInfusionPageSettings()` | `blur: 64` / `opacity: 5` / `noise: 5` | 改成 `blur: 0` / `opacity: 0` / `noise: 0` |
| `src/stores/storage/user-settings.ts:325-340` `createDefaultInfusionPageSettingsHome()` | `blur: 32` / `opacity: 5` / `noise: 5` | 改成 `blur: 0` / `opacity: 0` / `noise: 0` |
| `src/stores/storage/user-settings.ts:342-348` `createDefaultVisualFiltersSettings()` | brightness 100 / contrast 100 / dialogOverlayBlur 8 | **不动**（已与附图一致） |
| `src/stores/storage/user-settings.ts:237`、`:249-253` | `homeBannerPauseVideoWhenIdle: true`、`infusion.enabled/sameSettingsForAllPages/pauseVideoWhenIdle: true` | **不动**（已一致） |
| `src/utils/infusion-blend.ts:24/32/34` | `DEFAULT_INFUSION_BLEND_MODE='normal'`、`DEFAULT_INFUSION_BLEND_TARGET='ui'`、`DEFAULT_INFUSION_BLEND_TINT='#3b82f6'` | **不动** |
| `src/modules/settings/ui/categories/appearance/visual-effects.vue:321` `resetVisualEffectsSection()` | 克隆 `userSettingsStore.userSettingsDefault.infusion` 回写 storage | **不需要改**：默认改了，重置自然回到新值 |
| `src/modules/settings/ui/categories/appearance/visual-filters.vue:87` | 样式设置的重置按钮（复用 `settings.visualEffects.resetToDefaults` 文案） | 不需要改 |
| `src/localization/messages/*.json`（17 个语言包，**中文 locale code 是 `ch`**） | 文案已齐 | **不需要改** |
| `src-tauri/src/**` | 对 `infusion` **零命中**（已 grep 验证） | **没有第二份默认值**，改前端一处即可同时满足「重置为默认」与「新装软件」 |

**开工第一件事**：确认 `userSettingsStore.userSettingsDefault` 就是上面这些 `createDefault*` 的产物
（grep `userSettingsDefault` 即可），确认后改动即自动贯通「默认 / 重置 / 新装」三条路径。

## 5. 验收口径

- 门禁：`npx vitest run`（受影响的测试文件）+ ESLint 单检 0 问题；**若动到 Rust** 才需要 `cargo test --lib`。
  注意 `src/stores/storage/__tests__/user-settings.test.ts` 与 `src/stores/schemas/__tests__/user-settings.test.ts`
  里可能有默认值断言（如 `blendTarget`），改默认值后要同步断言。
- 真机：**必须用 `tauri build` 的打包产物**（`tauri dev` 的路由页内容区全空白 ⇒ dev 上的界面结论一律作废）。
- 验收点：
  1. 设置页点「重置为默认」⇒ 逐项读回附图这组值；
  2. **全新用户数据目录**首次启动 ⇒ 默认值即附图这组值；
  3. 顺手确认存量安装点「重置为默认」也能拿到新值（存量用户默认仍是旧值，见 §7）。

## 6. 上一轮遗留（与本轮一并做）

| 项 | 现状 |
|---|---|
| `eslint .` 全量基线 | **未跑**。基线必须用 `git stash push -u` 量 HEAD；已知 2026-10-23 基线 188 errors / 0 warnings（基线随提交变） |
| 发版 | **未做**。建议 3.12.2；三处版本号对齐（历史上踩过 `.cmd` 里 `>` 把版本顶掉的坑，见 `AGENTS.md`） |
| 提交 | **未提交**。11 个文件：见上一轮交接卡 §8 |
| 新包是否装机 | 2026-11-07 18:00 的 NSIS 包**已装在** `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，包含上一轮全部代码改动（临时探针只在 `#[cfg(test)]` 里，不影响 release 二进制） |

## 7. 风险 / 待用户决策

1. **存量用户不会自动变成新默认值**：只有「新装」和「点重置为默认」才拿到附图这组值。
   这**正是用户要求的口径**（用户原话只提这两条）；若还想让存量用户自动变，需要加一次版本化迁移
   （可参考 `changelog.lastSeenVersion` 机制）——**要不要做，属于范围决策**，别擅自扩大。
2. **`blendTint` 在附图里看不到**（因为混合对象=界面）。默认 `'#3b82f6'` 保持不动；若用户后续把混合对象切到「填充颜色」，
   才会露出该色值，届时需要再确认目标色。
3. **默认值改 0 会改变观感**：叠加模糊 64→0、不透明度 5→0、噪点 5→0 ⇒ 叠层背景从「明显毛玻璃」变成「基本透明」。
   这是附图明确要求的效果，但**打包后建议让用户看一眼实机观感**。
4. 改完默认值**不要**顺手改文案；17 个语言包的中文 locale 是 `ch` 不是 `zh`。

## 8. 已验证 / 推断 / 未验证

**已验证（本轮 grep 到具体文件与行号）**

- 附图各项与存储键、文案键的对应关系；全部默认值当前取值。
- `resetVisualEffectsSection()` 的实现路径（克隆 `userSettingsDefault` 回写）⇒ 改默认值即自动贯通「重置为默认」。
- `src-tauri/src` 对 `infusion` 零命中 ⇒ 无第二份默认值。

**推断（下一会话开工即确认）**

- `userSettingsStore.userSettingsDefault` == 上述 `createDefault*` 的产物。

**未验证**

- 新装首次启动的实际取值路径（需要真机用干净用户数据目录跑一次）。
- 改完默认值后的实机观感是否符合附图。

## 9. 本轮踩的坑（提醒，不必重复踩）

1. **真机验收只能用 `tauri build` 产物**；`tauri dev` 的路由页内容区全空白（`RouterView` 挂载但不渲染子组件），
   dev 上的界面结论一律作废。
2. **需要装机实测就直接打包 exe**，别等用户开口；`tauri build` 很慢 ⇒ 判断出要验就**早启动、丢后台**，
   长编译在 `.cmd` 内部用 `> logfile 2>&1` 重定向到文件，**不要挂 stdout 管道**（管道一断进程树跟着死）。
3. **`.cmd` 纯 ASCII**（cmd.exe 按 ANSI 读，中文注释会变乱码并被当命令执行）；**`.ps1` 同样纯 ASCII**（中文用码点拼）。
4. **绝不用 PowerShell `Get-Content -Raw` / `Set-Content` 改带中文的文件** ⇒ 用编辑工具或 node `fs.*Sync(...,'utf8')`。
5. **验收断言别只取「最后一条」日志**；**改完公式/像素零差异时先怀疑「路径没走到」**。
6. **`vitest --reporter=basic` 在本仓库不存在**；`$ErrorActionPreference='Stop'` 会把原生命令的 stderr 变成终止错误。
7. `cargo test --lib` 红线：被测试链到的代码不得触碰 `tauri::AppHandle` / `tauri::async_runtime::*`。
   本轮若不碰 Rust 就无需重跑。

## 10. 一句话 prompt（给新会话）

> 读 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-07-ui-default-settings.md`
> 及其「必读文档」清单（含上一轮交接卡、排查记录、`F:\soft\00selfmade\filemanager\AGENTS.md`），
> 接手「UI 默认设置」这批工作：把附图规格（副本在
> `F:\soft\00selfmade\filemanager\.superpowers\sdd\ui-defaults-spec-20261107.png`）里的
> 「视觉效果 / 样式设置」指标做成应用默认设置，使「重置为默认」与「新装软件」都回到这组值，
> 并按卡内清单完成上一轮遗留的 eslint 基线对照、发版决定与提交收尾。
