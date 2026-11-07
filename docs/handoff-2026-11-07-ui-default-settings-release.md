# 交接卡 · 3.12.2 「UI 默认设置 + reveal 修复」发版收尾

- 日期：2026-11-07
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（分支 `main`）
- 起点交接卡：`docs/handoff-2026-11-07-ui-default-settings.md`
- 状态：**3.12.2 已全渠道发布完成**——发版提交 `6e68d337`（annotated tag `v3.12.2`，tag 对象 `b50c3b08`，已推 origin），`main` HEAD = `6eabbc67` 且与 `origin/main` 同步；真机验收两阶段 9/9（见 §4）；站点与 GitHub Release 已上线并回读校验一致（见 §7.2）。

---

## 0. 项目背景（新会话先读这一节）

**产品**：Alpha File Manager —— Windows 桌面文件管理器。技术栈 Tauri 2（Rust 后端）+ Vue 3 + Pinia + TypeScript（前端），安装器 NSIS。品牌名已锁定，不要重新讨论命名或图标。

**两个独立仓库，别混**：

| 仓库 | 路径 | 远端 | 内容 |
|---|---|---|---|
| 内层（产品） | `F:\soft\00selfmade\filemanager\sigma-file-manager` | `kizemo/alpha-file-manager` | 全部产品代码、发版脚本、`docs/` 交接卡 |
| 外层（项目约定） | `F:\soft\00selfmade\filemanager` | `kizemo/focus-sync` | `AGENTS.md` 项目硬约束总表、扩展与文档 |

两个仓各有独立 git 历史，**不要当成同一个仓操作**。

**发版链路（本轮已完整跑通，可直接复用）**：

1. 改 `package.json` 版本 → `npm run sync-version` 同步 3 个 Cargo manifest（`src-tauri/Cargo.toml`、`default-file-manager-common/Cargo.toml`、`default-file-manager-launcher/Cargo.toml`）；`src-tauri/tauri.conf.json` 引用 `"../package.json"`，无需改。
2. **先写完所有发版元数据 → 再打包**：`CHANGELOG.md`、`README.md`、`README.zh-CN.md`、`_notes-<版本>.md`、`public/changelog/<locale>/changelog.md`（14 个语言包，中文 locale code 是 `ch` 不是 `zh`）。顺序反了会出现「安装包里带的还是上一版更新日志」——本轮真实踩到，见 §5。
3. `npm run tauri:build` 打包（很慢，早启动丢后台）。构建脚本放 `F:\soft\00selfmade\filemanager\.superpowers\sdd\*.cmd`，**必须纯 ASCII**；Rust toolchain 路径里 `pc-` 不能省。
4. `node scripts/release-publish.mjs prepare --bump none --skip-build --notes-file _notes-<版本>.md` 更新站点 `version.json` 并把安装包复制到站点源目录。
5. 上传站点：**先传安装包，最后翻 `version.json`**（反过来用户会点到还没传完的包）。
6. `node scripts/release-publish.mjs github` 建 GitHub Release；`node scripts/release-publish.mjs verify` 做公网回读（版本号 / 大小 / sha256 三项）。

**输出语言**：面向用户一律简体中文；代码标识符、路径、命令保持原样。

---

## 1. 本轮目标

承接上一轮交接卡的三件事：

1. 把附图规格里的「视觉效果 / 样式设置」指标做成**应用默认设置**，使「重置为默认」与「新装软件」都回到这组值。
2. 完成上一轮遗留的 `eslint .` 全量基线对照。
3. 完成发版决定（3.12.2 补丁版、三处版本号对齐）与提交收尾。

## 2. 代码改动（已提交）

| 提交 | 内容 |
|---|---|
| `067640dc` | fix(reveal)：快捷方式「打开文件所在目录」解析到目标目录并消除三个系统弹窗（上一轮遗留，7 个 Rust + 4 个前端/测试文件） |
| `e9c41a10` | feat(settings)：视觉效果默认参数改为规格值（叠加模糊 / 不透明度 / 噪点归零）+ 4 条回归测试 + 3 个 docs md |

