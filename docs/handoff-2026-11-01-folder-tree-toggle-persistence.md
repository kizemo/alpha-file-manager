# Handoff · Alpha File Manager 目录树开关持久化修复 + 下一阶段（2026-11-01）

> 本文档是**本轮（3.9.0 发版收尾 → 主题截图 → winget 提 PR）的交接终点**，也是**下一会话的作业书**。
> 下一会话请按第 2 节的顺序读文档，读完后用 10 行以内复述背景/目标/进度/下一步，再动手。
>
> 本文档中的所有路径都是**绝对路径**，可直接复制使用。

---

## 0. 进度快照（读完本文档后应能复述的内容）

| 项 | 值 |
|---|---|
| 仓库（代码在**内层**） | `F:\soft\00selfmade\filemanager\sigma-file-manager` |
| 仓库 remote | `kizemo/alpha-file-manager`（fork，parent = `aleksey-hoffman/sigma-file-manager`） |
| 外层目录 | `F:\soft\00selfmade\filemanager` —— 只放 `AGENTS.md` 与历史档，remote 是 `kizemo/focus-sync`，**不跟踪内层代码，不要动** |
| 当前版本号 | **3.9.0**（`package.json` 与 `src-tauri\Cargo.toml` 一致；`tauri.conf.json` 的 version 走 `"../package.json"`） |
| **线上已发布版本** | **3.9.0** —— 站点 `aiec.fun` 与 GitHub release `v3.9.0` 均已生效 |
| 线上 3.9.0 安装包 | `Alpha_File_Manager_3.9.0_x64-setup.exe`，44,776,031 B，sha256 `EC0EDA4D305A0CD2A4E1E002DDA045DF9D3BDFF0D31D1388AF010F81535FEB05` |
| 已安装版 | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`，FileVersion **3.9.0** |
| 本轮结束时 HEAD | `8a8da593`（`fix(navigator): 修复目录树开关跨重启失效`）—— **本地提交、刻意未 push**；`origin/main` 仍停在 `62eab9cb` |
| 本轮已推送的 4 个提交 | `d4e844a2`（feat ui 合并）→ `d9dc92a1`（feat ui 3.9.0 功能合集）→ `a9e7f8a9`（chore release 3.9.0）→ `62eab9cb`（docs readme 主题截图） |
| **待推送的修复提交** | `8a8da593` —— 2 文件 +211 行（`src/stores/storage/user-settings.ts` +7 / 新增单测 204 行）；推不推由下一会话按 **T1 真机验收结果**决定 |
| winget PR | **#467381 open** —— https://github.com/microsoft/winget-pkgs/pull/467381 ，等社区版主 `Moderator-Approved` |
| 真实用户数据 | **已恢复并核对 sha256**（三文件 MATCH_REAL），当前磁盘上是真实数据，不是夹具 |

### 0.1 本轮（r18）做完的六件事

1. **发版 3.9.0 闭环**：六条发版命令全绿，站点 + GitHub release `v3.9.0` 生效，14 张截图 + 双语 README 图注 + release notes + CHANGELOG 中英条目全部落地。
2. **主题 UI 介绍与双语截图**：「外观与体验 / Look and feel」新增「多种美化的自定义主题自由切换」说明；新增 8 张主题截图（设置页主题下拉展开 + 跃动活力 / 冷杉 / 暖纸·深色三主题，中英各一套）。`README.md` 用 `-en`，`README.zh-CN.md` 用 `-zh`。
3. **winget 三步代劳**：关闭旧 PR #460744（被 3.9.0 取代）→ 以 New package 形式开出 **#467381** → PR 正文末尾的 CodeFlow 链接从 `pull/460744` 改成 `pull/467381`。
4. **真实用户数据恢复**：杀进程 → restore → `user-settings.json` / `user-stats.json` / `user-workspaces.json` 三项 sha256 与备份**逐字节一致**。
5. **清理旧演示目录副本**：`C:\Users\Duanyi\afm-demo-390`（26 个文件 / 3,186,749 B）已**可恢复地**移入回收站；主用目录 `C:\Users\Public\afm-demo-390`（26 个文件）完好。
6. **修复「目录树开关跨重启失效」**：默认设置对象补 `navigator.showFolderTree: true`（+7 行，含中文 FORK-MODIFICATION 注释）+ 新增判别性单测 5 条（204 行）⇒ 本地提交 `8a8da593`，**未 push**（等 T1 真机验收）。

---

## 1. 已锁定的结论（本轮不可推翻）

### 1.1 3.9.0 已对外发布 —— 下一版**不能**用 `--bump none` 重发

理由与 3.8.0→3.9.0 那次同构：线上已有 `v3.9.0` 且安装包 sha256 = `EC0EDA4D…`，本地再打一个同版本号的包会**覆盖线上同版本**（sha256 变、版本号不变）⇒ 用户本地校验与更新提示行为全乱（`app_updater` 用 sha256 比对）。

⇒ 下一个版本走 **3.9.1（patch）**，仅在目录树修复确认要对外发布时才发。

### 1.2 目录树开关缺陷：根因、修复面、边界

**现象**：导航器工具栏的「目录树」开关关掉后，**重启应用又变回显示目录树**。

**根因（已逐行核实）**：

- `src/stores/storage/user-settings.ts:88` 起的 `const userSettings = ref<UserSettings>({ ... })` 是**默认设置对象**；它的 `navigator` 段**没有** `showFolderTree` 键。
- `initUserSettings()`（同文件 517-535）：`userSettingsDefault.value = cloneDeep(userSettings.value)`（524-525）→ `allowedUserSettingsStorageKeys.value = buildAllowedUserSettingsStorageKeys(userSettingsDefault.value)`（528-529）。
- `buildAllowedUserSettingsStorageKeys`（`src/stores/schemas/user-settings.ts:46-48`）= `collectNestedRecordPaths(默认对象)`；`collectNestedRecordPaths`（`src/stores/schemas/schema-utils.ts:92-110`）**把叶子键也收进集合**（`keys.add(nextPath)` 在递归之前）⇒ **允许键集合是从默认对象自动派生的**。
- `applyUserSettingsEntries`（`src/stores/storage/user-settings.ts:447-463`）：不在允许键集合里的键直接 `continue`，**静默丢弃** ⇒ 磁盘上真实存在的 `navigator.showFolderTree: false` 在启动加载时被丢掉。
- **写侧不过滤**：`setUserSettingsStorage`（537-547）无条件写盘 ⇒ 文件里确实有值，只是读不回来。
- 运行时逻辑是对的、**不该改**：`src/modules/navigator/pages/navigator.vue:511-512` 读 `userSettingsStore.userSettings.navigator.showFolderTree ?? true`；`:581-583` 通过 `set('navigator.showFolderTree', next)` 写回。
- `src/types/user-settings.ts:442` 已声明 `showFolderTree?: boolean;`（类型在，缺的只是默认对象里的键）。
- 同文件 98-103 行有既有 FORK-MODIFICATION 注释，写的正是这条规则：「这个字段必须出现在**默认对象**里：允许键集合（buildAllowedUserSettingsStorageKeys）是从默认对象自动派生的，默认对象里没有 ⇒ 读侧不认、写侧也存不进去。」

**⇒ 修复面**：只改**默认对象**（加 `showFolderTree: true`），**不动** `src/modules/navigator/**` 的运行时逻辑、**不动** schema 版本号（`USER_SETTINGS_SCHEMA_VERSION = 33`）、**不动**迁移链。

**兼容语义**：默认值 `true` 与 `navigator.vue` 的 `?? true` 兜底一致 ⇒ 从未用过该开关的用户行为不变；已经关过的用户，磁盘上的 `false` 从此被正常读回。

**修复已落地（本地提交 `8a8da593`，未 push）**：

| 文件 | 改动 |
|---|---|
| `src/stores/storage/user-settings.ts:160-166` | 默认对象 `navigator` 段（`showHiddenFiles: false` 之后）新增 `showFolderTree: true` + 6 行中文 FORK-MODIFICATION 注释 |
| `src/stores/storage/__tests__/user-settings-folder-tree.test.ts`（新建，204 行） | 5 条用例，风格照抄同类先例 `user-settings-font-size.test.ts`（LazyStore mock + `storedValues` 往返） |

单测的 5 条断言：默认值 `=== true`（控制项）｜允许键集合含 `'navigator.showFolderTree'`｜**启动快路径**灌入 `false` 后内存仍是 `false`｜**重启慢路径**（`LazyStore.entries()` = 真实 `user-settings.json` 内容）灌入 `false` 后内存仍是 `false`｜写侧落盘 `storedValues.get('navigator.showFolderTree') === false`。

**变异验证（证明单测有牙）**：临时回退那一行修复后重跑 ⇒ **4/5 变红**（`expected undefined to be false`，正是本 bug 的症状），只有写侧那条仍绿 —— 与「写侧本来不过滤」的根因完全自洽；恢复后 5/5 全绿。

### 1.3 winget PR #467381 的当前状态（2026-11-01 03:15Z 快照）

| 项 | 值 |
|---|---|
| PR | https://github.com/microsoft/winget-pkgs/pull/467381 —— `New package: kizemo.AlphaFileManager version 3.9.0` |
| base / head | `master` / `new-package-kizemo-alphafilemanager-3.9.0`（fork `kizemo/winget-pkgs`，分支基于上游最新 master） |
| 清单路径 | `manifests/k/kizemo/AlphaFileManager/3.9.0/` —— 4 个文件（version / installer / locale.en-US=defaultLocale / locale.zh-CN=locale） |
| 标签 | `Validation-Completed` + `Azure-Pipeline-Passed` + `New-Package` + `Needs-CLA`（**后者是残留标签**） |
| CLA | ✅ **已达成** —— `license/cla` check = success，输出 `All CLA requirements met.`（用户已在 GitHub 侧签署） |
| 剩余 | ⏳ 等社区版主打 `Moderator-Approved` → 合并 → 发布流水线 |
| 旧 PR #460744 | 已关闭，closedAt `2026-11-01T02:17:59Z` |

⚠️ **若之后往该分支推任何新 commit**，校验会重跑，CLA 可能再要求一次；那时需要账号本人执行
`gh pr comment 467381 --repo microsoft/winget-pkgs --body "@microsoft-github-policy-service agree"`
（个人独立拥有该作品 IP 时适用；归属雇主走另一条签署路径 —— **立场只能由账号本人决定，agent 不得代选**）。

---

## 2. 必读顺序（按此顺序读，读完再动手）

| # | 路径 | 读它拿什么 |
|---|---|---|
| 1 | `F:\soft\00selfmade\filemanager\AGENTS.md` | **本项目全部硬约束 + 踩坑清单**（运行时注入的版本末尾被截断，需要时直接 `read` 源文件） |
| 2 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-folder-tree-toggle-persistence.md` | 本文档 |
| 3 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md` | **发版六条命令**、五处状态一致性、缓存/回读/回滚细节 |
| 4 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\version-info.md` | 版本号规则与三处版本号同步关系 |
| 5 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\release-notes-3.9.0.md` | 升级说明**中文格式模板**（`### 新增 / ### 修复 / ### 备注`） |
| 6 | `F:\soft\00selfmade\filemanager\sigma-file-manager\CHANGELOG.md` | 更新日志**英文格式模板**（`## [x.y.z] - Month 2026` + `**Summary:**` + `### What changed` + `**Improved:/Changed:/Fixed:**`） |
| 7 | `F:\soft\00selfmade\filemanager\sigma-file-manager\src\stores\storage\user-settings.ts` | 目录树缺陷的根因现场（默认对象 88 行起、`applyUserSettingsEntries` 447 行、`initUserSettings` 517 行） |
| 8 | `F:\soft\00selfmade\filemanager\sigma-file-manager\src\stores\schemas\schema-utils.ts` | `collectNestedRecordPaths`（92-110 行）—— 允许键集合是怎么从默认对象派生的 |
| 9 | `F:\soft\00selfmade\filemanager\sigma-file-manager\src\modules\navigator\pages\navigator.vue` | 目录树开关的运行时读写（511-513 / 581-583 / 1739-1757）——**只读参考，不要改** |
| 10 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\winget\README.md` | winget 清单原文与本项目的 winget 约定 |

**读完后**先在对话里用 10 行以内复述：项目背景、本次任务目标、当前进度（3.9.0 已发布、截图与 README 已推送、winget PR 等审核、目录树修复已落地但未真机验收）、下一步计划，然后开始执行。

---

## 3. 任务清单（下一会话的作业）

### T1 · 真机验收「目录树开关跨重启」【最高优先级，且必须做】

**为什么必须做**：本次修复只动了**默认设置对象**，没有界面改动 ⇒ 单测能证明「加载路径不再丢弃该键」，但**证明不了「用户点一下开关、重启后仍然生效」**这件事真的成立。按项目铁律，**需要真机才能成立的结论不能只交代码 + 门禁数字**。

**验收链路（脚本已验证可用，全部在 `.superpowers\sdd\`）**：

| 脚本 | 作用 |
|---|---|
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r39-shot-run.ps1` | 单场景驱动：kill → 写设置 → 启动 → 最大化 → warmup 20s → 执行动作序列（`click/clickname/clickpopup/key/combo/wheel/shot/uia/sleep`） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r39-theme-set.cjs` | 写 `user-settings.json`（theme / locale / route / `navigator.showFolderTree` / `navigator.folderSettings`） |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r39-state.cjs` | 打印 `[state] theme/tree/locale/route/dir` —— 断言目录树状态的现成探针 |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r39-fixture.cjs` | 夹具 build / restore |
| `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r39-finalize.ps1` | **杀进程 → restore → 三文件 sha256 比对**（收尾一步就跑它） |

**验收步骤**：

1. **必须用 `tauri build` 的打包产物**（或覆盖安装已装版），**不能用 `tauri dev`** —— dev 构建所有路由页内容区全空白，在它上面的界面结论一律作废。
   - 打包命令：`npm run tauri:build`（末步 `publish-bundle-to-release.mjs` **只把安装包复制到仓库 `release/`，不上传**，可放心跑）。
   - `tauri build` 很慢（Rust release + 打包）⇒ **早启动、丢后台跑**；后台跑时**不要挂 stdout 管道**（`Tee-Object` / `| Select-String`）—— 管道一断进程树跟着死。长编译让 `.cmd` 内部用 `> logfile 2>&1` 重定向到文件，再单独读文件看进度。
   - 可复用 `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\build-fb4.cmd`（注意 `.cmd` 必须**纯 ASCII**，中文注释会变乱码并被当命令执行；`.cmd` 的 `echo` 里不能出现 `>`）。
2. **备份真实数据** → 灌夹具 → 验收 → 恢复。**恢复顺序必须是「先杀进程 → 再 restore」**（应用退出时会把内存状态写回真实文件 ⇒ 进程还活着时 restore 完还会被写回一次）。
   - 真实数据目录：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\`
   - **本轮留下的真实数据备份（可直接用）**：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\r39-userdata-backup\`（`user-settings.json` 5,375 B / `user-stats.json` 33,708 B / `user-workspaces.json` 591 B，另有 `manifest.json`）
   - ⚠️ `r39-fixture.cjs backup` 的语义是「**备份文件不存在才建**」⇒ **不会**覆盖旧备份 ⇒ 跑 backup 后一定看它打印的字节数；若明显小于真实值，先 `inspect` 备份文件本身再决定信谁。
3. **正向验收**：启动打包版 → 在导航器页点工具栏「隐藏目录树」按钮（`aria-label` = 中文「隐藏目录树」/ 英文 "Hide folder tree"，见 `navigator-toolbar-actions.vue:171`）→ 确认 `user-settings.json` 里出现 `navigator.showFolderTree: false` → **完全退出应用** → 再启动 → 目录树**仍然隐藏**（UIA 里树面板不再出现，或该按钮的 aria-label 翻成「显示目录树」/ "Show folder tree"）。
4. **阴性对照（必须有）**：先在**修复前**的 3.9.0 已装版上跑同一套步骤，确认「重启后目录树回来」这个现象**确实存在**（本轮已用 3.9.0 实测过该现象，但验收时最好留一份对照截图/日志）。
5. **收尾**：跑 `r39-finalize.ps1`，确认三文件 sha256 全部 `MATCH`，并核对**恢复后磁盘上不再是夹具**。
6. 截图与 UIA 文本落在 `.superpowers\sdd\shots\`，不要把长日志回灌进对话。

### T2 · 推送目录树修复的提交

- 修复提交已在内层仓 `main` 上**本地提交**：**`8a8da593`** `fix(navigator): 修复目录树开关跨重启失效`（2 文件 +211 行；`src/stores/storage/user-settings.ts` +7 行 `showFolderTree: true` + 中文 FORK-MODIFICATION 注释，新增单测 `src/stores/storage/__tests__/user-settings-folder-tree.test.ts` 204 行 5 条用例）。
- ⚠️ **该提交尚未 push**（`main` = `ahead 1`，`origin/main` 仍在 `62eab9cb`）—— **刻意如此**：真机验收（T1）没过就不该把未验收的修复推上去。
- **验收通过后**再 `git push origin main`（推送姿势：**先带默认代理，失败再退回 `git -c http.proxy=` 重试** —— 本机代理 `127.0.0.1:7897` 的 TLS 握手间歇性坏，fetch 与 push 可能走不通的方向相反，别把结论写死）。
- 若 T1 验收**没过**：先修再推，**不要**把没验收过的修复推上去。

### T3 · 下一版发版决策（3.9.1）

- **不要**用 `--bump none` 重发 3.9.0（理由见 1.1）。
- 若目录树修复确认要对外发布 → 走 `--bump patch` 得 **3.9.1**，并按 `release-workflow.md` 走完六条命令（顺序不能乱，`release:prepare --write-notes` 不能省，否则会**安静地发出正文为空的 GitHub release**）。
- `verify-changelog` 必须**早于**升版本号；`CHANGELOG.md` 与 `public\changelog\ch\changelog.md`（中文那一份）**都要补**。
- **发版前需要用户拍板的事**：是否发 3.9.1、是否顺带把 winget 清单补到 3.9.1（现存到 3.6.5）、是否同步 upstream（AFM 与 upstream 已深度分叉，merge base `fed49cee`，判断口径见 `AGENTS.md` §2.2）。

### T4 · winget PR #467381 跟进

- 现在**只等社区版主 `Moderator-Approved`**，不需要用户做任何操作。定期看一眼标签即可（`gh pr view 467381 --repo microsoft/winget-pkgs --json labels,state`）。
- ⚠️ **winget PR 的状态看标签，不看 check runs** —— 校验 App 跑完会把自己的 check run 删掉，「没有 check run」往往意味着**已经跑完了**。
- 合并后需要**另开一个 PR** 补 3.9.1 的清单（同一包 ID 不能并行开两个 open PR；重跑校验必须推非空 commit）。
- contents API 提交时，**相对路径必须以包含 `manifests/` 的那一层为基准**（否则机器人判 "missing the manifests folder"）。

---

## 4. 门禁（同口径对照，别混口径）

```powershell
cd F:\soft\00selfmade\filemanager\sigma-file-manager
npx eslint .                 # 基线用 git stash push -u 量 HEAD 现量，别拿旧数字对
npx vitest run               # 全量单测（默认 reporter；--reporter=basic 在本仓库这版不存在）
npm run ts                   # 类型检查，退出 0
cargo test --lib             # Rust 单测，基线 865 passed / 0 failed
```

**本轮（目录树修复）实测对照** —— 同一台机器、同一命令，基线用干净树现量 HEAD：

| 门禁 | HEAD 基线 | 改动后 | 结论 |
|---|---|---|---|
| `npx eslint .` | 186 errors / 0 warnings | **186 errors / 0 warnings** | 无回归，改动的两个文件均不在违规列表 |
| `npx vitest run` | 2 files failed / **6 failed** / 2409 passed (2415) | 2 files failed / **6 failed** / 2416 passed (2422) | 失败用例**逐条同名同文件一致**，无新增；+7 = 新单测 5 条 + 新增的 2 份 `docs/*.md` 被 `readme-markdown-table.test.ts` 各生成 1 条（该测试按 md 文件数动态生成） |
| `npm run ts` | — | **exit 0** | 通过 |
| `cargo test --lib` | 865 passed / 0 failed | **865 passed / 0 failed** | 一致（本次无 Rust 改动） |

> ⚠️ 上述基线数字是 **2026-11-01 在 `62eab9cb` 上现量的**。下一会话若在此之上又提交了东西，数字会变 ⇒ **一律现量，别拿这行对**。

**基线口径铁律**

- `eslint .` 的基线数字必须用**干净检出**（`git stash push -u` 量 HEAD）拿。eslint **不读 `.gitignore`** ⇒ 工作副本里的 `.superpowers/`（150+ 个一次性脚本）会被一起 lint；已把 `.superpowers/**` 与 `graphify-out/**` 写进 `eslint.config.ts` 的 `globalIgnores`。
- ⚠️ **基线数字会随提交变**，量的时候一律现量。
- 单文件 `eslint --fix` 通过 **≠** 回到基线（`--fix` 会把该文件里基线本来就有的违规一起修掉）。**禁 `eslint --fix` / `stylelint --fix`**。
- `$ErrorActionPreference='Stop'` 会把 npm/npx 的 stderr 变成 `NativeCommandError` ⇒ 调用 npm/npx 前后显式切回 `Continue`。
- `vitest` 红了先分清「真红」还是「超时」：断言失败才有内容可看；`Test timed out in 30000ms` 是另一回事（`single-instance-guard.test.ts` 要枚举全机进程，机器上 431 个进程时 WMI 一次 6.5 秒，42.5 秒 > 30 秒默认超时，脚本本身其实全过）。
- `cargo` 不在 PATH：`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`（目录名里的 **`pc-` 不能省**，写成 `windows-msvc` 会 `cargo metadata` 报 program not found、3 秒内失败）；MSVC 环境用 `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`。

**本轮已知的存量失败**：`npx vitest run` 有 **6 个存量失败用例**（HEAD 上同样失败，不是回归）。逐条对照确认「你的改动没新增失败」即可。

---

## 5. 已知踩坑（别重试）

### 5.1 本轮新沉淀

1. **`collectNestedRecordPaths` 会把叶子键也收进允许集合**（`keys.add(nextPath)` 在递归之前）⇒ 「默认对象里补一个布尔字段」就能解封该设置的持久化，不必改 schema 版本号。
2. **允许键集合是从「默认设置对象」派生的** ⇒ **任何**通过 `set('a.b.c', v)` 写入的设置，只要默认对象里没有对应键，就会在启动加载时被静默丢弃。**写侧不校验 ⇒ 文件里有值、读不回来**，现象极具迷惑性（像「保存了但没生效」）。新增设置项时第一件事就是把它加进 `src/stores/storage/user-settings.ts` 的默认对象。
3. **旧批跑脚本的「主题不生效」结论是错的**：真实原因是脚本自身缺陷/竞态。实测 `afm-warm-light` 正常生效（探针 `bg_4413d0db` 打印 `AFTER-APP theme=afm-warm-light`，读图确认下拉值「暖纸 · 浅色」、页面暖调浅色）。**主题系统本身没有 bug**。
4. **对外公开截图不能带用户真实工作区**：截图前必须把工作区切到 `C:\Users\Public\afm-demo-390`（26 个文件，演示目录），并在 `user-workspaces.json` 里把 directory 型标签 path 一并改写，否则路径里会露出真实用户名。改 `user-settings.json` 的 `navigator.defaultDirectory` **不等于**应用会开那个目录（`user-workspaces.json` 里的当前标签页优先）。
5. **`C:\Users\Public\afm-demo-390` 里没有历史条目**，会回落成列表视图 ⇒ 想拍网格要在 `navigator.folderSettings['c:/users/public/afm-demo-390'] = { layout: 'grid' }`。
6. **本机没有 `winget` 命令行**（App Installer 装了但别名没注册，从 `C:\Program Files\WindowsApps\...` 直接执行报 Access is denied）⇒ 本地 `winget validate` / `winget install --manifest` 跑不了，只能交给上游流水线。**安装包也没有代码签名**（`Get-AuthenticodeSignature` ⇒ `NotSigned`）。
7. **`gh pr edit` 走 GraphQL 需要 `read:org` scope**，当前令牌没有 ⇒ 改用 REST：`gh api -X PATCH repos/microsoft/winget-pkgs/pulls/<n> -F 'body=@<file>'`。
8. **`gh pr view --json` 的字段要写成单字符串**：`--json state,closedAt,url`（一个逗号分隔的字符串），别写成多个参数。

### 5.2 沿用自历史轮次（详见 `AGENTS.md` 对应条目）

- 真机验收只能用 `tauri build` 产物；`tauri dev` 的路由页内容区全空白，结论一律作废。
- **禁**用 PowerShell `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件（PS 5.1 按 ANSI 读会把中文变成 mojibake，还会写 BOM）⇒ 用编辑工具，或 node 的 `fs.readFileSync/writeFileSync(...,'utf8')`；改完立刻用 node 验 `s.includes('某个中文注释')` 且 `s.match(/\uFFFD/g)` 为空。
- `.ps1` / `.cmd` 必须**纯 ASCII**（按系统 ANSI 代码页读，中文会变乱码并被当命令执行）；中文一律作命令行参数传入。
- 后台跑长任务**不要挂 stdout 管道**；让 `.cmd` 内部 `> logfile 2>&1` 重定向，再单独读文件。
- `.cmd` 的 `echo` 里**不能出现 `>`**（会被当重定向符，仓库根目录会多出垃圾文件）。
- 应用日志被应用独占锁着 ⇒ 用 `File.Open(..., 'FileShare.ReadWrite')` + `StreamReader` 读；日志时间戳是 **UTC**（比本地少 8 小时）。
- `PrintWindow` 抓不到 overlay / hover 层 ⇒ 验这类交互走屏幕 DC BitBlt（`Graphics.CopyFromScreen`），并用 `WindowFromPoint` + `GetAncestor(h,2)` 校验实际绘制者（白名单**必须**包含应用自己的 `msedgewebview2` 子进程）。
- `SendInput` 的 `INPUT` 结构必须显式 `[StructLayout(LayoutKind.Explicit, Size = 40)]`；Ctrl+V 的 VK 是 `0x56`；粘贴前先 Ctrl+A。
- 「单修饰键 + 非字母主键」（如 Shift+Delete）的注入会**丢修饰键** ⇒ 永久删除走右键菜单顶部图标条里的垃圾桶，不要依赖键盘。
- UIA 判别要带**阳性对照**（先点一个必然有可见后果的元素），否则「没反应」分不清是注入失败还是功能没反应。
- `InvokePattern.Invoke()` 返回成功 ≠ 点击生效（禁用按钮上是静默空操作）。
- 验收夹具**不要建在 `%TEMP%` 下**（`builtin_ignored_paths()` 含 `/AppData/Local/Temp`，子树会被整棵剪掉）。
- 本机 DPI 144（150% 缩放）⇒ 物理像素 ÷ 1.5 = CSS 逻辑像素。
- `net use` 的盘符参数**必须带冒号**（`net use Y: \\host\share`）。

---

## 6. 交付标准（下一会话的收尾要求）

1. **T1 真机验收有硬证据**：目录树开关关掉 → 重启 → 仍然隐藏（截图或 UIA 文本 + `user-settings.json` 里的 `navigator.showFolderTree: false` 落盘证据 + `[state] tree=false` 探针输出），并有**修复前阴性对照**。
2. **真实用户数据已恢复**：跑完 `r39-finalize.ps1`，三文件 sha256 全部 `MATCH`；应用进程已退出；磁盘上不是夹具。
3. **提交已推送**：`origin/main` 与本地 HEAD 一致，`git status` 干净。
4. **向用户汇报**：目录树修复结论（含验收证据）、winget PR #467381 的最新标签状态、需要用户拍板的事项（是否发 3.9.1 / 是否补 winget 3.9.1 清单 / 是否同步 upstream）。
5. 报告里区分**「已验证 / 推断 / 未验证」**；没做的就说没做。
