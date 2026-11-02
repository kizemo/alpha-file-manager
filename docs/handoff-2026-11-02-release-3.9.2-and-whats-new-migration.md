# 交接文档 · 2026-11-02（下午）· 「新功能」迁移到关于页 + 发版 3.9.2 + 清理旧包

> **本文是接力入口。** 读者是下一个会话的 Agent。
> 先读完 §0 的前置文档再动手；本文只写「本轮新增的事实与下阶段计划」，不复述项目通史。

---

## §0 前置阅读（完整路径，按顺序）

| 顺序 | 文档 | 作用 |
|---|---|---|
| 1 | `F:\soft\00selfmade\filemanager\AGENTS.md` | **项目硬约束总纲**（外层仓，不在主仓里）。输出语言、门禁命令、真机验收规则、几十条实测踩坑。**必读，且优先级高于本文。** |
| 2 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-delete-hang-and-log-cleanup.md` | 本轮主文档。§7 = 任务 4「打开文件所在位置」的根因与真机验收全绿记录；§12 = `release/` 本地镜像的核对、定性与修复。 |
| 3 | `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-relay-four-tasks.md` | 四个任务的原始定义与验收判据。任务 3（全盘搜索极慢）尚未定案。 |
| 4 | `F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\session-status-20261102-e.md` | 本轮成果速览表 + 9 个 commit 表（比本文更紧凑，适合快速对齐）。 |

发版流程的历史范本（**照抄这几篇的步骤，别自己发明**）：

- `docs\handoff-2026-10-21-release-3.3.0.md` §7.1 —— aiec.fun 主源上传的完整步骤
- `docs\handoff-2026-10-20-release-3.2.11.md` —— 「打包发版 + version.json + README + 双端上传」的四件套范式
- `docs\handoff-2026-10-14-3.1.1-released-and-sync-fixed.md` —— 发布后**公网回读校验**的做法

---

## §1 项目坐标

| 项 | 值 |
|---|---|
| 主仓（代码） | `F:\soft\00selfmade\filemanager\sigma-file-manager` |
| 外层仓（AGENTS.md 所在） | `F:\soft\00selfmade\filemanager`，是**独立 git 仓**，remote = `https://github.com/kizemo/focus-sync.git` |
| 主仓 origin | `https://github.com/kizemo/alpha-file-manager.git` |
| 主仓 upstream | `https://github.com/aleksey-hoffman/sigma-file-manager.git`（上游，**不要 push**） |
| 品牌名 | **Alpha File Manager**（已锁定，不讨论改名/改图标） |
| Tauri identifier | `com.sigma-file-manager.app`（受保护常量，不可改） |
| 中文 locale code | **`ch`**（不是 `zh`），语言包在 `src\localization\messages\**`，共 17 个 |
| 已安装程序 | `D:\Program Files\Alpha File Manager\alpha-file-manager.exe`（80,307,712 B，mtime 2026-11-02 11:36:34） |
| 真实 user-data | `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\` |
| 应用日志 | `C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`（**UTC 时间戳**，运行时被独占锁，读它必须 `File.Open(..., 'Open','Read','ReadWrite')`） |

---

## §2 当前 git 状态（本轮结束时）

**主仓**：HEAD = `0f64f0d7`，工作树干净。

⚠️ **本地 9 个 commit 尚未 push**，`origin/main` 仍停在 `e6ca8afb`：

```
2fc908c5 → 2070f03b → cab049a6 → 174f1cd7 → 035662e2
        → 11675643 → 3cfaab77 → 0fea5ab6 → 0f64f0d7