默认值只改了 6 个数值，位置在 `src/stores/storage/user-settings.ts`：

- `createDefaultInfusionPageSettings()`（通用页）：`blur 64 → 0`、`opacity 5 → 0`、`noise 5 → 0`
- `createDefaultInfusionPageSettingsHome()`（home 页）：`blur 32 → 0`、`opacity 5 → 0`、`noise 5 → 0`

其余项（媒体对比度 100 / 媒体亮度 100 / 噪点缩放 0.5 / 混合模式 / 三个开关 / 亮度 100% / 对比度 100% / 对话框背景模糊 8px）本来就与规格一致，未动。

**为什么改这一处就够**（已逐条核实，非推断）：

- `initUserSettings()`（`user-settings.ts:524-542`）在 `init()`（`:799-815`）里**先于** bootstrap hydrate / `loadUserSettings()` 执行，`userSettingsDefault.value = cloneDeep(userSettings.value)` 此刻快照的正是内置默认值 ⇒ 一处改动同时贯通「默认值 / 重置为默认 / 新装软件」三条路径。
- `src/stores/schemas/user-settings.ts` 对 blur/opacity/noise **没有第二份默认值**；`src-tauri/src` 对 `infusion` 零命中（纯前端项）。
- 设置页 `src/modules/settings/ui/categories/appearance/visual-effects.vue:321 resetVisualEffectsSection()` 克隆 `userSettingsDefault` 回写，**无需改动**。

**不做存量用户自动迁移**：需求只要求「新装」+「重置为默认」，加版本化迁移属扩大范围。CHANGELOG / `_notes-3.12.2.md` 已如实写明「存量安装不会自动迁移」。

## 3. 门禁

| 项 | 结果 |
|---|---|
| `npx vitest run src/stores/storage/__tests__ src/stores/schemas/__tests__` | 13 files / 184 tests 全过 |
| 新增回归测试文件单跑 | 1 file / 14 tests 全过 |
| `npx eslint src/stores/storage/user-settings.ts` + 新测试文件 | exit 0 / 0 问题 |
| `eslint .` 全量基线对照 | HEAD（`b49fbd31`）**180 errors / 0 warnings** → 工作副本 **177 errors / 0 warnings**，**零新增**；唯一差异是 `src/utils/__tests__/reveal-containing-directory.test.ts` 由 3 → 0（上一轮重写顺手修掉 3 条存量错误）。历史 3.12.1 发版提交记录的基线同为 180，互相印证 |

版本对齐：`package.json` 3.12.1 → **3.12.2**，`npm run sync-version` 同步 3 个 Cargo manifest（`src-tauri/Cargo.toml`、`default-file-manager-common/Cargo.toml`、`default-file-manager-launcher/Cargo.toml`）；`src-tauri/tauri.conf.json` 的 version 引用 `"../package.json"`，无需改。

发版元数据：`CHANGELOG.md` 新增 `## [3.12.2] - November 2026`；`README.md` 新增 `### What changed in 3.12.2`；`README.zh-CN.md` 新增 `### 3.12.2 改了什么`；`_notes-3.12.2.md` 新建；**14 个 locale 的更新日志在 `public/changelog/<locale>/changelog.md`**（ch/de/es/fa/fr/hi/it/ja/pt/ru/sl/tr/ur/vi，ch 用中文段落式，其余英文），**不在** `src/localization/messages/*.json`（那里只是更新弹窗的 UI 文案）。`npm run verify-changelog` → exit 0。

## 4. 真机验收（已完成）

