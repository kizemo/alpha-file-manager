# 交接文档 · 阶段五批次 3（动作注册表接入右键菜单 / 扩展命令 API）

- 日期：2026-11-03（本版为**收尾版**，覆盖此前同一文件的中间结论）
- 仓库：`F:\soft\00selfmade\filemanager\sigma-file-manager`（仓库根 `F:\soft\00selfmade\filemanager`）
- 版本：**3.9.2**（`package.json` / `src-tauri/Cargo.toml` / `src-tauri/tauri.conf.json` 三处一致，本轮未改）
- 基线提交：`ceff7e38 docs(handoff): 新增阶段四交接文档并更新接力 prompt`
- 工作树状态：**批次 1–3 + permfix + L1–L3 全部未提交**；`src/core/` 整目录 untracked
- 本轮结论：**代码完成、门禁无回归、真机验收 `pass=23 fail=0`、用户数据完好**；剩下的事项全部需要用户本人操作或裁决（见第八节）

## 一、背景与目标

阶段五的目标是把「文件操作动作」统一收敛到**动作注册表**（`src/core/actions/`），
让 UI 入口、快捷键、扩展命令 API 共用同一份动作定义与可见性判定。
批次 3 是最后一块 UI 拼图：把**文件条目右键菜单**（含顶部 quick-actions 图标条）接到注册表，
并把**扩展命令 API** 也接到同一份动作定义上。

本轮同时完成 L1–L3（默认文件管理器改造），并修复 permfix 相关的删除重复提交问题。

## 二、本轮改动清单

### L1–L3 默认资源管理器改造（已实现 + 已有单测）

- **L1**：`.lnk` 快捷方式解析。只对指向**目录**的快捷方式生效；断链 ⇒ 回落选中 `.lnk` 自身，不报错。
  落点：`src/composables/use-init.ts` 的 `getDirEntry`。
- **L2**：默认文件管理器改道。判据 `await isDefaultFileManagerSafe()` 为真 ⇒ 应用内定位；为假或读取失败
  ⇒ 维持 Explorer。「显示但置灰」口径不变。应用内定位必须走「当前这一格」：`reuseCurrentPaneOnly: true`。
- **L3**：参数编码。launcher 改写成 `--sigma-select=<路径>`；前端兼容 `--sigma-select=` / `/select,` / `/select `；
  Explorer 兜底用 `/select,<路径>`。select 语义 = 「父目录 + 选中条目」，与普通「打开这个目录」不同；
  select **不混进** `getLaunchDirectoryCandidates`，纯 select 也算「有启动目标」。
- 新增 crate `sigma-file-manager-launcher`（**不依赖 tauri**，不会触发 `cargo test --lib` 的链接红线）。

### 批次 3（已实现 + 已有单测）

- 文件条目右键菜单接到动作注册表（`src/core/actions/`）。
- quick-actions 图标条接到注册表。
- 扩展命令 API 接到同一份动作定义。
- 六条裁决：两个 open-containing-directory 动作**不合一**；`registry.ts` 不改；
  「显示但置灰」落新增菜单适配器（`availability==='disabled'` 显示置灰、`hidden` 才不显示）；
  remote 四动作不进 `CONTEXT_MENU_ITEMS`；复用 `sigma.shortcut.<id>` 命名空间；
  `createCommands()` 旧签名必须保留。
- 范围：**只接文件条目右键菜单**，目录树 / 驱动器卡未接（待用户拍板是否扩范围）。

### permfix（删除进行中对同一路径重复提交）

- 前端 attach 分支只有源码契约，无行为测试（未验证项，见第五节）。

## 三、门禁结论（口径：`eslint .` 整仓；基线用 `git stash push -u` 现量 HEAD）

**以下数字全部量自「临时埋点已摘除 + `b5-build8` 干净产物同源」的同一份工作树**，
日志 `.superpowers/sdd/gate8-node.log`（脚本 `b5-gate8.cmd`）：

