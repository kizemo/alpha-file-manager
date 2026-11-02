# 交接卡 · 2026-11-01 · README 换新图标 + E:\r 删除卡死修复

> ⚠️ **本卡已降级为历史指针，权威交接源是另一个文件：**
> `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-relay-four-tasks.md`
> （2026-11-01 晚间新增，覆盖本卡的两个任务并额外纳入「全盘搜索慢」「打开文件所在位置无响应」两个新任务）。
> **新会话请以那份文档为唯一入口**，本卡仅供追溯早期过程；两者冲突时以新文档为准。

> **本卡是给「下一个新会话」的接力材料。** 新会话开工前请按第 8 节的清单顺序读文档，
> 再按第 4、5 节执行两个任务。本卡里所有事实都标注了「已验证 / 推断」，未验证的不要当结论用。
>
> **一句话接力 prompt（给用户复制用，见第 9 节）** 只引导读文档，不重复本卡内容。

---

## 1. 项目背景（30 秒版）

- 产品：**Alpha File Manager（AFM）**，Tauri 2 + Vue 3 + Rust 桌面文件管理器。
- 代码只在**内层仓** `F:\soft\00selfmade\filemanager\sigma-file-manager`（remote `kizemo/alpha-file-manager`）。
  外层 `F:\soft\00selfmade\filemanager`（remote `kizemo/focus-sync`）**不要动**。
- 当前版本 **3.9.1 已完整上线**（站点 + GitHub release `v3.9.1` + winget 清单已切 3.9.1）。
  `origin/main` = `bc918a79`，工作树在写本卡时干净（本卡自身未提交，新会话可随自己的改动一起提交）。
- **winget PR #467381 仍 OPEN**（`mergeable_state=blocked`，`reviewDecision=REVIEW_REQUIRED`），
  标题已是 `New package: kizemo.AlphaFileManager version 3.9.1`，head sha `5f453b2508`。
  - **当前 labels = `Azure-Pipeline-Passed` + `Validation-Completed` + `Needs-CLA` + `New-Package`**
    （2026-11-01 09:17Z 实测）。
    - 时间线：`03:14Z` 上游先给 3.9.0 清单打过通过标签；`05:42Z` 我把清单切到 3.9.1
      （删旧 3.9.0、加 3.9.1）后上游**摘掉**了 `Azure-Pipeline-Passed` / `Validation-Completed`；
      **`09:16:30Z` / `09:16:31Z` 上游已重新加回这两个标签** ⇒ **3.9.1 清单的 Azure 验证已通过**。
    - ⚠️ 查 Azure DevOps 的 `WinGetSvc-Validation`（def14）**看不到**本 PR 分支的构建
      （最近记录全是 2026-08-10 的 master 批次），但上游标签是权威信号，**以标签为准**。
    - GitHub Actions 侧 5 个 workflow（Manifest Validation Diagnosis / Wingetbot PR Triage /
      Transient Security Explanation / Missing Dependency Assist / Domain Validation Assist）
      在 head sha 上仍是 `conclusion=skipped`（这些是 AI 辅助诊断类，不影响放行）。
  - `license/cla` check 在 head sha 上 `conclusion=success`（`All CLA requirements met.`），
    **但 `Needs-CLA` 标签仍在**，且 issue comments 里**没有**任何 `agree` 签署评论
    ⇒ 该 check 的 success 不代表 winget 流程已放行，**仍须账号本人签**：
    `gh pr comment 467381 --repo microsoft/winget-pkgs --body "@microsoft-github-policy-service agree"`。
  - 社区审核：上游 bot 已在 `09:17:16Z` 再次留言「需要 moderator 批准」，目前**无任何 review**、
    无 `Moderator-Approved` 标签 ⇒ 这一关只能等社区志愿者。
  - 本会话挂了自动复查 reminder（每 3 小时一次，检查 winget PR 标签与评审进展）；
    新会话如已过期可忽略；**签完 CLA 后建议自己再查一次**。
- 下一版版本号是 **3.9.2**（3.9.1 已发布，不可覆盖重发）。
- 项目硬约束全在 `F:\soft\00selfmade\filemanager\AGENTS.md`（运行时注入版末尾会被截断，需要时直接 read 源文件）。
  本卡第 7 节只列与本两个任务直接相关的红线。

## 2. 上一轮已完成（不要重做）