验收脚本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-accept-defaults2.ps1`（纯 ASCII，中文串一律走命令行参数）。
证据输出目录：`F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-defaults-3122\`。

| 阶段 | 场景 | 结果 |
|---|---|---|
| A | 全新用户数据目录（无 `user-settings.json`）首次启动 → 设置 → UI 外观 → 视觉效果 | ✅ **PASS 9/9**：读回 `叠加模糊：0px` / `叠加不透明度：0%` / `叠加噪点：0%` / `叠加媒体对比度：100%` / `叠加媒体亮度：100%` / `叠加噪点缩放：0.5` / `亮度：100%` / `对比度：100%` / `对话框背景模糊：8px` |
| B | 把配置改成非默认值（blur 64 / opacity 5 / noise 5）→ 点「将视觉效果重置为默认值」 | ✅ **PASS 9/9**：重置前读回 `叠加模糊：64px` / `叠加不透明度：5%` / `叠加噪点：5%`；点重置按钮后读回 `叠加模糊：0px` / `叠加不透明度：0%` / `叠加噪点：0%`，其余 6 项（媒体对比度 100%、媒体亮度 100%、噪点缩放 0.5、亮度 100%、对比度 100%、对话框背景模糊 8px）不变 |

两阶段都在**最终打包产物**（`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = 3.12.2，安装包时间戳 20:39:50）上跑通，不是 dev 构建。

### 4.1 阶段 A 第一次失败的原因（已修，不是产品缺陷）

第一次跑阶段 A 失败，**不是产品缺陷**，是脚本定位问题：

- 侧边栏导航是**纯图标按钮，没有无障碍名**（UIA 里 `Name` 为空，只有 class `nav-sidebar-item` + 图标 class `lucide-settings-icon`），按名字找「设置」永远找不到。
  ⇒ 已把脚本改成按图标 class 定位。
- 更新日志模态框在 webview 渲染后才出现（首轮 UIA 树只有 12 个元素），必须**等树填充**再关；关法：优先找「关闭」按钮走 `InvokePattern`（本轮实测成功），失败再退到 `Esc ×2`。
- 真实配置里 `changelog.lastSeenVersion = 3.12.1`，装 3.12.2 后模态框会自动弹，属于预期。

### 4.2 用户数据安全

验收全程「先杀进程 → 再还原」：脚本结束时打印 `PROFILE_RESTORED orig=6036 now=6036`，并用 SHA256 核对**逐字节一致**
（`08F2387306B5A215D03DF0371B5E9CECA34C08F611C595FAA4996767A20232A4`）。真实文件：
`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-settings.json`。

## 5. 打包

- 命令：`npm run tauri:build`（末步 `scripts/publish-bundle-to-release.mjs` **只把安装包复制到 `release/`，不上传**）。
- 构建脚本：`.superpowers\sdd\build-3122b.cmd` / `build-3122c.cmd`（**纯 ASCII**，Rust toolchain 路径必须是 `1.90.0-x86_64-pc-windows-msvc`，**`pc-` 不能省**）。
- 产物：`src-tauri\target\release\bundle\nsis\Alpha File Manager_3.12.2_x64-setup.exe`（45169271 B，2026-11-07 20:40:25）；`release\Alpha File Manager_3.12.2_x64-setup.exe` 已手动覆盖为同一份。
  ⚠️ `publish-bundle-to-release.mjs` 见到 `release/` 里已存在**内容不同**的同名包时，会改写到 `release\local-builds\` 而不覆盖 ⇒ 打包后要自己 `copyFileSync` 一次。
- 已装机：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe` = **3.12.2**（文件时间戳 20:39:50）。
- ⚠️ **本轮踩到的发布事故**：3.12.2 的 `CHANGELOG.md` / 14 个 locale 更新日志是 **20:28** 才落盘的，而第一次打包的 vite 前端构建在 **20:24~20:27** 就跑完了 ⇒ **装出来的包里「新功能」对话框停在 3.12.1**（UIA 实测：版本列表顶部是 3.12.1，没有 3.12.2）。
  ⇒ 结论：**`tauri build` 会在打包前跑一次前端构建，任何在此之前落盘的元数据都不会进包**。发版时必须「先写完元数据 → 再打包」，或者像本轮一样重打一次。
  重打（`build-3122c.cmd`）后复验：新包的对话框版本列表顶部已是 **「3.12.2 2026 年 11 月 当前」**（证据：`accept-defaults-3122\a-00-before-dismiss.txt`）。