| 项 | 结果 | 是否回归 |
|---|---|---|
| `vue-tsc --build` | **TSC_EXIT=0** | 无 |
| `eslint .` 工作树 | **186 problems（186 errors / 0 warnings）** | **零回归**（HEAD 基线同为 186） |
| `cargo test --lib` | **880 passed / 0 failed** | 无 |
| `cargo test -p sigma-file-manager-launcher` | exit 0 | 无 |
| vitest 全量 | **VITEST_EXIT=1 — 19 failed / 2579 passed（2598）；Test Files 4 failed / 277 passed（281）；81.54s** | **无本轮回归，全部为存量红** |

vitest 的 19 条红分两类，**都不是本轮改动引起**：

1. **7 条已知基线红**：`use-file-browser-navigation.test.ts` 5 条、`module-load-recovery.test.ts` 1 条、
   `readme-markdown-table.test.ts` 1 条（用户未提交的 docs 表格列数不一致引起）。
2. **12 条存量测试缺口**：`file-browser-tree-view.vue` 相关用例
   `SyntaxError: Need to install with 'app.use' function`。证据链：
   - 该 `.vue` 在 **`HEAD:44` 已有 `import { useI18n }`、`HEAD:46` 已有 `const { t } = useI18n();`**；
   - 工作区该文件 `DIFF_LINES=0`（与 HEAD 逐字相同），测试文件 `git status` 未修改；
   - `git diff --name-only` 中**没有任何** `i18n|vitest|setup` 文件。
   - 根因：测试只 stub 了 `FileBrowserRenameDialog`，覆盖不到 tree-view 自身的 `useI18n()`，
     虚拟机未装 `vue-i18n` plugin。
   - ⇒ 这是**存量测试基础设施缺口**，与本轮改动无关。

> ⚠️ 口径提醒：上一份交接里记的 `7 failed / 1077 passed (1084)` 与本次 `19 failed / 2579 passed (2598)`
> **不是同一口径**（用例总数差 2.4 倍），不能当作「本轮多红了 12 条」的基线对照。
> 本轮因构建进行中无法 `git stash`，改用「与 HEAD 逐字比对 + 检查测试基础设施改动」定案。

> ⚠️ 基线数字**随提交变**，量的时候一律现量 HEAD，别拿旧数字对。
> eslint **不读 `.gitignore`**，工作副本里的 `.superpowers/` 会被一起 lint ⇒
> `eslint.config.ts` 的 `globalIgnores` 必须含 `.superpowers/**` 与 `graphify-out/**`
> （本轮已含，故工作树与干净检出同口径）。

## 四、真机验收（release 产物）

验收脚本：`.superpowers/sdd/accept-b3-actions.ps1`（**现 910 行**，纯 ASCII，`ASCII_NON=0`）
运行方式：`powershell.exe -NoProfile -ExecutionPolicy Bypass -File <脚本> -ExePath <exe>`
产物：`src-tauri\target\release\alpha-file-manager.exe`（**release 产物，不是 `tauri dev`**）

### 最终结果：第 6 轮 `SUMMARY pass=23 fail=0`

- 运行窗口：2026-11-03 22:54:01 → 22:55:12（`accept-b3-actions.log` 最后写入时间）
- 所跑产物：`b5-build8`（exe 80286208 B @ **22:51:28**，晚于全部源码改动 ⇒ 无「旧二进制」风险）
- G 段自证：三件用户数据 `restored=True identical=True`，
  `verify` 报 backupBytes / liveBytes = **5382 / 1854 / 31979**，`verified=True`