```

其中关键三个：

| commit | 内容 |
|---|---|
| `11675643` | 任务 4 修复：`file-browser-actions-menu.vue` 补上从未渲染的 `open-containing-directory` 菜单项 + 3 条单测 |
| `0fea5ab6` | `fix(release)`：发布脚本不再覆盖已发布资产，内容不同则隔离到 `release/local-builds/`；新增 `scripts/verify-publish-bundle-quarantine.mjs` 与 `npm run verify:publish-quarantine` |
| `0f64f0d7` | 本文的前身文档补 §12 |

**外层仓**：HEAD = `54a8f52`（`AGENTS.md` 新增 4 条坑），**同样未 push**。外层仓还有未跟踪文件（`docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md`、`docs/blog/`、`docs/marketing/`、`vt3.txt`），**本轮未动，也不要顺手提交**。

> 用户已明确授权 push 到 GitHub 与 aiec，**不需要再征询**。

---

## §3 本轮已完成（成果表）

| 项 | 状态 | 证据位置 |
|---|---|---|
| 任务 4「打开文件所在位置」 | **修完 + 真机验收全绿**（7 条判据全过，含快捷方式定位与断链 toast 实拍） | `.superpowers\sdd\accept-20261102-reveal\`；文档 §7 |
| 任务 2「删除卡死」搬运期取消 | **修完 + A3g 真机验收全绿**（pass=7 fail=0） | `.superpowers\sdd\accept-20261102-A3g\` |
| §5.2 假 0% 进度 | 已修（deleting 阶段整体走不确定态进度条） | 见 `handoff-2026-11-01` |
| `release/` 本地镜像 | **已定性 + 已修脚本 + 已重写三个镜像文件** | `.superpowers\sdd\release-mirror-classify-20261102.txt`、`release\LOCAL-BUILDS.txt`、`release\SHA256SUMS.txt`、`release\README.md` |
| 任务 3 扫描侧计时 | **已拿到**（4 轮全盘扫描，见 §4.1） | 应用日志 |
| 任务 3 查询侧计时 | **未取得**，但已得到决定性否定证据（见 §4.2） | `.superpowers\sdd\accept-task3-20261102-v2\` |
| user-data 差异 | **已出报告**（见 §5） | `.superpowers\sdd\userdata-diff-20261102.txt` |
| 验收收尾 | 已恢复 5 个目录型 tab、回收站按白名单只清我方 7 条、磁盘夹具已清 | `restore-workspaces-20261102.cjs` |

门禁基线（**每次都用同一条命令量，混口径会误判成回归**）：

- `npm run ts` = 0
- `eslint .` = **188 errors / 0 warnings**（其中 `eslint src` = 175、`eslint scripts` = 13）
- ⚠️ **绝不能跑 `npm run lint`**；eslint 必须传**绝对路径**
- 只按 `git diff -U0` 的改动区间修 NEW 错误；未跟踪新文件整份算 NEW

---

## §4 任务 3（全盘搜索极慢）现状

### 4.1 扫描侧：已定案，瓶颈是四块盘**串行**建索引

埋点：`src-tauri/src/global_search/scan.rs:346`
格式：`[global-search] scan root timings: root={} elapsed={}ms indexed={} depth={}`

4 轮全盘扫描（UTC 04:17–05:14 = 本地 12:17–13:14），每轮**串行** G: → F: → D: → C:：

| 盘 | 第1轮 | 第2轮 | 第3轮 | 第4轮 |
|---|---|---|---|---|
| G: | 21,159 | 29,139 | 26,889 | 29,193 |
| F: | 142,955 | 157,354 | 158,979 | 155,966 |
| D: | 161,490 | 186,835 | 182,588 | 190,987 |
| C: | 403,362 | 374,864 | 361,667 | 373,933 |
| **合计 ms** | **728,966** | **748,192** | **730,123** | **750,079** |

单位 ms。每轮 **12.2–12.5 分钟**，四轮高度一致 ⇒ **不是抖动，是结构性串行**。
indexed 合计约 1,336,800 docs，depth = 9。
并行化理论下界 ≈ max(C:) ≈ **6.3 分钟**。

### 4.2 查询侧：埋点从未触发（决定性否定证据）

埋点：`src-tauri/src/global_search/query.rs:241`
格式：`[global-search] query timings: total={}ms index_open={}ms collect={}ms materialize={}ms finalize={}ms candidates={} results={} index_docs={}`

v2 验收脚本 `.superpowers\sdd\task3-search-accept-20261102-v2.ps1` 于 13:42:50 启动，全过程记录在
`.superpowers\sdd\accept-task3-20261102-v2\run-console.log`（144 行）。**已确证的事实链**：

1. UIA 树真正填充完成（`total=612 named=178`）—— v1 的「只等 8 秒」问题已修掉
2. `Ctrl+Shift+F` 成功打开全局搜索视图（`647 elements`，找到 1 个 `ControlType.Edit`，可访问名「全局搜索」，带 `ValuePattern`）
3. `Set-Clipboard 'exe'` → `Ctrl+A` → `Ctrl+V`，**`edit value after paste: 'exe'`** —— ValuePattern 读回确认文字进了输入框
4. 随后轮询应用日志 30 分钟：**`queryTimings=0`**，且 `scanTimings` 停在基线 **16** 没有增长
5. 截图 `shot-search.png`（4.6 MB）已落盘，**尚未人工判读**

⇒ **结论：文字进了 DOM 输入框，但查询根本没有执行。**

**下一步探针（新会话第一件事，按此顺序）**：

1. **先判读 `shot-search.png`**：搜索面板里到底有没有结果行？
   - 有结果 ⇒ 查询执行了，是**埋点没被走到**（去查 `query.rs:241` 的调用路径与日志级别门控）
   - 无结果 ⇒ 查询确实没发出去，继续第 2 步
2. **粘贴后补一次 `ENTER`**。当前 v2 脚本**没有按 Enter** —— 这是最可能的缺口。
   全局搜索很可能是「输入 + 显式提交」而非「输入即搜」。
3. **配阳性对照**（AGENTS.md 铁律：探针必须有牙）。判据不能只看日志，要同时看
   UIA 树里是否出现结果行 / 截图内容哈希是否变化。
4. 已知无关但别再踩：全局搜索的放大镜按钮**没有可访问名**
   （`src/modules/global-search/global-search-toolbar-button.vue` 渲染纯图标 Button），
   按名字找它必然失败 ⇒ **只能走快捷键**。
   快捷键定义在 `src/stores/runtime/shortcuts.ts:103-117`（`toggleGlobalSearch` = ctrl+shift+f，
   `scope:'global'`，`conditions:{dialogIsOpened:false}`），handler 在 `src/composables/use-init.ts:97-101`。

⚠️ **AGENTS.md 规定：任务 3 未定案前不要写修复代码。** 本轮只交数据。并行扫描是候选方案，但要先拿到查询侧数据才能判断「慢」到底慢在建索引还是查索引。

---

## §5 user-data 当前值 vs 2026-11-01 备份（回答用户问题 3）

**用户已决定：不恢复。** 以下是差异说明。

- 备份目录：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\user-data-backup-20261101\`
- 实时目录：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\`
- 完整报告：`.superpowers\sdd\userdata-diff-20261102.txt`（对比脚本 `.superpowers\sdd\userdata-diff-20261102.cjs`，**只读**）
- 快照时间：2026-11-02 14:09:25（**当时应用正在运行**，`user-extensions.json` 在被持续写入，字节数会漂）

**目录结构与文件集合完全一致**：顶层各 18 个文件 + 子目录 `media\source-backgrounds\` 各 6 张壁纸 jpg（递归合计 24 个文件），无新增、无丢失。

- 顶层 18 个里 **14 个 sha256 完全相同**（`.user-settings.before-acceptance.json`、`user-remote-connections.json`，以及 12 个历史 `*.backup` / `*.bak` / `*.userstate` 快照）。
- `media\source-backgrounds\` 的 **6 张壁纸 sha256 也全部一致**（2026-11-02 14:2x 补测，见 §5.5）。

**只有 4 个文件内容不同**：

| 文件 | 备份字节 | 当前字节 | 差异性质 |
|---|---|---|---|
| `user-workspaces.json` | 863 | 1,685 | **本轮验收的有意改动**：恢复了 5 个目录型 tab，`currentTabGroupIndex` 1 → 4 |
| `user-settings.json` | 5,382 | 5,378 | **纯 UI 状态漂移**：`settingsCurrentTab` `appearance` → `general`；`lastRoute.name` `navigator` → `settings`；`appUpdates.lastCheckTimestamp` 刷新 |
| `user-stats.json` | 32,649 | 32,272 | **历史/常用项滚动窗口**：`history` 99 → 98 条，`frequentItems` 99 → 100 条 |
| `user-extensions.json` | 559,142 | 529,570 | **两个扩展各自的 settings 变化**，见下 |

`user-extensions.json` 的细节（顶层 5 个键，只有 2 个变）：

- `registryCache.fetchedAt`：时间戳刷新（`data` 部分**完全一致**，2,549 B）
- `installedExtensions.kizemo.focus-sync.settings`：**59,670 → 29,513 B（少约 30 KB）**
- `installedExtensions.sigma.clipboard.settings`：**338,201 → 359,189 B（多约 21 KB）**
- 两个扩展的 `enabled` / `autoUpdate` / `version` / `manifest` / `installedAt` **全部未变** ⇒ 扩展没被卸载、没被禁用

**需要用户注意的唯一一项**：`kizemo.focus-sync` 的 settings 体积**腰斩**。
本轮**没有**任何脚本写过 `user-extensions.json`，所以这不是验收造成的；
最可能是该扩展自身的滚动清理/状态重置。
👉 **新会话应向用户确认 focus-sync 的同步状态是否仍符合预期**；若不符合，备份里那份 59,670 B 的 settings 还在，可单独取回。

`user-stats.json` 的具体进出条目（滚动窗口自然淘汰，非丢失）：

- 当前新增：`D:/Program Files/dabi`、`D:/Program Files`、`C:/Users/Duanyi/AppData`、`…\Start Menu\Programs`、`F:/soft/00selfmade/ecclaw`
- 备份里有、当前已滚出：2 条 `sfm://remote/…/var/www/wordpress*`、4 条 `E:/办公文件/A1综合管理/A02登记注册/02上海昀昕/2026年检资料/…`