## 6. 本轮工具坑（写给下一个会话）

1. **UIA 树要等 webview 渲染**：窗口 handle 出现 ≠ 元素已可枚举。刚拿到窗口时只有 12 个 Pane，要轮询等树长起来（本脚本 `Wait-Elements 40 90`）。
2. **侧边栏 / 工具栏图标按钮没有无障碍名**：UIA 取元素**不能只取有名字的**。按 `ClassName` 里的 lucide 图标 class 定位（`lucide-settings-icon`、`lucide-rotate-ccw-icon`），再往上走到 Button 祖先。
3. **无障碍名缺失的重置按钮**：`visual-effects.vue` 的重置按钮只有 Tooltip（「将视觉效果重置为默认值」），Tooltip 内容不悬停就不进 UIA 树 ⇒ 按「视觉效果」标题同行右侧最左的 Button 判定，或直接找 `rotate-ccw` 图标祖先。
4. **`SendInput` 的 `INPUT` 结构必须显式 `[StructLayout(LayoutKind.Explicit, Size = 40)]`**，只声明 `KEYBDINPUT` 会让 `Marshal.SizeOf` < 40 ⇒ 静默失败（返回 0，不抛异常），表现为「键打不进去」。本脚本已按 40 字节写死。
5. **`PowerShell` 函数里有返回值就不能混 `Write-Output`**：诊断输出会进管道变成返回值的一部分，脚本会在第一条断言前静默退出。函数内一律用 `Write-Host`。
6. **`git stash push -u` 必须放在 `cd` 之前**（本地安全策略会拦截「cd 后写/删目标」）；`stash pop` 放 cd 之后可以。日志不要重定向到文件，用 `2>&1 | Select-String`。
7. **`.cmd` 的 `echo` 里不能出现 `>`**（会被当重定向符，可能在仓库根目录造出垃圾文件）；描述性文字用 `to` / `=>`。
8. **`publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 下所有历史安装包**：遇到内容不同的旧包会改写到 `release\local-builds\`，不是失败；但 `release/` 里有被占用的旧包时整步会 exit 1 —— 那时**新包已经打好了**，单独 `copyFileSync` 补进 `release/` 即可，不必重打包。

## 7. 本轮收尾状态与后续

已完成：

1. ✅ `build-3122c.cmd` 重打完成（20:40:25，45169271 B），装到 `D:\Program Files\Alpha File Manager\`（3.12.2）。
2. ✅ 阶段 A（全新用户数据首次启动）与阶段 B（点「将视觉效果重置为默认值」）在**最终打包产物**上各跑一遍，**均 PASS 9/9**。
3. ✅ 第三个提交 `6e68d337`（23 个文件：4 个版本文件 + `Cargo.lock` + 18 个发版元数据文件 + `_notes-3.12.2.md`）。
4. ✅ 交接卡（本文件）+ 接力 prompt（`docs/prompt-2026-11-07-ui-default-settings-release-next.md`）已落盘。

未做 / 仍待处理（**线上发布已在 §7.2 完成，本节旧版本里「不做任何线上发布动作」的说法已作废，不要再拿它去问用户要不要发布**）：

- ✅ **线上发布已全部完成**：站点 `version.json` → 3.12.2、GitHub Release `v3.12.2` 已建、公网回读三项一致 —— 详见 §7.2。
- **winget 清单未做**：3.12.2 没有提交到 winget 仓库。参考 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md`。
- `extensions/kizemo.focus-sync/sidecar/src/bin/` 继续**刻意不入库**（沿用 3.12.1 的决策）。
- 存量安装**不会自动迁移**到新的视觉效果默认值 —— 本轮明确的产品决定（需求只要求「新装」+「重置为默认」），CHANGELOG 与 `_notes-3.12.2.md` 已写明。若将来要做版本化迁移，需另起一轮设计（迁移键 + 幂等 + 老版本降级回退）。
- `release\Alpha File Manager_3.12.2_x64-setup.exe`（根目录 = 最终新包，45169271 B，20:40:25，sha256 `47e08c97…`）与 `release\local-builds\Alpha File Manager_3.12.2_x64-setup.exe`（打包脚本在内容不同时自动改写的旧包副本）同时存在，二者内容不同属正常。**对外发布用的是根目录那份**，已逐字节核对。