| 判据 | 内容 | 结果 |
|---|---|---|
| B1 | 沙箱内不干扰操作者桌面（前台窗口未被抢走） | PASS |
| B2 | release 产物能列出夹具目录内容 | PASS |
| B3 | 鼠标注入能到达 webview（阳性对照） | PASS |
| B4 | 文件条目右键菜单能打开 | PASS |
| B5 | 文件条目菜单渲染出完整动作列表 | PASS（13 项） |
| B6 | Windows 专属的「打开文件所在目录」出现在文件菜单 | PASS |
| B7 | 「打开所属目录」在**普通目录视图**里保持隐藏（护栏） | PASS |
| B8 | quick-actions 图标条渲染（5 个无访问名图标按钮） | PASS |
| C1 | 目录条目右键菜单能打开 | PASS |
| C2 | 目录条目菜单渲染 | PASS（12 项） |
| C3 | 菜单投影随条目上下文变化（文件 13 项 vs 目录 12 项，差 5） | PASS |
| C4 | 目录条目不提供「打开文件所在目录」 | PASS |
| D1 | `.lnk` 条目能渲染右键菜单 | PASS（13 项） |
| D2 | **双击 `.lnk` ⇒ 应用内导航到目标目录 `afm-b3-target`**（L1） | PASS（target visible=True） |
| E1 | 工具栏按钮能打开命令面板 | PASS（palette visible=True） |
| F1 | `--sigma-select=` 启动落到 select 目标目录（L3） | PASS（seen=True） |
| F2 | select 启动覆盖已保存的工作区目录 | PASS（见 `afm-b3-select`、不见 `alpha.txt`） |
| G1 | 三件真实用户数据按字节恢复 | PASS（verified=true） |

实测菜单内容（release 产物，中文界面）：

- 文件条目（13 项）：添加到收藏 / 属性 Alt+Enter / 局域网共享 / 彻底删除 / 解除文件锁定 /
  复制路径到剪贴板 / 打开文件所在目录 / 打印 Ctrl+O / 快速查看 Space / 压缩 / 在终端中打开 Alt+T /
  Shell 扩展 / 打开方式...
- 目录条目（12 项）：与文件条目不同（多「新的」「在新标签页中打开」，少「打开文件所在目录」「打印」「快速查看」）
- `.lnk` 条目（13 项）：与文件条目一致

### 本轮共修掉 4 个「假缺陷」——全部是**验收脚本**的问题，没有一处改产品代码

| # | 现象 | 真因 | 处置 |
|---|---|---|---|
| 1 | B7「打开所属目录」不出现 | 该动作按既有设计**只出现在搜索结果视图**（`context-menu-action-visibility.ts:128` 的 `SEARCH_RESULTS_ONLY_ACTIONS`，单测 `:180` 明确断言普通视图不可见） | 断言改成反向护栏 |
| 2 | C 阶段找不到 `nested` 目录 | Radix 下拉菜单打开时给非菜单内容加 `aria-hidden`，Chromium 把文件列表整块**丢出可访问性树** | 枚举前先点文件行关菜单 |
| 3 | **F1 连续三轮 FAIL** | 验收脚本 helper 的形参写成了 `$args` —— **PowerShell 自动变量**，绑定值被吞为空 ⇒ 子进程命令行只剩 exe 路径，`--sigma-select=` 从未进入进程 | 改名为 `$launchArgs`（`accept-b3-actions.ps1:413-421`） |
| 4 | **第 5 轮 D1 / E1 FAIL** | C 段打开目录右键菜单后**没有关**，菜单开着的整段时间里文件列表与工具栏都被 `aria-hidden` ⇒ D1 找不到 `target-link.lnk`、E1 枚举不到顶部 `Button`（链式：D 段被跳过后就不会执行后面的关闭点击） | C 段 capture 之后补「关菜单」点击；D1 查找与 E1 按钮枚举各包 3 次轮询重试（脚本 885 → 910 行） |

> F1 的取证闭环值得记住：先用**应用日志时间戳**证明 F 阶段实例确实起来了、且当时无第二实例，
> 再用**文件修改时间**证明所跑的 exe 早于 L3 源码 —— 两条硬证据把「产品缺陷」排除掉，
> 才把怀疑转向脚本自身。

### 复验的两条硬前置（release 构建下）

- **「隔离沙箱」在 release 里并不隔离**：`src-tauri/src/app_dirs.rs:81-89` 的
  `isolated_root_from_env_value` 带 `#[cfg(debug_assertions)]` ⇒ `SFM_E2E_DATA_DIR` 覆盖被**编译掉**，
  沙箱实例与用户正式版**共享同一份 user-data**。