> ⚠️ `user-remote-connections.json` 的 **mtime 变了但 sha256 相同**（应用启动时重写了一遍同样的内容）。别把 mtime 当改动证据。

### 5.5 一处已闭合的覆盖盲区（记录以免重做）

首版 `userdata-diff-20261102.cjs` 用 `readdirSync` **只比了顶层，没有递归子目录**，
因此漏掉了 `media\source-backgrounds\` 下的 6 张内置壁纸；
而首版报告又误写成「18 个里 14 个相同（含 6 张壁纸 jpg）」——**那句话是错的**，
14 = `.user-settings.before-acceptance.json` + `user-remote-connections.json` + 12 个历史快照，与壁纸无关。

2026-11-02 已用 `Get-FileHash -Algorithm SHA256` 两侧逐一比对补齐：
**6 张壁纸 sha256 全部一致**（511881 / 608779 / 327244 / 379120 / 354709 / 584577 字节），`same=6 diff=0`。
⇒ §5 的差异结论现已覆盖 `user-data` 的**全部 24 个文件**，不存在未验证部分。

> 另一处漂移：本次复核时 `user-extensions.json` 已从快照时的 529,570 B 涨到 **535,330 B**（应用仍在运行、扩展在持续写入）。
> ⇒ §5 表格里该文件的字节数只是**快照瞬时值**，不要拿它当稳定基线。

---

## §6 下阶段任务（按顺序执行）

### 任务 A：把「设置 → 常规」里的「新功能」迁移到「关于」页

**精确文件坐标（已勘察，直接开工）**：

| 角色 | 路径 |
|---|---|
| **迁出源** | `src\modules\settings\ui\categories\general\whats-new.vue`（`import { useChangelog }`，本地 `function openChangelog()`，模板 `@click="openChangelog"`） |
| 源的注册处 | `src\modules\settings\ui\categories\general\index.vue` |
| **迁入目标** | `src\modules\settings\ui\categories\about\index.vue` 与 `about.vue` |
| changelog 能力本体 | `src\modules\changelog\composables\use-changelog.ts`（`useChangelog()` 暴露 `open` / `isOpen` / `releases` / `appVersion` / `markAsSeen`）、`src\modules\changelog\components\changelog-dialog.vue`、`src\modules\changelog\index.ts` |
| 对话框挂载点 | `src\layouts\window\app.vue:11,35`（`<ChangelogDialog />`，**全局挂载，迁移时不要动它**） |
| 启动自动弹出 | `src\composables\use-init.ts:24,85,520-522`（`checkAndShowChangelog()`）—— **这就是「启动会弹新功能模态框挡住文件列表」的来源，本轮不要改它的行为** |
| ⚠️ 另一处入口 | `src\modules\settings\ui\actions.vue:39,44,101`（`const { open: openChangelog } = useChangelog()`，模板 `@click="openChangelog"`）。该文件 81 行有注释：「原 `openChangelogPage` 指向 GitHub 的 CHANGELOG.md，现『更新日志』改指 aiec 下载页」。**迁移前必须先读这个文件，判断这个按钮是否也要一起迁 / 是否与 whats-new.vue 重复。** |
| 设置页搜索关键词 | `src\localization\messages\ch.json:766` → `"whatsNew": "新功能, 更新日志, 版本说明, 版本, 更新, 新闻"`。**17 个语言包都有对应键**，迁移后搜索索引的归属分类要跟着改，否则在设置页搜「新功能」会跳到已不存在的分类。 |
| 设置页导航 | `src\modules\settings\ui\nav.vue`、`nav-item.vue`、`content.vue`、`view.vue`、`header.vue` |

**迁移要求**：

1. 「关于」页里保留同等可达性（一个能点开 changelog 对话框的条目），文案沿用现有 i18n 键，**不要硬编码中文**
2. 「常规」页移除该条目后，`general/index.vue` 的 import 与注册要一起清掉，别留死代码（eslint 会报 unused）
3. 17 个语言包（`src\localization\messages\**`）里 `whatsNew` 搜索关键词的**归属分类**要跟着搬
4. **补单测**：断言「关于」页渲染出该入口、「常规」页不再渲染。参考本轮 `11675643` 的写法（3 条单测 + 负对照）
5. 门禁：`npm run ts` = 0、改动文件 eslint = 0、`npm run test:unit:run -- <files>` 全绿、stylelint 只允许 OLD 违规

### 任务 B：发版 3.9.2

**版本号 = `3.9.2`**（3.9.1 已发布，**不可覆盖**）。

步骤：

1. **先杀 AFM 实例**（单实例守卫；exe 被占用会 `failed to remove file … os error 5`）：
   `Get-Process alpha-file-manager | Stop-Process -Force`
   ⚠️ 本轮结束时 **pid 894176 可能仍在运行**（v2 脚本被中途 stop 了），开工第一件事就是确认它没了。
2. **三处版本号必须一致**：`package.json`、`src-tauri\Cargo.toml`、`src-tauri\tauri.conf.json`。
   项目有 `npm run sync-version`，`tauri:build` 会自动先跑它。
3. **打包**：`npm run tauri:build`
   （= `sync-version` + `tauri build` + `node ./scripts/publish-bundle-to-release.mjs`）。
   - Rust toolchain 不在 PATH：`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`（**`pc-` 不能省**）
   - MSVC 环境：`call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`
   - **可直接复用模板** `.superpowers\sdd\build-reveal-menu-20261102-b.cmd`
   - `.cmd` 必须**纯 ASCII**、**运行期间绝不能编辑**（cmd.exe 按字节偏移逐行读，会导致执行错位；本项目曾因此把版本从 3.6.0 顶到 3.7.0）。要改就**新建文件名**。
   - **后台跑不要挂 stdout 管道**（`Tee-Object` / `| Select-String` 一断进程树就死）。让 `.cmd` 内部自己 `> logfile 2>&1`。
   - `tauri build` 很慢（十几分钟），**早启动、丢后台**，同时做文档。
4. **⚠️ 分清「构建失败」和「复制失败」**：`publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 下所有历史安装包逐个复制；只要 `release/` 里**任何一个**旧包被别的进程锁住，整步就 exit 1，而 `tauri build` 本身早已成功。新包可用 `node -e "fs.copyFileSync(a,b)"` 单独补进去，**不必重跑打包**。
5. **文档四件套**：
   - `README.md` 与 `README.zh-CN.md`（两份都要改；`README.md:191,196,252` 与 `README.zh-CN.md:176,181,228` 是下载入口段落）
   - `CHANGELOG.md`（仓库根）
   - **aiec 版本文件 = 站点上的 `version.json`**，见下
   - 本轮的 handoff 文档补一节发布结果