交接卡 `docs/handoff-2026-11-01-folder-tree-toggle-persistence.md` 的 T1–T4 全部完成并写回该卡第 7 节（提交 `bc918a79`）：
目录树开关持久化缺陷修复 → 出货 exe 沙箱真机验收通过（`rel-391b`）→ 真实用户数据恢复并 sha256 核对 MATCH →
推送 → 发布 3.9.1（安装包 44,775,751 B，sha256 `67996F5B…DCF2`）→ winget 清单切 3.9.1。
唯一遗留：winget CLA 需账号本人签署（见上）。

## 3. 本轮（写卡会话）已完成的侦查 —— 全部只读，未改任何代码

### 3.1 图标（任务 1 的全部前置事实）

| 文件 | 大小 | 时间 | 实际配色（**已看图确认**） |
|---|---:|---|---|
| `.github/media/alpha-file-manager-logo.png` | 41,379 B | 2026-10-09 18:51:54 | **旧 logo**（README 现用，sha256 前缀 `EC94F2A63183E750`） |
| `src/assets/icons/logo-1024x1024.png` | 31,145 B | 2026-11-01 09:20:48 | **深棕文件夹 + 米白 α** ← 用户要的「深色文件夹 + 浅色字符」 |
| `src/assets/icons/logo-1024x1024-dark.png` | 30,914 B | 2026-11-01 09:20:48 | 米黄文件夹 + 深棕 α（**文件名里的 dark 指字符颜色，不是文件夹颜色，别按名字选**） |
| `src-tauri/icons/icon.png` | 25,746 B | 2026-11-01 09:20:48 | 米黄文件夹 + 深棕 α（安装器/任务栏实际在用） |
| `.github/media/logo-1024x1024.png` | 26,463 B | 2026-10-09 | 旧图，引用情况未查（动它前先 grep） |

- README 引用点（已验证，内容逐字相同，都在 **L14**）：
  `README.md`、`README.zh-CN.md` →
  `<img valign="middle" src="./.github/media/alpha-file-manager-logo.png" width="56px" alt="Alpha File Manager">`
- 同路径还被两处历史文档引用（已验证）：`docs/handoff-2026-10-24-github-presence.md` L113、
  `docs/handoff-2026-10-24-public-presence-and-winget-release.md` L46。
- `README.md` **L134 的配色描述已过期**（已验证与实物不符）：它写「安装器/任务栏用深色文件夹 + 浅色 α」，
  但实物 `src-tauri/icons/icon.png` 是浅色文件夹 + 深色 α。换图标时一并订正该句。
- **不要动**：`CHANGELOG.md` 的 50 处图片引用与 `docs/screenshots/` 的 39 张 png（历史截图/产品截图，不是图标）。
- 其它同日重绘的新图标（如需顺带核对）：`src/assets/icons/logo-32x32.png`、`src-tauri/icons/128x128.png`、
  `src-tauri/icons/128x128@2x.png`（旧 logo 当初取自最后这张）。
  旧残留（2026-09-25，未随重绘更新）：`src/assets/icons/logo-copying-32x32.png`、`public/favicon.ico`。

### 3.2 删除卡死（任务 2 的全部前置事实）

**日志（已验证）**

- 真实路径在 **Local，不是 Roaming**（Roaming 下根本没有 logs 目录，别再去找）：
  `%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log`（写卡时 1,332,245 B）。
  同目录还有 `startup-probe.log`（独立落盘通道，不依赖日志插件）。
- 读法（日志被应用独占锁）：
  `[System.IO.File]::Open($p,'Open','Read','FileShare.ReadWrite')` + `StreamReader`；`Get-Content` 亦可；
  `ReadAllText` 会抛 IOException。**日志时间戳是 UTC（比本地少 8 小时）**。
- 轮转配置（`src-tauri/src/lib.rs:600-611`）：`level=Info`、`max_file_size=2 MB`、`RotationStrategy::KeepOne`。
- **关键结论（已验证）：当前日志里没有 E:\r 删除的任何痕迹。** 整份 1.33 MB 全是 2026-11-01 本地 14:10–14:13
  沙箱验收那两次启动的记录（末条 `2026-11-01T06:12:16Z`）。事发记录已被轮转淘汰。
  ⇒ **任务 2 不能靠旧日志定根因，必须复现**；且删除链路日志极薄（见下），复现时要自己加观测。

**代码链路（已验证）**

- 前端入口 `src/modules/navigator/components/file-browser/composables/use-file-browser-selection.ts`：
  - L1545-1588 `showLargeBatchDeletePreview()`：L1562 `summary = await invoke('scan_delete_targets', { paths })`，
    **await 期间没有任何进度/加载提示**，扫描全部完成后才 `toast.custom(...)` 弹「正在删除大批量…」；
    统计失败 `catch { return }` 静默放过。
  - L1590-1601 `deleteItems()`：先 `await showLargeBatchDeletePreview(entries)`，再走真正的删除。