- **release 里单实例插件是装的**：`src-tauri/src/lib.rs:333-335` 仅在 `!app_dirs::is_isolated()` 时装载
  ⇒ release 下恒为装 ⇒ **机器上不能有第二个实例**（否则新实例把参数交给它并自己退出）。

## 五、已验证 / 推断 / 未验证

### 已验证（有证据）

- `vue-tsc --build` TSC_EXIT=0；`eslint .` 186 = HEAD 基线 186（零回归）；
  `cargo test --lib` 880 passed / 0 failed；launcher crate exit 0。
- build8 为**埋点摘除后的干净产物**：exe 80286208 B @ 22:51:28，比带埋点版本小 84992 B，与摘除量一致；
  构建日志 `BUILD_EXIT=0`。
- release 产物上真机验收 **23 判据全 PASS**（含 F1/F2/D1/D2/E1），G1 用户数据 `verified=true`。
- 文件条目菜单 13 项、目录条目 12 项、`.lnk` 条目 13 项；quick-actions 图标条 5 个无访问名按钮。
- 菜单里的「打开文件所在目录」（Windows 专属）确实出现在文件条目菜单中。
- **双击 `.lnk` ⇒ 应用内落到 `afm-b3-target`**（L1 成立）。
- **`--sigma-select=` 启动确实落到 select 目标并覆盖已保存工作区**（L3 成立）。
- 三件真实用户数据按字节恢复（5382 / 1854 / 31979）。
- **release 构建也有应用日志**（此前「拿不到日志」是错误结论，见第七节第 12 条）。

### 推断（证据一致但未直接观测）

- `open-containing-directory-in-app`（打开所属目录）只出现在搜索结果视图 ——
  源码谓词 + 既有单测一致，但本轮真机没有构造搜索结果视图做端到端观测。
- 12 条 tree-view vitest 失败属存量缺口 —— 依据是两文件与 HEAD 逐字相同且测试基础设施零改动
  （构建期间无法 `git stash` 做同口径对照）。

### 未验证（明确留给后续）

- **扩展命令 API 的真机行为**：需要往 `user-extensions.json` 灌扩展，而用户已裁决
  「focus-sync 同步状态确认后才动该文件」⇒ 目前只有单测证据。
- **permfix B1–B3**：物理键盘长按回车复现「删除进行中对同一路径重复提交」缺真机证据
  （沙箱桌面无键盘输入，Agent 无法代做）。
- **物理键盘 Ctrl+Shift+P 端到端**（命令面板）：同样需要用户本人操作。
- 前端 attach 分支只有源码契约，无行为测试。
- 11.8 秒删除耗时去向未钉死。

## 六、证据路径

- 本轮结论汇总：`.superpowers/sdd/b5-findings.md`
- 验收脚本：`.superpowers/sdd/accept-b3-actions.ps1`（910 行，纯 ASCII）
- 夹具脚本：`.superpowers/sdd/b3-fixture.cjs`（`backup | seed | read | restore | verify`）
- **验收日志**：`.superpowers/sdd/accept-b3-actions.log`（第 6 轮，`SUMMARY pass=23 fail=0`，22:55:12 收尾）
- 截图目录：`.superpowers/sdd/accept-b3-shots/`
- **打包日志**：`.superpowers/sdd/b5-build8.log`（`BUILD_EXIT=0`，`Finished release profile in 4m 12s`）；
  历史：`b5-build5.log`（导致 F1 假失败的旧产物）、`b5-build6.log`
- **最终产物（2026-11-03 22:51）**：
  - `src-tauri\target\release\alpha-file-manager.exe` = **80286208 B @ 22:51:28**
  - `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.9.2_x64-setup.exe` = **44799979 B @ 22:51:26**
  - 同包已复制到 `release\local-builds\Alpha File Manager_3.9.2_x64-setup.exe`（44799979 B）
  - SHA256 = `F075CACEDF8E63B9F6A79A439312D78299956FB9ECC1E857E863688BA864300C`