6. **commit → push 到 GitHub 与 aiec**（用户已授权，不必再问）

**关于 aiec（重要，容易误解）**：

- **`aiec` 不是 git remote**，是主下载/更新源站点 **`https://www.aiec.fun/alpha-file-manager/`**
- 「aiec 版本文件」= 站点上的 **`version.json`**（应用内 `app_updater` 的主源就是它；GitHub Releases 是兜底源）
- 「push 到 aiec」= **上传安装包 + 更新 version.json 到该站点**，走 `scripts\release-publish.mjs`：
  `npm run release:prepare` → `release:upload` → `release:verify` → `release:github` → `release:status`
- `release:prepare` 用 `latestInstaller()` 的 sha256 写 `SITE_DIR/version.json`；
  `release:github` 的上传源是 **`SITE_DIR/Alpha_File_Manager_<ver>_x64-setup.exe`**（**不是 `release/`**）
- ⚠️ `latestInstaller()` 在 `0fea5ab6` 之后**也会扫 `release/local-builds/`**
- ⚠️ **命名口径**：官方资产用**下划线**（`Alpha_File_Manager_<ver>_x64-setup.exe`），`release/` 本地镜像用**空格**（`Alpha File Manager_<ver>_x64-setup.exe`）
- 🚨 **`docs\handoff-2026-10-22-delete-progress-and-installer-log.md:251-256` 记录过：`www.aiec.fun` 从这台机器上不可达**（`app updater: primary source aiec.fun failed`）。
  ⇒ **上传前必须先预检可达性**（`Invoke-WebRequest -Method Head https://www.aiec.fun/alpha-file-manager/version.json`）。
  不可达就**先做 GitHub 侧**，并把 aiec 侧明确报成「未完成 + 原因」，**不要静默跳过**。
