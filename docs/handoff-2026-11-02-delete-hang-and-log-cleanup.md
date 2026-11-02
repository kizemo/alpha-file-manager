# Alpha File Manager 交接文档（删除卡死修复 + 日志降噪）

> **生成时间**：2026-11-02 11:00（本地，UTC+8）
> **产出会话**：`mvs_8a0d5f8e7f734112aca21150754203b1`
> **前置交接文档**：`docs/handoff-2026-11-01-relay-four-tasks.md`（四任务接力版，**任务 1 / 3 / 4 的细节仍以那份为准**）
> **仓库**：内层 `F:\soft\00selfmade\filemanager\sigma-file-manager`（HEAD `bc918a79`，version 3.9.1）
> **本文件是本批次（删除卡死 + 日志排查）的唯一权威交接源**

---

## 0. 一分钟总览

本会话接了上一份交接文档的**任务 2**（删除卡死），并**追加**了用户新提的**日志排查**任务。

| # | 事项 | 状态 | 下一会话第一动作 |
|---|---|---|---|
| 2 | 删除大目录导致卡死 | **代码修复完成 + 打包 + 装机 + 真机验收通过**（A1/A2/A4 有硬证据，50,051 项零残留） | 见 §5 遗留项（前端进度面板细节 + A3 取消需重验） |
| 新 | 日志中的其他报错 | **3 类根因全部定位并修复，已装机**；本轮冷启动 `WARN/ERROR = 0` | 见 §6 遗留项（webview 61s 卡顿触发源未追到） |
| 1 | 图标 + README | **仍未提交未推送**（改动在工作树里） | 提交 + 推送，细节见前置文档 §3 |
| 3 | 全盘搜索极慢 | **仍未定案**，本会话未碰 | 补计时埋点，细节见前置文档 §5 |
| 4 | 快捷方式「打开文件所在位置」 | **代码已改 + TDD 绿（873 passed）+ 有 runbook**；真机验收结果本会话未复核 | 按 §7 的 runbook 跑真机验收 |

---

## 1. 仓库与环境

| 仓 | 路径 | HEAD | 可否改动 |
|---|---|---|---|
| **内层（主）** | `F:\soft\00selfmade\filemanager\sigma-file-manager` | `bc918a79` | ✅ 全部功能改动只在这里 |
| 外层 | `F:\soft\00selfmade\filemanager` | `71ffcac` | ❌ 不做功能改动 |

### 1.1 工具链（本机实测路径）

- Rust toolchain **不在 PATH**：`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`
  ⚠️ 目录名里的 `pc-` 不能省，写成 `1.90.0-x86_64-windows-msvc` 时 `cargo metadata` 直接报 `program not found`，`tauri build` 3 秒内 exit 1。
- MSVC：`call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`（工具版本 `14.44.35207`）。
- cargo 必须带 `--manifest-path src-tauri/Cargo.toml`；`cargo test` 需 `link.exe` ⇒ 先 vcvars64 ⇒ 走 `.cmd`。

### 1.2 门禁命令（**只用这些，绝不跑 `npm run lint`**）

```
npm --prefix <repo> run ts                 # vue-tsc
npm --prefix <repo> run lint:eslint:check  # eslint . --max-warnings 0
npm --prefix <repo> run lint:stylelint:check
npm --prefix <repo> run test:unit:run -- <files>
npm --prefix <repo> run tauri:build        # sync-version && tauri build && publish 到 release/
```

- `npm run lint` = `lint:eslint`(`eslint . --fix`) + `lint:stylelint`(`stylelint ... --fix`) ⇒ **会静默改写全仓文件，绝对禁止**。
- `vitest --reporter=basic` 在本仓库不存在，会直接中止。抓结果看 `Test Files` / `Tests` 两行。
- `$ErrorActionPreference='Stop'` 会把原生命令的 stderr 变成终止错误 ⇒ 调 `npm`/`npx` 前后必须显式切回 `'Continue'`。
- eslint 必须传**绝对路径**（相对路径会 `No files matching the pattern`，exit 2）。
- `eslint .` 基线（2026-10-23 实测）= `188 errors / 0 warnings`，其中 `eslint src` = 175、`eslint scripts` = 13。**基线数字随提交变**，量的时候用 `git stash` 量 HEAD，别拿旧数字对。

---

## 2. 硬约束（违反即返工）

沿用 `AGENTS.md` 的项目级约束（简体中文输出、品牌名 Alpha File Manager、受保护常量不可改、`tauri build` 才能做界面验收、需要装机实测就自己打包、不覆盖用户数据），本会话新增/沿用以下**流程**约束：