- 新前端产物含 L3 逻辑：`dist\assets\workspaces-*.js` 命中 `sigma-select`
  （`dist` 被 gitignore ⇒ Grep 工具搜会「无结果」，必须用 `Select-String` 走文件系统）
- 门禁日志：`.superpowers/sdd/gate8-node.log`（本轮）、`b5-gate.log`、`b5-eslint-base2.log`、
  `b5-eslint-work.log`、`b5-cargo-lib.log`、`cargo9-lib.log`、`cargo10-lib.log`
- **用户数据备份快照**：`.superpowers/sdd/b3-backup-20261103-2233/`（5382 / 1854 / 31979，与 live 逐字节一致）
- 首轮验收日志备份：`accept-b3-actions-run1.log`（5007 B）；重跑会覆盖 `accept-b3-actions.log`
- F1 复验 runbook：`.superpowers/sdd/b5-reverify-f1.md`
- **应用日志**（release 也有）：`%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log`、
  同目录 `logs\startup-probe.log`（时间戳 **UTC**）
- 阶段五计划：`docs/plan-2026-11-02-action-registry-and-incremental-index.md`
- 上一份交接：`docs/handoff-2026-11-03-stage5-permfix-l123.md`
- 批次 1 / 2 交接：`docs/handoff-2026-11-03-phase5-batch1-action-registry.md`、
  `docs/handoff-2026-11-03-phase5-batch2-shortcut-action-ids.md`
- 接力 prompt：`.superpowers/sdd/relay-prompt-20261103-batch3.md`

## 七、本轮踩到的坑（可复用）

1. **PowerShell 函数形参不能叫 `$args`**：它是自动变量，绑定值被吞为空，子进程命令行悄悄少掉参数。
   症状是「参数明明拼对了却没进进程」。这一条让 F1 白跑了三轮验收。
2. **Radix 下拉菜单打开时，背景内容被 `aria-hidden` 摘出 Chromium 的可访问性树** ⇒
   菜单开着时枚举 UIA 树**看不到背后的文件列表与工具栏**。凡是「菜单打开后再枚举」的断言，
   必须先关菜单（点一下菜单外的元素触发 `DismissableLayer`）。
3. **一张验收脚本里的状态会跨段传递**：C 段忘了关菜单，锅要记在后面的 D1 / E1 上。
   分段断言之间必须显式复位（关菜单 / 关面板），别指望「后面的段会顺手关掉」。
4. **验收断言要跟产品设计对齐**：`open-containing-directory-in-app` 是**搜索结果视图专属**
   （`SEARCH_RESULTS_ONLY_ACTIONS`），普通目录视图里恒隐藏且**这是正确行为**。
   **断言写错 ≠ 产品有 bug**，先查源码谓词。
5. **eslint 基线必须现量、必须同口径**：`eslint .` 整仓；`eslint src` 会凭空少 7 条存量违规。
   `eslint --fix` 单文件通过 ≠ 回到基线，**必须再跑一次全量对照**。
6. **验收夹具不要建在 `%TEMP%` 下**：`global_search/ignore.rs` 的 `builtin_ignored_paths()`
   含 `/AppData/Local/Temp`，夹具放那儿会被整棵剪掉，搜索命中恒为 0。
7. **用户数据恢复顺序**：先杀进程（`alpha-file-manager` 计数必须为 0）→ 再 `restore` → 再 `verify`。
   应用退出时会把内存状态写回真实文件，进程还活着就 restore 会被再写回一次。
   另：`b3-fixture.cjs backup` 的语义是「**备份不存在才建**」，不会覆盖旧备份 ⇒ 跑完要看它打印的字节数。