- 官方资产的 SHA256 **不必下载**，从 GitHub Releases API 的 `digest` 字段取（`sha256:...`）：
  `Invoke-RestMethod -Uri 'https://api.github.com/repos/kizemo/alpha-file-manager/releases?per_page=100' -Headers @{'User-Agent'='afm-accept'}`
  本机到 GitHub 发行 CDN 实测 **0.01 MB/s**（44 MB 要 40+ 分钟），**别试图下载资产做比对**。

**发布后必须做公网回读校验**（范本见 §0 的 3.1.1 那篇）：站点 `version.json` 的 version / pub_date、GitHub Release 的 tag / 资产 digest，四处状态一致才算发完。

### 任务 C：发版后清理旧打包文件（节约空间）

**用户口径**：旧的打包文件可以清理；**`release/` 里的本地构建文件不用改名**（维持「登记 + 隔离」方案）。

现状（`.superpowers\sdd\release-mirror-classify-20261102.txt`）：
`release/` 共 **49 个本地安装包** = **16 OFFICIAL_COPY**（与官方字节一致）/ **12 LOCAL_REBUILD**（本地重建，字节与官方不同）/ **21 NO_OFFICIAL_ASSET**（官方根本没这个资产）。
28 个官方 x64-setup 资产**全部**在本地有对应文件。
12 个 LOCAL_REBUILD 的版本号：`2.2.2 / 3.0.1 / 3.1.1 / 3.2.4 / 3.2.11 / 3.5.0 / 3.5.1 / 3.6.3 / 3.6.4 / 3.8.0 / 3.9.0 / 3.9.1`。