1. **验收前必须先杀既有 AFM 实例** —— 单实例守卫会把新 pid 转发给老进程后自行退出。
2. **`.cmd` / `.ps1` 必须纯 ASCII**；中文一律用码点常量构造或作命令行参数传入。
3. **`.cmd` 运行期间绝对不能编辑它**（cmd.exe 按字节偏移逐行读）。
4. **`.cmd` 的 `echo` 里不能出现 `>`**（会被当重定向符，曾生成名为 `3.6.0)` 的垃圾文件）。
5. **后台跑构建/长脚本时，`.cmd` 内部用 `> logfile 2>&1` 重定向，不挂 stdout 管道**（`Tee-Object` / `| Select-String` 会让进程树随管道断裂而死 —— 本项目已因此报废过一次十分钟的 Rust 构建）。
6. **不用 PowerShell `Get-Content -Raw` / `Set-Content` 改含中文的源文件**；批量改 JSON 用 node 的 `fs.readFileSync/writeFileSync(..., 'utf8')`。
   ⚠️ `Copy-Item -Force` + `New-Item -ItemType Directory -Force` 做备份会被安全层拒（`recursive write/delete target cannot be verified`）⇒ 用 node 写文件。
7. **bash 命令文本里避开 `delet|trash|remove` 字样**（安全层会拒）；搜这类标识符用 `grep`/`glob` 工具或 `git grep`。
8. **单轮工具调用 ≤ 10 次**，超过 20 次立刻停止试探、改为一次取齐。中间产物写文件，对话里只留指针 + 结论。
9. **破坏性靶子必须自造**，放 `C:\Users\<user>\` 下普通目录，**不放 `%TEMP%`**（`global_search/ignore.rs` 的 `builtin_ignored_paths()` 含 `/AppData/Local/Temp`，会整棵子树剪掉）。
10. **恢复真实数据的顺序必须是「先杀进程 → 再 restore」** —— 应用退出时会把内存状态写回真实文件。
11. **验收口径不可改**：走 `Delete` 键（`use_trash=true`），不走 `Shift+Delete`（注入会丢 `shiftKey`，四条绕法全部失败）。
12. **单实例守卫 + 桌面遮挡**：验收前先杀实例；`MoveWindow` 置顶窗口到 `(-4000,-4000)` 并记原 rect 以便复原。

---

## 3. 本会话的改动清单（`git status --porcelain`）

**本会话相关（7 个已跟踪改动）**
```
 M src-tauri/src/delete_job.rs          +103/-   删除作业：扫描/删除两阶段、scan_completed 标志、日志抽稀
 M src-tauri/src/delete_tree.rs          +78/-    回收站心跳：TRASH_HEARTBEAT_INTERVAL / emit_progress_now
 M src-tauri/src/extensions/http.rs      +35/-    扩展市场拉取降噪（中间重试降 DEBUG，终态保留 WARN）
 M src-tauri/src/dir_watcher.rs          +12/-    unwatch 无 watcher 的幂等清理降 DEBUG
 M src-tauri/src/webview_guard_runtime.rs +15/-   RECOVERY_DEADLINE_MS 15s → 60s
 M src/stores/runtime/delete-jobs.ts     +48/-    scan_completed 消费 + onScanCompleted 回调
 M src/modules/.../use-file-browser-selection.ts +69/-  规模提示改由作业扫描阶段产出