- 前端 job 层 `src/stores/runtime/delete-jobs.ts`：L230 `invoke('start_delete_job')`、
  L107 监听 `delete-job-progress`、L243 `cancel_delete_job`。契约测试 `src/utils/__tests__/delete-progress-contract.test.ts`。
- Rust `src-tauri/src/delete_job.rs`（471 行）：
  - `scan_delete_targets`（L239-240）：**同步**调 `delete_tree::scan_paths`，无进度、无取消入口。
    ⚠️ 新会话第一件事确认它是不是同步 `#[tauri::command]`、跑在哪个线程——若在主线程，扫描期间整个窗口冻死。
  - `start_delete_job`（L264 起）：`spawn_blocking(run_delete_blocking)`；**job 内部 L161 又扫一遍**
    （`scan_paths_with_progress`）才开始删 ⇒ 大树被**扫两遍**。
  - 事件：`delete-job-progress` / `delete-job-finished`；日志只有 started（L338）/ finished（L375）两条 info，
    **扫描阶段零日志** ⇒ 卡 scan 时从日志看「活着但什么都没发生」。
- Rust `src-tauri/src/delete_tree.rs`（1118 行）：`scan_directory_recursive` 单线程递归、逐条目 `symlink_metadata`；
  `PROGRESS_INTERVAL=100ms`、`PROGRESS_ITEM_INTERVAL=500`、`MAX_PARALLEL_WORKERS=4`；
  回收站路径走 `crate::utils::delete_path_to_trash`（`delete_paths_to_trash`）。
  文件头注释（L11）记载旧实现是「逐个顶层路径 `fs::remove_dir_all`」，已重写。
- 命令注册：`src-tauri/src/lib.rs` L428-431（`recycle_bin_unavailable_paths` / `scan_delete_targets` /
  `start_delete_job` / `cancel_delete_job`）。

**靶子现场 `E:\r`（用户真实数据，只读；已验证）**