清理规则（**必须遵守**）：

1. `release/` **整个目录被 gitignore**（`.gitignore:43`）⇒ 清理不影响仓库，`git status` 也看不到，**要自己记录清了什么**
2. **不要永久删除**。走 `E:\Users\Duanyi\.minimax\.minimax\bin\mavis-trash.cmd -- "<路径>"`（可恢复）
3. **先杀掉可能锁住文件的进程**（实测 3.4.0 曾被锁）。锁住会让清理脚本 exit 1
4. **保留策略要先跟用户确认具体保留几个版本**，别自己拍。建议保留：最新 2–3 个 OFFICIAL_COPY + 当前 3.9.2 + `release/local-builds/` 全部（那是隔离区，是证据）
5. `release\SHA256SUMS.txt`（2,266 B，只收官方一致项）、`release\LOCAL-BUILDS.txt`（8,795 B，逐条登记本地 sha256 / 官方 sha256 / 官方 URL）、`release\README.md`（8,629 B）是本轮重写的**诚实性凭据**，清理后要**同步更新**，不能留下指向已不存在文件的条目
6. 🔍 **顺手量一下 `src-tauri\target\`** —— 那通常才是磁盘大头（Rust release 产物动辄十几 GB），比 `release/` 值得清。清之前先确认没有正在跑的构建。

### 任务 D：任务 3 查询侧计时收尾

见 §4.2 的四步探针。**这是唯一还没定案的任务**，定案前不要写修复代码。

---

## §7 硬约束速查（只列最易踩的；完整清单在 `AGENTS.md`）

- **真机验收必须用 `tauri build` 的打包产物**。`tauri dev` 下所有路由页面内容区空白（`RouterView` 挂载但不渲染子组件），**在 dev 构建上做的界面验收结论一律作废**。
- 面向用户输出一律**简体中文**；代码标识符、路径、命令、配置键名保持原样。
- 验收脚本**只能在 Windows PowerShell 5.1 下跑**（`powershell.exe -NoProfile -ExecutionPolicy Bypass -File ...`）。PS7 会报 `CS1069: System.Drawing.Bitmap` 找不到。中文通过**命令行参数**传入。
- `.ps1` / `.cmd` 都必须**纯 ASCII**（PS5.1 / cmd.exe 按 ANSI 读无 BOM 文件，中文会搞崩解析器）。
- **绝不要**用 PowerShell `Get-Content -Raw` / `Set-Content` 改带中文注释的源文件（会 mojibake）。改 JSON 用 node `fs.readFileSync/writeFileSync(..., 'utf8')`。
- `SendInput` 的 `INPUT` 结构必须显式 `[StructLayout(LayoutKind.Explicit, Size = 40)]`，否则 `SendInput` **静默返回 0**。照抄 `.superpowers\sdd\app-key.ps1`。
- 输入文字首选路径：`Set-Clipboard` + `app-key.ps1 -Keys 'CTRL+V'`。粘贴前先 `CTRL+A`，否则是**追加**。
- **注入探针要「阳性对照排在前面」**：先跑必然命中的那项，再跑必然不命中的那项。
- **UIA 读不到 Radix Toast**（toast 出现 ≠ UIA 查得到）⇒ 必须配屏幕 BitBlt 实拍。
- `PrintWindow` **不渲染 hover / overlay 层**，且在平滑滚动未结束时会抓到黑块，**还会返回陈旧帧**。判后台任务运行状态要看**文件系统**，不要看屏幕。
- 屏幕 BitBlt 会被别的窗口遮挡 ⇒ 用 `WindowFromPoint` + `GetAncestor(h,2)` 校验实际绘制者，白名单**必须包含应用自己的 `msedgewebview2`**。
- Radix 对话框在 UIA 里是 `[Window]`，可访问名 = 标题。**元素搜索必须限定在该子树内**，否则会点到标题栏关闭按钮。
- Radix 菜单的 UIA 枚举**有竞态** ⇒ 必须轮询重试。
- 取主窗口**不要用 `Process.MainWindowHandle`**（会落在 15×15 的托盘窗）。自己 `EnumWindows` + 过滤 class = `Tauri Window` + 可见 + 面积最大。
- 应用启动会弹「新功能」更新日志模态框挡住列表 ⇒ 验收脚本必须先关它（关闭按钮名「关闭」= U+5173 U+95ED）。
- 验收量到的是**物理像素**，本机 DPI 144（150% 缩放），换算 CSS 逻辑像素要**除以 1.5**。
- `Save-AfmShot` 拍整窗 ⇒ **屏幕坐标 = PNG 坐标 − 窗口原点**（本机原点 `(-11,-11)`）。
- 破坏性靶子**不要放 `%TEMP%`**（`builtin_ignored_paths()` 会剪掉整棵子树，命中恒为 0）。放 `C:\Users\<user>\` 下的普通目录。
- 恢复真实数据顺序：**先杀进程 → 再 restore**（应用退出时会把内存统计状态写回磁盘）。
- `SFM_E2E_DATA_DIR` **只在 debug 构建生效**，release 构建把 override 编译掉了。
- `vitest --reporter=basic` 在本仓不存在 ⇒ 用默认 reporter，按 `Test Files` / `Tests` 两行抓结果。
- `$ErrorActionPreference = 'Stop'` 会把原生命令的 stderr 变成终止错误 ⇒ 调 `npm` / `npx` 前后显式切回 `Continue`。
- `cargo test --lib` 里任何被链接到的代码都**不能碰 `tauri::AppHandle` 或 `tauri::async_runtime::*`**（会 `STATUS_ENTRYPOINT_NOT_FOUND` 0xC0000139 加载期崩溃）。
- 单轮工具调用 ≤ 10 次；中间产物一律落盘，对话只留指针 + 结论。
- `E:\Users\Duanyi\.minimax\.minimax\maintenance\task-state\LATEST.md` 属**另一个项目**（rime_claude / Fluxing / winget PR），**与 AFM 无关，不得据其行事**。

**可复用脚本**（都在 `.superpowers\sdd\`）：
`app-key.ps1`（按键注入）、`afm-accept.ps1`（`Show-AfmWindows` / `Get-AfmMainWindow` / `Save-AfmShot` / `Test-AfmPointClear`）、`shift-occluders.ps1`（挪开遮挡窗口）、`dash-accept-data.cjs`（`backup|seed|restore`）、`restore-workspaces-20261102.cjs`、`build-reveal-menu-20261102-b.cmd`（打包模板）、`task3-search-accept-20261102-v2.ps1`、`userdata-diff-20261102.cjs`、`release-mirror-fix-20261102.cjs`、`official-assets-20261102.json`（28 条官方资产）。

---

## §8 待用户决策 / 需用户本人操作

| 项 | 状态 |
|---|---|
| **winget PR #467381 卡 Needs-CLA** | 须**账号本人**执行：`gh pr comment 467381 --repo microsoft/winget-pkgs --body "@microsoft-github-policy-service agree"`。本机 `gh` **未登录**。指南已落盘 `docs\winget-cla-signing-guide.md`。用户至今未回应。 |
| **两条历史删除作业来源** | 待用户确认是否本人所为：① 12:45:26 UTC `use_trash=false`（job `7be5e1cf-240f-46f8-af17-e66b2edd4836`）② 12:53:22 UTC `total_items=101 / deleted_items=101 / use_trash=true` |
| **`kizemo.focus-sync` settings 腰斩** | 见 §5，需用户确认同步状态是否仍符合预期 |
| **清理保留策略** | 见 §6 任务 C 第 4 条，保留几个版本要用户拍 |
| `release/` 里 12 个本地构建的官方字节 | 用户已表态**不用改名**，维持「登记 + 隔离」。若要换回官方字节，受 0.01 MB/s 网速限制（44 MB / 个）。 |

**仍未追到的三个谜团**（低优先，别当阻塞）：
`webview guard` 61,775 ms 心跳超时的触发源；`Error code 26: File opened that is not a database file`（global-search 索引 sqlite）的来源；`.superpowers\sdd\accept-mem-e2e-20261031.ps1:316` 的 `GetElement(int)` bug。

---

## §9 新会话接力用的一句话 prompt

> ⚠️ **本文已被下一阶段的交接文档取代，接力请改用新的那篇：**
> `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md`
> 本文保留为**上一阶段的原始记录**（§7 任务 4 真机验收、§5 user-data 逐文件差异、§6 任务 A→D 原始定义、§10 执行结果），
> 只在需要追溯细节时读。

```
请先按顺序完整阅读这 4 份文档（完整路径如下），据此掌握 Alpha File Manager 项目的背景、目标、已完成进度与下阶段计划，读完先向我简述你理解的当前现状与准备先做哪一步，不要复述文档内容，也不要重做已完成的事：1) F:\soft\00selfmade\filemanager\AGENTS.md —— 项目硬约束总纲，优先级最高；2) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md —— 本阶段交接入口，含当前状态与下阶段任务；3) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-relay-four-tasks.md —— 四个任务的原始定义与验收判据；4) F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-release-3.9.2-and-whats-new-migration.md —— 上一阶段交接原文，含发版与清理的细节和踩坑记录。
```

---

## §10 任务 A→D 执行结果（2026-11-02 20:40 收官）

§6 的四个任务已全部执行完毕并验证。下表为准，§2 / §8 中与本节冲突的旧描述作废。

| 任务 | 状态 | 结果 |
|---|---|---|
| A 「新功能」迁到「关于」页 | ✅ | commit `41d03516`。`whats-new.vue` 整条目平移到 `src/modules/settings/ui/categories/about/`，store 里 `category:'about'`、排在 `aboutInfo` 之后；新增单测 6 条全绿 + 负对照（改回 `general` → 2 条红 → 还原）；17 个语言包未动 |
| B 发版 3.9.2 | ✅ | commit `8b0d8cd0` + 轻量 tag `v3.9.2`（已推 origin）；**含一次缺陷修复后的同版本重发**，见下 |
| C 清理旧打包文件 | ✅ | 47 个旧安装包移出 `release/`；`src-tauri/target/debug`（69.74 GiB）整体迁出。**F: 净释放 62.3 GB**（74.3 → 136.60 GB），E: 暂存区再**回收 70.79 GB**（1,315 → 1,385.86 GB） |
| D 任务 3 查询侧计时 | ✅ | 真机取到 `total=21ms`（index_open 0 / collect 18 / materialize 3，191 万文档）→ **查询侧不是瓶颈，不写修复代码**；瓶颈在扫描侧（4 轮 12.2–12.5 分钟/轮） |

### 10.1 发版缺陷与「同版本重发」（重点留档）

首个 3.9.2 安装包内嵌了**过期 changelog**：vite 在 08:03:39 把 `public/changelog/ch/changelog.md`（当时还没有 3.9.2）拷进 `dist/`，而 3.9.2 的 changelog 08:05:50 才写入源文件，cargo 08:07:49 链接时嵌的是旧 `dist/`。

处置：重建 → **同版本重发**（不升 3.9.3）。新包占 `release/` 官方位，旧缺陷包隔离到 `release/local-builds/` 留痕。

修正包（源站 / Cloudflare 边缘 / GitHub `v3.9.2` 三处一致）：

```
44,873,125 B
sha256 fb9d4708ff1a6d3d0accaf41a3de44a31d8c3ee6b5a5b9cc6317f1b658d90428
```

⚠️ 首发时 **Cloudflare 边缘仍缓存旧包**（max-age=7200），而 `version.json` 已是新 sha256 → 用户会下到校验不过的包。
处置用**定向 purge**（只清安装包与 version.json 两个 URL）；**不要用 `purge_everything`**，那会冲掉整个 aiec.fun 站点缓存。

### 10.2 `release/` 目录约定（已确立）

- **顶层** = 与 GitHub 官方资产逐字节一致的官方位（当前 3.9.2 / 3.7.0 / 3.6.5）
- **`local-builds/`** = 隔离区，放本地重建产物 / 同名不同内容的包（当前旧缺陷 3.9.2 + 3.9.1）
- `latestInstaller()` 同时搜两处、按 **mtime** 取最新 ⇒ 顶层新包必须比隔离区同名包新，否则会把错误 sha256 写进 `version.json`
- 三份登记文件由 `.superpowers/sdd/release-mirror-fix-20261102.cjs` 生成；当前 `OFFICIAL_COPY=3 / LOCAL_REBUILD=2 / NO_OFFICIAL_ASSET=0`

### 10.3 本轮已修复的小瑕疵

`release/SHA256SUMS.txt` 之前 `sha256sum -c` 会多报 `1 line is improperly formatted`。
根因：清单头与条目之间有一条**空行**——GNU `sha256sum` 只跳过 `#` 注释行，**不跳过空行**。已改成 `#`，并同步修正生成器。现在 3 条全 OK、**warning_count = 0**。

### 10.4 winget PR 现状（2026-11-02 20:37 复查）

PR `microsoft/winget-pkgs#467381`：`OPEN / REVIEW_REQUIRED / mergeable=MERGEABLE / mergeState=BLOCKED / reviews=0`。
阻塞点是**社区审核员的 `Moderator-Approved` 还没给**（用户本人 11-02 07:51 已签 CLA），不是 PR 本身有问题。

**用户已拍板**：保持 3.9.1 不动，**等本 PR 合并后另开 3.9.2 的 PR**。
3.9.2 的 manifest 已备好在 `.superpowers/sdd/winget-392/3.9.2/`（4 个 yaml，脚本 `prepare-winget-392.cjs`，sha256/URL 取自 GitHub 实际资产，自检 `problems = 0`）。

### 10.5 用户已拍板的三件事（2026-11-02）

1. `E:\_afm-trash-20261102` 暂存区 → **永久删除**，已完成（F: 迁出 + E: 回收站清空，合计回收约 133 GB）
2. F: 回收站里用户自有的 3 项 → **不清空**，保持原样
3. 第二条 `total_items=101` 的删除作业 → **不再追查**（性质是 `use_trash=true`，进回收站可恢复，非永久删除）