8. **`.cmd` / `.ps1` 必须纯 ASCII**：cmd.exe 按系统 ANSI 读 `.cmd`，PowerShell 5.1 按 ANSI 读无 BOM 的 `.ps1`
   ⇒ 中文注释会变乱码甚至被当成命令执行。中文一律用 `[char]` 码点拼。
   改完 `.ps1` 必做双自检：`[^\x00-\x7F]` 计数为 0 + `[scriptblock]::Create()` 能解析。
9. **`.cmd` 的 `echo` 里不能出现 `>`**：cmd.exe 把它当重定向符，句子被截断还会生成垃圾文件。
10. **`.cmd` 运行期间绝对不能编辑**：cmd.exe 按字节偏移逐行读，改文件会让后续指令错位执行
    （曾把版本号从 3.6.0 顶到 3.7.0 并丢掉十几分钟构建产物）。要改就先停任务，另存新文件名再跑。
11. **长编译不要挂 stdout 管道**：`Tee-Object` / `| Select-String` 一断，进程树跟着死 ⇒
    产物没落地。`.cmd` 内部用 `> logfile 2>&1` 重定向到文件。
    另：`Start-Process` 起的 detached `.cmd` 有时来不及写最后的 `echo ..._EXIT=` 行，
    判据要同时看日志里的 `test result:` / `BUILD_EXIT=`，不能只认收尾标记。
12. **「release 拿不到应用日志」是错误结论**：`tauri_plugin_log::Builder::default()` 本身带 LogDir 目标，
    只有 `log_dir_override()` 存在时才额外加 Stdout+Folder 目标。
    ⇒ 日志在 `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\`；**时间戳是 UTC**（本地 −8h）。
13. **验收产物必须是「改动之后」重新打的包**：F1 假失败的根因之一就是跑到了上一轮的旧 exe。
    改完前端/后端后务必重新 `tauri build`，并用 `Get-Item exe | LastWriteTime` 与源码时间戳逐个对照。
14. **release 构建下 `SFM_E2E_DATA_DIR` 无效 + 单实例插件生效** ⇒ 沙箱实例与用户正式版
    共享 user-data 且**不能同时存在两个实例**。真机验收前必须先确认用户已关闭正式版。
15. **先分清 `src-tauri/src` 里的埋点改动与真实实现**：`init-trace.ts` / `lib.rs` / `dir_reader/commands.rs`
    三处只有埋点 ⇒ `git checkout --` 精确还原；`use-init.ts` 混有 L3 真实实现 ⇒ **只能手工摘埋点**，
    摘完要用 `git diff` 核对「只剩 L3」。

## 八、下阶段计划

1. **交棒后第一件事：把改动提交**（本轮按纪律未 commit）。`src/core/` 整目录 untracked，别漏 `git add`。
2. 用户裁决后，把批次 3 的接线扩到目录树 / 驱动器卡（当前只接了文件条目右键菜单）。
3. 用户确认 focus-sync 同步状态后，往 `user-extensions.json` 灌一个带 commands 的扩展，
   补扩展命令 API 的真机证据。
4. 用户本人用物理键盘补 permfix B1–B3 与 Ctrl+Shift+P 的端到端证据。
5. 任务 D「看一眼仪表盘」由用户本人执行。
6. `docs/handoff-2026-11-01-relay-four-tasks.md` 的 markdown 表格列数不一致
   （`readme-markdown-table.test.ts` 失败：第 235 行 3 列 / 表头 2 列，第 541 行 5 列 / 表头 3 列，
   第 863 行 5 列 / 表头 3 列）—— 该文档是用户声明「不动不提交」的文件，需用户裁决。
   这一条同时也是 vitest 7 条基线红里的 1 条，修掉它 vitest 可少 1 红。
7. winget PR `https://github.com/microsoft/winget-pkgs/pull/460774` 等版主 `Moderator-Approved`。
8. 若要把 vitest 拉回全绿：修 12 条 tree-view 用例（补 `vue-i18n` plugin 安装或补 stub），
   但要动存量测试文件，需先与用户确认收益。

## 九、一句话接力 prompt

见 `.superpowers/sdd/relay-prompt-20261103-batch3.md`（只给全路径指引，不复述本文件内容）。