## 7.1 收尾补记（本次会话结束前）

- 外层仓 `F:/soft/00selfmade/filemanager/AGENTS.md` 的两条新铁律（连同前几轮遗留未提交的项目硬约束，共 62 行）已提交：**`355aab1`**（外层仓 `main`）。该提交**只含 `AGENTS.md` 一个文件**，未卷入 `docs/extension-changelog.md` 与其它未跟踪文件。
- 内层仓工作区仍只余一个刻意不入库的未跟踪目录：`extensions/kizemo.focus-sync/sidecar/src/bin/`（沿用 3.12.1 决策）。
- `release/` 下 3.12.2 仍有两份安装包：根目录 = 最终新包（45169271 B，20:40:25，含 3.12.2 更新日志）；`release/local-builds/` = 打包脚本自动改写的旧包副本。对外发布前请人工确认取用**根目录那份**。

## 7.2 3.12.2 对外发布（已完成）

- **git push**：内层仓 `main` 由 `b49fbd31` 推到 `ca2215bb`（origin/main）；标签 `v3.12.2` 指向 **`6e68d337`（发版提交）**，与 `v3.12.1` 的惯例一致（tag = 发版提交，不是 HEAD）。
- **站点版本清单**：`node scripts/release-publish.mjs prepare --bump none --skip-build --notes-file _notes-3.12.2.md` ⇒ `E:\办公文件\L1网站\alpha-file-manager\version.json` 变为 **3.12.2** / `sha256 47e08c97…` / `pub_date 2026-11-07T13:43:02Z`；`history[]` 首条归档 3.12.1（站点上挂着的那一版被顶掉时才归档，与仓库版本号无关）。站点说明文字直接取仓库的 `_notes-3.12.2.md`（已核对：3.12.1 的站点 notes 与 `_notes-3.12.1.md` 逐字相同）。
- **上传顺序（别改）**：① 先把 45 MB 安装包单独 put 到服务器（paramiko 单文件 + 远端 `md5sum`/`stat` 校验，脚本 `.superpowers/sdd/upload-one.py`）⇒ `REMOTE_MATCH`；② 再传 `index.html` / `.htaccess` / `version.json` / `download.php`（`_upload_alpha_files.py --files-only`，四个文件远端 md5+size 全部 OK）。反过来会把 version.json 先翻到 3.12.2，让用户点到还没传完的包。
- **GitHub Release**：`v3.12.2` 已创建（`kizemo/alpha-file-manager`），资产 `Alpha_File_Manager_3.12.2_x64-setup.exe`，45169271 字节，官方 digest `sha256:47e08c976e8906c2848623a4a7f6cbfdd80cea8833607904fe7efd1614280cd4`（与本地包逐字节一致），`draft=false` / `prerelease=false`。
- **公网回读**：`node scripts/release-publish.mjs verify` ⇒ 线上版本 3.12.2，**版本号 / 大小 / sha256 三项一致**，源站与 Cloudflare 边缘（`CF-Cache-Status=MISS`）都已生效。
- **本轮未做**：winget 清单；外层仓 `filemanager` 的 push —— 远端 `main` 被另一会话 force 改写（Focus Sync 0.5.8/0.5.9 发布线），本地现为 **ahead 82 / behind 2**，变基在 `README.md` 上 add/add 冲突，已 `rebase --abort` 保持本地原状；`355aab1` 与 `d3f9f44` 两条 AGENTS.md 提交**只在本地**。