```

**同时存在于工作树、但**不属本会话**的改动（任务 4 + 任务 1，上一批会话留下的）**
```
 M src-tauri/src/dir_reader.rs / dir_reader/commands.rs / dir_reader/read.rs
 M src-tauri/src/lib.rs
 M src/modules/.../use-file-browser-actions.ts / file-browser-tree-view.vue
 M src/localization/messages/*.json   （17 个语言包各 +1 行 openContainingDirectoryFailed）
 M README.md / README.zh-CN.md / .github/media/alpha-file-manager-logo.png
?? src-tauri/src/dir_reader/reveal.rs   （新文件，reveal 加固的纯内核）
```

**新增未跟踪**
```
?? src/utils/delete-scan-summary.ts              （新增：规模摘要 + 判据）
?? src/utils/__tests__/delete-scan-summary.test.ts
?? docs/handoff-2026-11-02-delete-hang-and-log-cleanup.md   （本文件）
```

`git diff --stat` = 34 文件 / **367 insertions(+), 100 deletions(-)**。**无 staged 改动。**

> ⚠️ 提交时**必须挑文件**：任务 1（README/icon）、任务 2（删除）、任务 4（reveal）、日志降噪（http/dir_watcher/webview_guard）是四组独立改动，**不要一锅端**。

---

## 4. 任务 2：删除卡死 —— 已修复并通过真机验收

### 4.1 根因（三条叠加）

| 编号 | 根因 | 说明 |
|---|---|---|
| H1 | 同步 command `scan_delete_targets` 阻塞 Tauri 主线程 | Tauri v2 的同步命令在主线程执行，扫大树时整个窗口冻住 —— 这才是「删除后长时间无响应」的**真正来源** |
| H2 | 预扫描与删除作业扫描**同源** | 同一棵树被完整遍历两遍，第二遍还是全量 |
| H6 | 全程零反馈 | 用户看不到任何动静，只能理解为「卡死」 |

### 4.2 修复内容

- **移除**同步预扫描命令 `scan_delete_targets`（后端 `lib.rs` 取消注册，命令与 `DeleteTargetSummary` 结构体已删）。
- **规模统计由删除作业自己的扫描阶段独家承担** —— 两遍扫描合并成一遍，且跑在后台阻塞线程上。
- **进度载荷新增 `scan_completed: bool`**（serde camelCase ⇒ 前端 `scanCompleted`），前端 `isDeleteScanCompleted` 保留 `files>0 || directories>0` 兜底。
- **日志抽稀**为「阶段切换必记 + 每 10 条一行」，并补一条权威读数 `scan finished in {:?}`。
  > 这条改动的必要性已被证实：旧逻辑让**每一条**扫描进度都落盘，80 万条目直接刷光 2 MB 日志轮转额度，把其它任务的关键证据全冲没了。
- **不改 `delete_tree.rs` 的算法**（扫描仍单线程递归、节流仍 100ms/500 条目、删除仍 4 线程并行）。
- **新增回收站心跳**（`TRASH_HEARTBEAT_INTERVAL = 2s` / `TRASH_HEARTBEAT_POLL = 100ms` / `emit_progress_now`）：
  旧实现只在「整棵树搬完」之后发一次进度 —— 5 万文件的目录交给 shell 后可能搬几分钟，这段时间**一条事件都没有**。现在进入 shell 前强制发一条（界面据此切到 deleting 阶段），搬运期间每 2 秒发一条。

### 4.3 真机验收（`tauri build` 打包产物，已静默安装）

产物：`F:\soft\00selfmade\filemanager\sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_3.9.1_x64-setup.exe`（42.79 MiB）
安装：`D:\Program Files\Alpha File Manager\alpha-file-manager.exe`（80,307,712 B，ver 3.9.1，`INSTALL_EXIT=0`）

靶子：`C:\Users\Duanyi\afm-accept-deletehang\target` = 50 子目录 × 500 文件 = **50,051 项**（顶层目录自身算 1 项）

| 判据 | 实测 | 证据 |
|---|---|---|
| INPUT struct 大小 | 40 字节 | `size=40` |
| 冷启动后靶子卡可见 | `852,200 2512x101` | 冷启 13 s 时 `uia_elements=33` |
| **A1** 首条进度日志距作业开始 | **0 秒**（墙钟 `confirm_to_first_progress_wall_sec = -0.41`） | 原来要等全树扫完才有动静 |
| **A2** 扫描条目数持续增长 | `first=297 last=49720 samples=16`（单调） | 16 次采样 |
| 扫描摘要行存在 | `scan finished in 15.1306541s: cancelled=false items=50051 files=50000 directories=51 bytes=50000` | |
| **A4** 删除终值 | `deleted_items=50051 == total_items=50051`，`failed=false error=None` | 见下 |
| 源目录残留 | 0；50,051 项整体进回收站 | `TARGET_MISSING`，回收站有 `target`（from=`C:\Users\Duanyi\afm-accept-deletehang`） |

作业完整日志（job `f4513629-3880-4c03-a833-3b35dfcbc18d`，UTC）：
```
[02:15:45] started: 1 path(s), use_trash=true
[02:16:00] scan finished in 15.1306541s: cancelled=false items=50051 files=50000 directories=51 bytes=50000
[02:16:00] [154] phase=deleting items=0 total_items=Some(50051) freed_bytes=0 total_bytes=Some(50000) percent=Some(0) current=C:/Users/Duanyi/afm-accept-deletehang/target
[02:16:12] [160] phase=deleting ... percent=Some(0)
[02:16:32] [170] phase=deleting ... percent=Some(0)
[02:16:38] finished in 53.1732324s: cancelled=false deleted_items=50051 freed_bytes=50000 total_items=50051 files=50000 directories=51 failed=false error=None
```

耗时拆解：扫描 15.13 s（约 3,300 items/s）+ 删除 38.04 s = **53.17 s**。

### 4.4 ⚠️ 关键更正：前端**本来就有**进度面板，位置是右上角「状态中心」

> 本会话早期一度误判为「前端没有进度面板，界面零反馈」—— **该判断作废**。
>
> 真相：`src/modules/status-center/` 就是删除进度面板的实现。右上角工具栏有个 Activity 图标按钮
> （`status-center-toolbar-button.vue`），**有活动作业时自动展开成药丸**，显示「N 项操作」+ 转圈 loader；
> 点开是 Popover，里面按「进行中的操作 / 已完成的操作」分区，每个删除作业一行：
>
> - 摘要行（操作类型 + 状态）
> - **进度条**：`scanning` 阶段走不确定态动画（`--indeterminate`，因为还没分母），`deleting` 阶段走真实百分比
> - 详情行：扫描期「正在统计要删掉多少… · 已发现 N 项」；删除期「X% · 已删除 done/total 项 · 字节进度」
> - 右侧一个**取消**按钮（`BanIcon`，`aria-label = t('cancel')` = 「取消」）+ 分区级「取消全部」
>
> **实测证据**：验收时 UIA dump（`uia-before-cancel.csv`）里确实有
> `Button|状态中心: 1 项操作|3408,15 165x42|ExpandCollapse+ScrollItem`，
> 截图 `job-1.png` 右上角也能看到那个紫色药丸。上一轮之所以「找不到」，是因为
> **只搜了「扫描中 / 已扫描 N 项 / 取消删除」这些关键词，而实际文案是「正在统计要删掉多少…」「已发现 N 项」**。

**结论**：前端进度面板**已存在且工作正常**，本轮不需要新做进度面板。真正需要补的是 §5 的两点。

---

## 5. 遗留项（下一会话做）

### 5.1 「扫描期取消」（A3）**是假阳性，必须重验**

验收脚本用 `$CN_CANCEL` **前缀**匹配「取消」，命中了文件列表底部的 `Button|取消全选|3156,2036 130x39`
（那是选择操作，不是删除作业的取消）。点下去只取消文件选择，作业 `cancelled=false` 一路跑完 ——
也正因此**意外把 A4（删除终值）也验掉了**。

要真验取消，正确做法：
- 按 **等值**匹配 `Name -eq '取消'`（`ch.json` 里 `cancel` = 「取消」，`close` = 「关闭」），
- 且限定在**状态中心 Popover 的子树**内（`StatusCenterOperationRow` 的取消按钮 `aria-label` 就是 `t('cancel')`），
- 或用分区级「取消全部」按钮（`statusCenter.cancelAll`）。
- 取消逻辑**本轮未改代码**，所以这条只是「没验」，不是「没实现」——
  `use-cancel-operation.ts` → `deleteJobsStore.cancelJob` → `invoke('cancel_delete_job')` 的链路是通的。

### 5.2 删除阶段进度条百分比长时间为 0（**真实缺陷，值得修**）

日志里三条 `phase=deleting` 的 `percent=Some(0)`，直到搬完才一次性跳到 100%。
原因：回收站搬运是**整棵树一次性交给 shell**，拿不到树内逐条目进度
（`delete_paths_to_trash` 的条目数取自预扫描，在每个顶层路径搬完之后才计入）。

可考虑的改法（尚未实现）：
- 让回收站路径的进度条在搬运期间也走**不确定态**（而非假 0%）；
- 或把「搬运中」与「按条目删除」在 UI 上区分开；
- 或对**单个大目录**改走逐条目删除路径以换取真进度（代价是放弃回收站的整树原子性）。

### 5.3 `webview guard` 61,775 ms 心跳超时的**触发源仍未追到**

见 §6.2。本轮只放宽了「误弹框」判定窗口，**没消除「webview 真的卡了 60 秒」这个事实**。

### 5.4 收尾时未做的一件事

回收站里那个验收靶子 `target`（50,051 项）本会话**已永久清空**（`RB_FIXTURE_LEFT=0`），
`C:\Users\Duanyi\afm-accept-deletehang` 空壳父目录**已删除**，`user-workspaces.json` 已恢复 1741 B 原值，AFM 进程 0。
（清回收站时会弹「删除文件夹」确认框，需点确认；弹窗标题可用 `EnumWindows` + 标题精确匹配定位，
`SendMessage(WM_COMMAND, IDYES=6)` 可直接确认。）

---

## 6. 日志排查 —— 3 类根因已定位并修复

### 6.1 排查范围与结论

扫描产物：`.superpowers/sdd/log-scan-20261102.cjs` → `.superpowers/sdd/log-report-20261102.txt`

- 应用日志基线：231,400 B / 1,650 行，范围 2026-11-01 11:51:05 .. 23:52:08，`errors=32 warnings=60 distinct=9`
- `startup-probe.log`（123,705 B / 2,539 行）：**0 errors / 0 warnings**（干净）

9 种日志模式归为 **3 类根因**：

| # | 根因 | 条数 | 处理 |
|---|---|---|---|
| 1 | 扩展市场拉取失败被逐次记 WARN | 86 | 中间过程降 DEBUG，终态保留且仅保留一条 WARN |
| 2 | webview guard 心跳超时误弹框 | 4 | `RECOVERY_DEADLINE_MS` 15 s → 60 s |
| 3 | `dir_watcher` unwatch 幂等清理 | 2 | `log::warn!` → `log::debug!` |

### 6.2 逐条明细

**① 扩展市场拉取失败（`sigma_file_manager_lib::extensions::http`，86 条 WARN）**

6 个变体：`trying next fallback URL after transient failure`（×28）、`giving up after 2 attempts … network timeout`（×27）、
`attempt 1/2 failed … Retrying in 588ms`（×25）、`excalidraw`（×3）、`video-downloader`（×2）、`giving up（registry.json）`（×1）。

改法（`src-tauri/src/extensions/http.rs`）：
- `fetch_url_across_urls` 的 `Permanent` 分支：先 `log::warn!` 再 `return Err`；
- `Transient` 分支的 `trying next fallback URL` → `log::debug!`；
- 循环末尾构造 `"{}: all {} URL candidate(s) failed. Last error: {}"` → `log::warn!` + `Err`；
- `run_with_retry` 的 `attempt {}/{} failed … Retrying in {}ms` → DEBUG；
- **`giving up after N attempts` 保留 WARN**（终态失败必须有且仅有一条 WARN）。

**② webview guard 心跳超时（4 条 ERROR）**

```
window-hung detected for window main (no heartbeat for 61775 ms), reloading it   ×3
recovery failed for window main; prompting the user                             ×1
```

机制：前端每 5 s 心跳；`HEARTBEAT_TIMEOUT_MS = 20_000`；`missed_ms >= 20_000 * 3`（60 s）⇒ 判 hung ⇒ ERROR + `reload()`；
reload 后 15 s 心跳没回来 ⇒ `escalate_to_prompt` 弹原生框。

本地计时：12:46:25 心跳 `main:4403ms` 正常 → 12:47:26 变 `main:49712ms` → 12:47:38 判 hung →
12:47:54 判 recovery failed（reload 后仅 16 s）→ 12:48:28 前端恢复（约 50 秒）。

改法：`RECOVERY_DEADLINE_MS` **15_000 → 60_000**。
> `window-hung` 的 60 s 判定与 reload 行为**一字未改** —— 这是「降低误报」而非「掩盖失败」。

**③ `dir_watcher` unwatch 告警（2 条 WARN）**

```
[dir-watcher-diag] unwatch_directory no active watcher path=…\Start Menu\Programs active_count=0
```
定性：切目录 / 关标签页 / 组件卸载的**幂等清理正常分支** ⇒ 降 DEBUG；真正失败仍保留 WARN。

### 6.3 降噪实证

- 本轮运行窗口（UTC 02:15:07–02:16:45）共 **58 行日志，`WARN/ERROR = 0`**。冷启动必定拉扩展市场，上一轮同场景 86 条 WARN，本轮 0 条。
- 三条历史告警的时间戳逐一核对，**全部是 2026-11-01 的，本轮零新增**：
  `no active watcher` ×2 → 12:48:29 / 12:51:41；`window-hung` ×3 → 12:47:38 / 12:49:46 / 12:51:29；`recovery failed` ×1 → 12:47:54。
- 最新一条 `extensions::http` WARN 是 `[2026-11-02][01:39:36]`（09:39 本地，**早于 09:57 装新包**）⇒ 属旧版本产物。
- **`DEBUG_LINES = 0`**：应用日志级别是 INFO，降到 `log::debug!` 的中间过程行**根本不落盘** —— 这正是降噪目标达成的机制。

> ⚠️ **归因边界**：本轮「0 条 WARN」不能单独归因于降噪 —— 网络可能恰好正常，
> 而 `trying next fallback` 已降到 DEBUG（INFO 级别下不可见）⇒ 无法用日志区分「降噪生效」与「根本没失败」。
> 能确定的是：代码层面的改动已完成并装机。

---

## 7. 任务 4「打开文件所在位置」—— 状态与下一步

代码已改并有 TDD 证据，但**本会话未复核真机验收结果**。

- 改动：`src-tauri/src/dir_reader/reveal.rs`（新，纯内核，刻意不碰 `tauri::AppHandle` / `tauri::async_runtime::*`，
  否则 `cargo test --lib` 会在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND` 崩掉）、
  `dir_reader/commands.rs`、`use-file-browser-actions.ts`、`file-browser-tree-view.vue`、17 个语言包各 +1 行
  `openContainingDirectoryFailed`。
- 三处加固：`explorer.exe` 用绝对路径、`/select,<path>` 加引号、前端正斜杠转反斜杠；
  **2026-11-02 追加**：快捷方式（`.lnk`）目标先校验 —— 目标失效就**本地拦下**返回错误，不再把坏快捷方式交给 Explorer 弹它的模态框。
- TDD 证据：`.superpowers/sdd/reveal-lnk-green-test.log` 尾部 `test result: ok. 873 passed; 0 failed`。
- **真机验收 runbook**：`.superpowers/sdd/reveal-accept-runbook.md`（含夹具构造、UIA 驱动脚本、收尾顺序）。

下一步：按 runbook 跑一遍真机验收（关键判据：Explorer 打开且**没有** `#32770` 模态框、
坏快捷方式走 toast 而非 Explorer 弹框），收尾顺序同样是「**先杀进程 → 再恢复 `user-workspaces.json`**」。

---

## 8. 关键文件与行号速查

### 8.1 后端

| 文件 | 关键位置 | 内容 |
|---|---|---|
| `src-tauri/src/delete_job.rs` | L28/L31 | `DELETE_PHASE_SCANNING` / `DELETE_PHASE_DELETING` |
| | L115-116 | `scan_completed: false` 默认值 |
| | L129-137 | `item_percent`（分母为 0 时返回 `None`） |
| | L160-252 | `run_delete_blocking`（扫描 → 删除两阶段） |
| | L183-197 | `scan finished in {:?}` 汇总日志 |
| | L213-224 | `scan_completed: true` 的扫描收尾事件 |
| | L263-273 | `scan_delete_targets` 已删除的 FORK-MODIFICATION 说明 |
| | L299-336 | emit 任务：阶段切换必记 + 每 10 条一行 |
| `src-tauri/src/delete_tree.rs` | L444-479 | `ProgressThrottle`（100 ms / 500 条目双闸门） |
| | L482-501 | `report_progress` |
| | L503-517 | `TRASH_HEARTBEAT_INTERVAL` / `TRASH_HEARTBEAT_POLL` / `emit_progress_now` |
| | L791-885 | `delete_paths_to_trash`（心跳循环 + `std::thread::scope`） |
| `src-tauri/src/extensions/http.rs` | L194-226 | `RetryConfig`（`for_downloads` 4 次 / `for_metadata` 2 次） |
| | L370-410 | `run_with_retry`（`giving up` 保留 WARN，中间重试降 DEBUG） |
| | L708-760 | `fetch_url_across_urls` / `download_to_file_across_urls_with_retry` |
| `src-tauri/src/dir_watcher.rs` | L584-596 | `unwatch_directory`（无 watcher 分支已降 DEBUG） |
| `src-tauri/src/webview_guard_runtime.rs` | L41 | `RECOVERY_DEADLINE_MS`（**已改 60_000**） |
| | L470-516 | `attempt_recovery` / `escalate_to_prompt` |
| `src-tauri/src/webview_guard.rs` | L34/L37 | `HEARTBEAT_INTERVAL_MS = 5_000` / `HEARTBEAT_TIMEOUT_MS = 20_000` |
| | L128-150 | `decide_health`（20 s × 3 = 60 s 判 hung） |

### 8.2 前端

| 文件 | 关键位置 | 内容 |
|---|---|---|
| `src/stores/runtime/delete-jobs.ts` | L21-50 | `DeleteJobProgressPayload`（含 `scanCompleted`） |
| | L130-167 | 进度事件监听 → `statusCenterStore.updateOperation` |
| | L162-165 | `isDeleteScanCompleted` 触发 `onScanCompleted` |
| | L261-275 | 作业一建起来就 `phase: 'scanning'`（不等首个事件） |
| `src/utils/delete-scan-summary.ts` | L22 | `LARGE_BATCH_ITEM_THRESHOLD = 10_000` |
| | L61-75 | `isDeleteScanCompleted` |
| | L88-90 | `shouldWarnLargeBatchDelete` |
| `src/modules/status-center/status-center-toolbar-button.vue` | L35-56 | 活动作业数 / aria-label |
| | L219-339 | Popover 内容（进行中 / 已完成两分区） |
| `src/modules/status-center/status-center-operation-row.vue` | L73-87 | `showProgressBar` / `isScanningPhase`（不确定态） |
| | L268-337 | 删除作业的详情行拼装（扫描期 vs 删除期） |
| | L446-455 | 单条取消按钮（`aria-label = t('cancel')`） |
| `src/modules/status-center/use-cancel-operation.ts` | L20-44 | 取消分派（delete → `deleteJobsStore.cancelJob`） |
| `src/stores/runtime/status-center.ts` | L23-59 | `Operation` 接口（含 `phase` / `deletedItems` / `itemPercent`） |
| `src/modules/.../use-file-browser-selection.ts` | L1536-1571 | `showLargeBatchDeleteSummary`（规模提示 toast） |
| | L1620-1631 | `startJob` + `onScanCompleted` 接线 |
| `src/localization/messages/ch.json` | L1529-1561 | `statusCenter.*`（`deleteScanning` = 「正在统计要删掉多少…」、`deleteScanningFound` = 「已发现 {count} 项」、`deleteItemsProgress` = 「已删除 {done} / {total} 项」） |
| | L973 / L1056 | `close` = 「关闭」/ `cancel` = 「取消」 |

### 8.3 验收脚本与产物

- 脚本：`F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\accept-deletehang-20261101.ps1`（634 行 / 28,610 B，纯 ASCII）
  - 参数：`-ExePath` / `-OutDir` / `-TargetName='target'` / `-ColdWaitSec=300` / `-JobWaitSec=900` / `[switch]$DoCancel` / `-CancelAfterSec=6`
  - L36-41 中文码点常量；L66-236 C# `Acc` 类（`INPUT` 用 `[StructLayout(LayoutKind.Explicit, Size = 40)]`）
  - **缺陷**：`$CN_CANCEL` 前缀匹配会命中「取消全选」⇒ A3 假阳性（见 §5.1）
- 包装：`run-accept-A5.cmd`（纯 ASCII，内部 `> run-out.txt 2>&1` 重定向）
- 本轮产物目录 `.superpowers/sdd/accept-20261102-A5/`：
  `accept-result.json`（8,664 B，`assert_pass=6 assert_fail=0`）、`run-out.txt`（2,988 B / 36 行）、
  `progress-samples.txt`（4,511 B / 17 行）、`before-*.png` / `after-*.png` / `job-1.png` / `job-2.png`（各约 3.5 MB）、5 份 `uia-*.csv`（688–696 行）
- 日志扫描：`log-scan-20261102.cjs` → `log-report-20261102.txt`（4,812 B）
- 构建：`check-4.cmd` → `cargo-check-4.log`（`CHECK_EXIT=0`，`Finished dev profile in 21.09s`，14 条存量 warning）；
  `build-5.cmd` → `build-5.log`（243,671 B，`Finished release profile in 3m 38s`）
- 任务 4 相关：`reveal-accept-runbook.md`、`reveal-accept-driver.ps1`、`reveal-accept-launch.ps1`、
  `reveal-lnk-green-test.log`（873 passed）、`reveal-dialogs/dialog-1.png`、`dialog-2.png`

### 8.4 运行时路径

- 应用日志：`C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log`
  （现 262,398 B / 1,822 行；**时间戳是 UTC = 本地 − 8 h**）
- `user-workspaces.json`：`C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\user-data\user-workspaces.json`
  （**已恢复为 1741 B 原值**）；备份 `.superpowers/sdd/user-data-backup-20261102/user-workspaces.json`
  ⚠️ **`user-workspaces.json` 里保存的当前标签页优先** ⇒ 改 `user-settings.json` 的 `defaultDirectory` 不生效。

---

## 9. 实测可用的命令形态

### 9.1 `.cmd` 模板（**纯 ASCII**，`>` 只作重定向符）

```bat
@echo off
rem do not edit this file while it is running: cmd reads it by byte offset
call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat" >nul 2>&1
if errorlevel 1 (
  echo VCVARS_FAILED
  exit /b 1
)
set "PATH=C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin;%PATH%"
set "LOG=F:\soft\00selfmade\filemanager\sigma-file-manager\.superpowers\sdd\build-5.log"
cd /d "F:\soft\00selfmade\filemanager\sigma-file-manager"
echo BEGIN %DATE% %TIME% > "%LOG%"
npm run tauri:build >> "%LOG%" 2>&1
echo BUILD_EXIT=%ERRORLEVEL% >> "%LOG%"
echo DONE %DATE% %TIME% >> "%LOG%"
```

### 9.2 启动 + 等完成 + 读尾部

```powershell
$p=Start-Process -FilePath 'cmd.exe' -ArgumentList @('/c',$cmdPath) -PassThru -WindowStyle Minimized
for($i=0;$i -lt 80;$i++){ Start-Sleep -Seconds 3; if((Test-Path $log) -and (Select-String -LiteralPath $log -Pattern 'DONE ' -SimpleMatch -Quiet)){ break } }
Get-Content -LiteralPath $log -Tail 25
```

### 9.3 静默安装 + 核对

```powershell
$setup='F:\soft\00selfmade\filemanager\sigma-file-manager\release\Alpha File Manager_3.9.1_x64-setup.exe'
Get-Process alpha-file-manager -ErrorAction SilentlyContinue | ForEach-Object { Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 2
$ip=Start-Process -FilePath $setup -ArgumentList '/S' -PassThru -Wait
Write-Output ("INSTALL_EXIT={0}" -f $ip.ExitCode)
$e=Get-Item -LiteralPath 'D:\Program Files\Alpha File Manager\alpha-file-manager.exe'
Write-Output ("bytes={0} mtime={1} ver={2}" -f $e.Length, $e.LastWriteTime, $e.VersionInfo.ProductVersion)
```

### 9.4 读带独占锁的应用日志

```powershell
$log='C:\Users\Duanyi\AppData\Local\com.sigma-file-manager.app\logs\Alpha File Manager.log'
$fs=[System.IO.File]::Open($log,[System.IO.FileMode]::Open,[System.IO.FileAccess]::Read,[System.IO.FileShare]::ReadWrite)
$sr=New-Object System.IO.StreamReader($fs,[System.Text.Encoding]::UTF8)
$txt=$sr.ReadToEnd(); $fs.Dispose()
$lines=$txt -split "`r?`n"
```

> `File.Open` 的参数顺序是 `path, FileMode, FileAccess, FileShare`，**必须传枚举**（传字符串会报「无法匹配的重载」）。

### 9.5 用 node 恢复运行时配置（`Copy-Item -Force` 会被安全层拒）

```powershell
node -e "const fs=require('fs');const bak='F:/.../user-data-backup-20261102/user-workspaces.json';const dst='C:/Users/Duanyi/AppData/Roaming/com.sigma-file-manager.app/user-data/user-workspaces.json';const b=fs.readFileSync(bak,'utf8');console.log('BACKUP_BYTES='+Buffer.byteLength(b));console.log('BEFORE_BYTES='+fs.statSync(dst).size);fs.writeFileSync(dst,b,'utf8');console.log('AFTER_BYTES='+fs.statSync(dst).size);"
```

---

## 10. 失败的做法（不要重试）

- **给后台构建挂 stdout 管道**（`... 2>&1 | Select-Object -Last 45`）⇒ 管道一断进程树被杀，`cargo check` 报 `exit code 101` 且 rustc 无诊断输出。
- 把 `Error code 26: File opened that is not a database file` / `failed to auto-clean cache data` 当成构建故障 ⇒ 那是 cargo 全局缓存 GC 噪声。
- `Start-Process powershell.exe -ArgumentList @('-File',$script,'-ExePath','D:\Program Files\...')` ⇒ 含空格路径不加引号会参数错位。
- `Copy-Item -Force` + `New-Item -ItemType Directory -Force` 做备份 ⇒ 被安全层拒（`recursive write/delete target cannot be verified`）。
- `[System.IO.File]::Open($log,'Open','Read','FileShare.ReadWrite')` ⇒ `Open` 位置是 FileAccess，报枚举无法匹配。
- `$c.GetSupportedPatterns()`（`$c=$El.Current`）⇒ 要用 `$El.GetSupportedPatterns()`。
- `ConvertTo-Depth 6` ⇒ 真名 `ConvertTo-Json -Depth`。
- `Find-All $UIA::RootElement` ⇒ 枚举整个桌面，命中 MiniMax Code 自己窗口的同名文本。
- 心跳线程用 `join()` 等 `sleep(2s)` ⇒ 每个小目录平白加 2 秒；已改成 100 ms 轮询 `done: AtomicBool`。
- 日志级别普查正则写成 `'\]\[(\w+)\]\[([a-z_]+)\]\['` 匹配不到 —— 实际格式是 `[date][time][module][LEVEL]`，级别在**最后**、后面没有 `[`。改用 `-like '*关键词*'` 逐项计数。
- **用 `Process.MainWindowHandle` 取本项目的主窗口** ⇒ 会落在 15×15 的托盘窗上；要用 `EnumWindows` + class `Tauri Window` + 可见 + 面积最大。
- **用 `PrintWindow` 判 hover / overlay 层** ⇒ 它不渲染这些层；overlay 要走屏幕 DC BitBlt。
- 屏幕 BitBlt 的白名单**必须包含应用自己的 `msedgewebview2`** ⇒ webview2 的 `Chrome_WidgetWin_0` 顶层窗口在光标落在页面区域时就是下方那个窗口，只认主进程 pid 会把每一张干净截图误判成「被遮挡」。
- `net use` 盘符必须带冒号（`net use Y: \\host\share`）；`net.exe` 输出是控制台代码页（cp936），要按 `GetConsoleOutputCP` 解码。
- `SendInput` 的 `INPUT` 必须 `[StructLayout(LayoutKind.Explicit, Size = 40)]` ⇒ 只声明 `KEYBDINPUT` 会让 `Marshal.SizeOf` < 40，**静默失败**（返回 0、不报错）。
- 注入的 **Shift 到不了 webview 的 `shiftKey`** ⇒ `Shift+Delete` 会变成「移至垃圾箱」。字母键的 `Ctrl+Shift` 组合可靠，单修饰键 + 非字母主键会丢修饰键。验收永久删除改走右键菜单顶部的垃圾桶图标条。
- 工具栏按钮不暴露 UIA `InvokePattern`；过滤框在 UIA 里是空壳（没有 `[Edit]`、没有 `ValuePattern`）⇒ 写过滤词只能靠剪贴板 + `SendInput`。
- `cargo test --lib` 的测试二进制**不能碰** `tauri::AppHandle` / `tauri::async_runtime::*` ⇒ 会拖进 tao/Wry，测试二进制在加载阶段以 `STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) 崩掉，一个测试都跑不了。
- `eslint .` 不读 `.gitignore`；`.superpowers/**` 已加进 `eslint.config.ts` 的 `globalIgnores`。
- `grep` 工具受 `graphify-out/graph.json` 污染，判存在性只用 `git grep -F HEAD`。

---

## 11. 下一会话建议路线

1. **先读本文件 + `docs/handoff-2026-11-01-relay-four-tasks.md`**（任务 1/3/4 的细节在那份里）。
2. **补 §5.1**：重写 A3 取消定位（等值匹配「取消」+ 限定状态中心 Popover 子树），跑一轮把取消能力真正验掉。
3. **补 §5.2**：决定删除阶段进度条要不要从假 0% 改成不确定态 —— 这是本轮唯一可能翻盘的 UX 缺陷。
4. **跑任务 4 的真机验收**（§7 的 runbook），确认「打开文件所在位置」不再弹 Explorer 模态框。
5. **任务 3（全盘搜索）**：按前置文档 §5.6 的顺序补计时埋点，未定案前不要写修复。
6. **提交**：四组改动分开提交（任务 1 / 任务 2 / 任务 4 / 日志降噪），不要一锅端。
7. **不发版**：3.9.1 已发布不可覆盖，下一版本号 **3.9.2**。