- 结构：`E:\r\Users\Duanyi\` 下是一份完整 Windows 用户目录副本（约 52 个子目录 + 约 110 个散落文件，
  含 `.cargo` `.rustup` `.gradle` `.ollama` `.npm` `AppData` `OneDrive` `go` 等，时间戳全为 2026-09-27）。
- 浅四层（`E:\r`、`Users\Duanyi`、`AppData`、`AppData\Local|Roaming`）**reparse point 数 = 0** ⇒ junction 无限递归假设在浅层被证伪（深层未查）。
- `AppData` 下抽样 3000 文件**最长路径 252 字符**（逼近 260 MAX_PATH；更深的 `.rustup`/`.cargo` 子树可能越界，未验证）。
- **全量递归统计 600 s 超时无任何输出**（后台任务 `bg_ed81da66-c5ce-40f8-8e24-dc976844532e` failed）
  ⇒ 连枚举一遍都要 >10 分钟，规模极大。这本身就是「卡死」量级的直接证据。
- ⚠️ **不许拿 `E:\r` 当删除靶子反复试**（项目铁律：破坏性验收靶子必须自造）。

**根因假设（推断，按可能性排序，新会话逐个证伪/证实）**

1. **H1（最可能）**：点删除后前端 `await scan_delete_targets`——同步全树递归扫描、零反馈——
   对 `E:\r` 这个规模就是数分钟到十几分钟的「无响应」窗口；toast 要等扫描完才弹。
   若该同步 command 跑在主线程，则窗口级冻死；若在 worker 线程，则是「界面活着但毫无反馈」。**用复现时窗口是否响应来区分**。
2. **H2**：扫描跑两遍（preview 一遍 + job 内一遍），代价 ×2。
3. **H3**：`use_trash=true` 时逐条目走回收站 API，大树极慢且可能单线程。
4. **H4**：>260 长路径逐条目报错/重试累积。
5. **H5**：`E:\r\Users\Duanyi\OneDrive` 内云占位文件在 metadata/删除时触发网络水合而停摆。
6. **H6（观测缺陷，本身就是修复点）**：扫描阶段无日志、无进度、preview 无取消 ⇒ 卡住时既看不出活着也停不下来。

## 4. 任务 1 执行计划（GitHub 文档换新图标）

1. 选型已定：**`src/assets/icons/logo-1024x1024.png`（深棕文件夹 + 米白 α）**。不要按文件名选（见 3.1 表）。
2. 推荐落法（改动面最小，新会话可自行调整但要在汇报里说明）：
   把该 png **覆盖复制**为 `.github/media/alpha-file-manager-logo.png`（两个 README 与两处历史文档的引用路径全部不变），
   复制后核对 sha256 与源文件一致。
3. 顺带订正 `README.md` L134 的过期配色描述（实物是浅色文件夹 + 深色 α 用于安装器/任务栏）。
   中英两个 README 的对应句子都要看（`README.zh-CN.md` 里搜同段）。
4. 检查是否还有其它 GitHub 可见面在用旧图标：`grep` 全仓 `alpha-file-manager-logo` 与 `.github/media`
   （写卡时仅 4 处，见 3.1）；`docs/screenshots/afm-social-preview.png`（461 KB）是否含旧图标由新会话看图决定，
   **默认不动**，除非用户要求。
5. 提交并推送 `origin/main`（push 先带默认代理，失败再 `git -c http.proxy=` 重试），
   然后 `web_fetch` GitHub 上的 README 渲染页或 raw 页确认新图已生效。
6. 门禁：本任务只改二进制图片与两句文档，跑 `eslint .` 对照基线即可（基线现量法见第 7 节）。

## 5. 任务 2 执行计划（E:\r 删除卡死：查日志 → 定根因 → 修复）

1. **先读当前日志**（3.2 的读法）确认有无新的事发记录；没有就按「必须复现」走。读 `startup-probe.log` 排除启动期问题。
2. **确认 `scan_delete_targets` 的线程模型**：读 `delete_job.rs` 签名 + Tauri v2 同步 command 的执行线程；
   这决定 H1 是「窗口冻死」还是「无反馈等待」。
3. **自造等价靶子**（放 `C:\Users\Duanyi\` 下普通目录，**不要 %TEMP%**——会被全局搜索忽略规则剪掉，且这里是删除靶子也要避开系统目录）：
   规模 ≥ 20 万条目、深层嵌套、**含 >260 长路径**、含少量 symlink/junction、可选一段 OneDrive 形态目录。
   造靶与清理脚本放 `.superpowers/sdd/`（纯 ASCII；大靶子清理见第 7 节红线）。
4. **加观测再复现**：给扫描阶段补 info 日志（进入/每 N 条/结束耗时）与前端 preview 的加载态；
   复现时记录「点删除 → 首个反馈」的耗时、窗口是否响应（沙箱截图/UIA 判别）、日志时间线、进程线程数。
5. **修复方向候选**（按 H1→H6 对应，新会话按证据选）：
   preview 扫描改异步 + 进度事件 + 可取消 + 立即显示进度对话框；preview 与 job 共享同一次扫描结果（去掉双扫）；
   扫描本身并行化或至少让进度事件从扫描期就开始流；长路径与云占位文件的错误处理降级为跳过并计数。
   注意 `cargo test --lib` 红线（第 7 节）：新增测试链接到的代码不能碰 `tauri::AppHandle` / `tauri::async_runtime::*`，
   必要时照 `dir_reader.rs` 的 `*_blocking` 模式拆同步内核。
6. **验收**：自造靶子上「点删除 → 首个进度反馈 ≤ 2 s、进度持续增长、中途可取消、终值 `deleted_items == total_items`」；
   门禁全绿；**真机验收必须用 `tauri build` 打包产物 + 隔离桌面沙箱**（脚本范式见
   `.superpowers/sdd/r42-tree-sandbox.ps1`，后台跑、日志重定向到文件、不挂 stdout 管道）。
7. 修复若随 3.9.2 发版：走 `docs/operations/release-workflow.md` 六步；**不要**覆盖 3.9.1。

## 6. 进度状态一览

| 项 | 状态 |
|---|---|
| 3.9.1 发版 + 站点 + GitHub release | 完成（已验证） |
| winget 清单 3.9.1 + PR #467381 | 清单已提交，**Azure 验证已通过**（`Azure-Pipeline-Passed`+`Validation-Completed` 已回）；**待账号本人签 CLA** + 待社区 moderator 批准 |
| 任务 1 图标：选型与落点侦查 | 完成（已验证）；**替换已实施**（`.github/media/alpha-file-manager-logo.png` 已由 41,379 B 换成 31,145 B 的新图，两份 README 已改），**未提交、未验收** |
| 任务 2 删除：日志路径/代码链路/靶子现场侦查 | 完成（已验证）；**修复已在改**（`delete_job.rs` / `delete_tree.rs` / `lib.rs` / `use-file-browser-selection.ts` / `delete-jobs.ts` 已改，新增 `src/utils/delete-scan-summary.ts` + 测试），**未提交、未验收** |
| winget PR #467381 | 清单已提交，Azure 验证已通过；**待账号本人签 CLA** + 待社区 moderator 批准（2026-11-01 09:17Z 复核无变化） |
| 本卡 | 已写，**未提交**（新会话随改动一起提交即可） |

## 7. 红线速记（与本两任务直接相关的子集；全集在 AGENTS.md）

- 真机验收只能用 `tauri build` 产物；`tauri dev` 界面结论一律作废。验收在隔离桌面沙箱跑，不在交互桌面跑。
- 破坏性靶子必须自造；**不拿 `E:\r` 试删除**。验收前后「备份真实数据 → 灌夹具 → 恢复」，恢复顺序「先杀进程 → 再 restore」并核对 sha256。
- 门禁基线**现量、干净树、同一条命令**：`git stash push -u` 后量 HEAD 的 `eslint .`（整仓口径），再 stash pop 对照；
  别拿旧数字对（基线随提交变）。`vitest` 用默认 reporter（`--reporter=basic` 不存在）。
- `.cmd` / `.ps1` 纯 ASCII；运行期间绝不编辑 `.cmd`；`.cmd` 的 `echo` 里不能出现 `>`；后台长任务日志重定向到文件、不挂管道。
- 不用 `Get-Content -Raw` / `Set-Content` 改带中文的源文件（用编辑工具）；不用 `eslint --fix` / `stylelint --fix`。
- 删文件走 `E:\Users\Duanyi\.minimax\.minimax\bin\mavis-trash.cmd -- "<路径>"`；
  20 万文件级大树该 wrapper 会假报 `failed to trash`，兜底用
  `[Microsoft.VisualBasic.FileIO.FileSystem]::DeleteDirectory($p,'OnlyErrorDialogs','SendToRecycleBin')`；
  `icacls /deny` 造的目录先 `/remove:d` 再删。
- PowerShell：`EnumerateFiles(...).Count` 要包 `@(...)`；函数内有返回值时别混 `Write-Output`（用 `Write-Host`）。
- bash 命令文本里避开 `delet|trash|remove` 字样（安全层会拒），搜这类标识符用 `grep` / `glob` 工具。
- `cargo test --lib` 红线：被测代码不碰 `tauri::AppHandle` / `tauri::async_runtime::*`（否则 0xC0000139 加载崩）。
- 新设置项必须先进 `src/stores/storage/user-settings.ts` 默认对象（本两任务大概率用不到，改了设置才需要）。
- 不擅自发版/公开动作；winget CLA 只能账号本人签。

## 8. 必读文档清单（新会话按此顺序读，路径全量）

1. `F:\soft\00selfmade\filemanager\AGENTS.md` —— 项目硬约束全集（注入版会截断，直接 read 源文件）。
2. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-readme-icon-and-delete-hang.md` —— **本卡**。
3. `E:\Users\Duanyi\.minimax\.minimax\maintenance\task-state\LATEST.md` —— 自动交接卡（最近会话快照）。
4. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-folder-tree-toggle-persistence.md`
   —— 上一轮闭环卡（含沙箱验收脚本范式、发版六步实操、winget 推送实操）。
5. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\operations\release-workflow.md` —— 发版流程（任务 2 若发版才需要）。
6. `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\design-2026-10-24-file-unlock-and-force-delete.md`
   与 `F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-10-27-delete-enter-key-and-list-view-grouping.md`
   —— 删除功能的设计与历史上下文（任务 2 必读）。
7. 代码入口（任务 2 直接打开）：
   `src\modules\navigator\components\file-browser\composables\use-file-browser-selection.ts`（L1535-1610）、
   `src\stores\runtime\delete-jobs.ts`、`src-tauri\src\delete_job.rs`、`src-tauri\src\delete_tree.rs`。

## 9. 一句话接力 prompt（给用户复制到新会话）

> 请先依次阅读 `F:\soft\00selfmade\filemanager\AGENTS.md`、`F:\soft\00selfmade\filemanager\sigma-file-manager\docs\handoff-2026-11-01-readme-icon-and-delete-hang.md` 以及卡内第 8 节列出的全文档清单，充分掌握项目背景、目标、进度与下阶段计划后，按卡中第 4、5 节执行两个任务：把 GitHub README 等文档的旧图标换成新图标（深色文件夹 + 浅色字符版本），并排查修复 AFM 对 `E:\r` 删除长时间无响应的缺陷。