## 8. 下阶段计划（**范围尚未由用户定义**）

开工前先向用户确认本轮范围。以下按「能不能直接做」排序：前两项可直接做，第 3 项是阻塞（需决策），第 4 项等需求。

1. **winget 清单**（可直接做）。3.12.2 未提交 winget-pkgs；做法参考 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-02-phase4-winget-and-search-scan.md`。
2. **`release/` 目录收敛**（可直接做，低风险）。`release/SHA256SUMS.txt` / `LOCAL-BUILDS.txt` 只到 3.9.2，属既有状况、`release/` 被 gitignore，本轮未动。若要统一，先归档 `release/local-builds/` 里的历史副本，否则重跑 `publish-bundle-to-release.mjs` 会再次改写。
3. **外层仓 push 阻塞（需属主决策，禁止自行 force push）**。可选路径：(a) 交给属主合并；(b) `git -C F:\soft\00selfmade\filemanager push origin d3f9f44:refs/heads/agents-docs` 另开分支保住两条 AGENTS.md 提交；(c) 保持本地不动。
4. **新功能需求**：未定义，等用户给。

已完成、**不要再做一遍**：3.12.2 代码提交、版本号对齐、发版元数据、打包、装机、两阶段真机验收、站点上传、GitHub Release、公网回读。

## 9. 必读文档与证据路径（全路径）

按顺序读：

1. 本文件 —— `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-07-ui-default-settings-release.md`
2. 本轮起点交接卡 —— `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-07-ui-default-settings.md`
3. 仓库硬约束总表（必读）—— `F:\soft\00selfmade\filemanager\AGENTS.md`：含 `.cmd`/`.ps1` 纯 ASCII、只有 `tauri build` 产物可做界面验收、真机数据「先杀进程 → 再 restore」、UIA 坐标必须 `SetProcessDPIAware()`、后台构建不挂 stdout 管道等铁律。
4. reveal 缺陷排查与归档 —— `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\work-2026-11-07-reveal-shortcut-target.md`、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-07-reveal-shortcut-target.md`
5. 发版说明源 —— `F:\soft\00selfmade\filemanager\sigma-file-manager\_notes-3.12.2.md`
6. 上一轮接力 prompt（格式参考，已被本轮 `docs\prompt-2026-11-07-relay-next.md` 取代）—— `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\prompt-2026-11-07-ui-default-settings-release-next.md`

证据与脚本：

- 真机验收脚本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-accept-defaults2.ps1`（纯 ASCII，中文串走命令行参数）
- 验收证据目录：`F:\soft\00selfmade\filemanager\.superpowers\sdd\accept-defaults-3122\`（阶段 A/B 读数 txt、三张 PNG、`user-settings.json` 原始备份）
- 规格图副本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\ui-defaults-spec-20261107.png`
- 构建脚本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\build-3122c.cmd`
- 单文件上传脚本：`F:\soft\00selfmade\filemanager\.superpowers\sdd\upload-one.py`
- 站点源目录：`E:\办公文件\L1网站\alpha-file-manager\`（`version.json` / `index.html` / `download.php` / `_upload_alpha_files.py` / `_release-notes-v*.md`）
- SFTP 凭据：`E:\办公文件\L1网站\.vscode\sftp.json`（**勿外传**）；上传 venv：`E:\办公文件\L1网站\.venv-sftp\Scripts\python.exe`
- 用户数据：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-settings.json`

结论可信度：

- **已验证（有硬证据）**：两阶段真机验收 9/9（最终打包产物，非 dev）；`eslint .` 基线 180 → 177 零新增；站点 / GitHub Release / 公网回读三项一致；tag 与 HEAD 关系；两份 release 包 sha256 一致。
- **未验证 / 无结论**：winget 清单不存在；存量用户迁移未实现（是设计缺口，不是缺陷）。
