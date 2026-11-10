# 交接：focus sync「AFM 访问闩」（v0.5.15）—— **已装机，四场景 + 逃生阀 A/B 均验收通过**

- 生成时间：2026-11-06（当日三轮接力；最后一次更新 2026-11-06 12:50）
- 会话 ID：`mvs_7019d0c24b8b4c5cb064c0f640b4cd72`（首轮：分析 + 代码）→ 接力轮（**P0 装机验收 + P1 打包修复**）→ 第三轮（**用户反馈排查 + 活体复验**，见 §十）
- 内层仓：`F:\soft\00selfmade\filemanager\sigma-file-manager`（branch `main`）
- 外层仓：`F:\soft\00selfmade\filemanager`（文档仓，`src-tauri` / `package.json` **只在内层仓**）

> **📌 阅读顺序提示**：本文档 §二 / §六 写的是**换装前**的历史状态。
> **当前事实以 §八点五（P0 + P1 结果）与 §十（第三轮排查结论）为准。**

---

## 一、需求原文与目标

用户（kizemo）原话：

> 当前，在任何情况下，切到下载弹窗、打开弹窗，都会自动同步之前的资源管理器路径，而这不一定是使用者需要的，或者超出了使用者预期。
> 因此希望，只有在使用者把窗口切换到 AFM 窗口，再切回弹窗，路径才进行同步。
> 请深入分析这个需求，然后进行优化。在优化前，先确定当前节点，如出现问题，可以回滚到当前状态。

三步中「确定当前节点」「深入分析」「代码优化 + 单测」已完成并汇报。**真机验收也已完成**（2026-11-06，见 §八点五 与 §十）；
本文档 §二、§六 描述的是「换装前」的旧状态，阅读时请以 §八点五 / §十 为准。

---

## 二、⚠️ 换装前的事实（**已于 2026-11-06 解决**，详见 §八点五 / §十）

2026-11-06 用户反馈「focus sync 仍然会默认执行，而不是从 AFM 切到弹窗时才执行」。
实测 `/health` 后判定：**该现象来自旧二进制，新逻辑既没被验证、也没被证伪。**

> **✅ 后续结论（2026-11-06 12:45 补）**
>
> 1. canonical 位已替换为 **0.5.15** 并重启，`/health` 报 `version=0.5.15` + `afm_visit_latched` 字段；
> 2. 四场景 + 逃生阀 A/B **全部通过**（§八点五）；
> 3. 用户二次反馈「仍然和以前相同」经活体复验判定为 **「测试动作与旧行为不可区分」**，
>    **不是改动未生效** —— 判别方法、证据与「是否需要重装」的结论见 **§十**。

```
GET http://127.0.0.1:37421/health
  version            = 0.5.14          ← 新代码是 0.5.15
  afm_visit_latched  = （字段不存在）
  keys = status,uptime_secs,last_ping_received_ts,last_dialog_event_ts,active_dialogs,
         uia_healthy,session_id,version,mode,unsupported_dialog_count,first_push_received,
         last_health_poll_ts,last_health_poll_age_ms,extension_alive
进程：
  PID=10144 focus-sync-sidecar  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\
                                 extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe
  PID=61096 alpha-file-manager  D:\Program Files\Alpha File Manager\alpha-file-manager.exe
```

⇒ **运行时用的侧车，和这次改的代码，是两个不同的文件。** 新产物只落到了打包引用位（见 §四），
安装到 `%APPDATA%` 的那份从没被替换过。**接力第一件事就是换文件、重启、看 `version` 变成 0.5.15，再测行为。**
在没有做到这一步之前，任何「功能没生效」的结论都不成立。

---

## 三、当前节点与回滚

- 回滚 tag：`pre-focus-sync-afm-gate-2026-11-06`，指向内层仓 HEAD `c72e6e09c88c4264476d05294169718f9d1abba9`
- 回滚命令（在**内层仓**执行）：

  ```powershell
  git -C F:\soft\00selfmade\filemanager\sigma-file-manager reset --hard pre-focus-sync-afm-gate-2026-11-06
  ```

- 改动前 `git status --short` 为空，tag 已回读校验与 sha 一致。
- 当前工作树有未提交改动：内层 4 个侧车源文件 + 暂存 `bin\focus-sync-sidecar.exe`；
  外层 `M AGENTS.md`、`M docs/extension-changelog.md`、未跟踪 `docs/SFM-EXTENSION-DEVELOPMENT-GUIDE.md`、
  `docs/blog/`、`docs/marketing/`、`vt3.txt`。**版本 `package.json` = 3.11.0 未 bump。**

---

## 四、已完成的工作（代码已全部落地并通过门禁）

### 4.1 设计：三条写弹窗路径与闸门收口位置

侧车往弹窗写路径一共三条路，用户抱怨的两个现象分别落在 B 和 C：

| 编号 | 路径 | 触发条件 | 与抱怨的对应 |
|---|---|---|---|
| A | `handle_set_path` 即时写 | 扩展推送时弹窗已前台 | 近乎不可达（要求 AFM 非前台） |
| B | `DialogEvent::Opened` write-back | 弹窗打开就写 | 「**打开弹窗就自动同步**」 |
| C | `should_resync_known_dialog` 每 1200ms 重发 `Opened` → 落到 B | 切回已知弹窗 | 「**切回弹窗就自动同步**」 |

**关键分析结论（不要再走回头路）**：侧车原本没有任何「使用者是否访问过 AFM」的状态位；
**per-dialog 计数法会恒真退化** —— 侧车启动后 AFM 必然已被访问过，所以信号必须是**上升沿**。

v0.5.15 的「AFM 访问闩」规则：

- **上弦**：AFM 前台上升沿。信号源**只有这一个**（曾考虑「AFM 路径真变化」这一路，已放弃 ——
  扩展 `checkHealth()` 恢复后会重推记忆路径，见 `extensions\kizemo.focus-sync\dist\index.js` L209，
  会把「没访问也同步」重新放进来）
- **闸门**：只收口在 B（`main.rs` 的 Opened 写回分支）。C 复用同一入口 ⇒ 自然被拦，日志口径只有一种
- **下弦**：写成功即下弦；**写失败不下弦**（让冷却重试复用同一次授权）；A 成功写入也下弦
- **被拦时仍用 `initial_path` 登记**（保证「回 AFM 再切回」能同步）
- 既有守卫一律不动：`first_push_received` 哨兵、v0.5.7 G-2 决策后登记、v0.5.12 `write_succeeded`、
  G-3 源端 1200ms 限流、R-2 非前台只存不写、v0.5.8 死句柄剪枝。新闩只做加法。
- **「Sync Now」不需要绕过闩**（用户显式动作走 A 路径）

### 4.2 改动清单（4 文件，全在 `sigma-file-manager\extensions\kizemo.focus-sync\sidecar\src\`）

| 文件 | 内容 |
|---|---|
| `state.rs` | `afm_visit_latch: AtomicBool`（L185）+ `arm_afm_visit()` / `afm_gate_open()` / `clear_afm_visit()`；逃生阀 `afm_gate_enabled()`（L299-308，`OnceLock` + `std.env::var`）；`HealthSnapshot` / `health_snapshot()` 加 `afm_visit_latched`；`#[cfg(test)] mod afm_gate_tests`（L432 起，7 条用例） |
| `uia_event.rs` | tick 内 AFM 前台上升沿上弦；`pub fn is_afm_window(hwnd)` = class `Tauri Window` **且**标题前缀 `Alpha File Manager`；`foreground_is_afm()`；未新增 import |
| `main.rs` | B 分支改为 `if writable && gate_open { ... state.clear_afm_visit() } else if writable && !gate_open { ... }`，后者记 `kind: "opened_write_back_skip_no_afm_visit"`。⚠️ 本文件日志写法是 `log_event(&Event::SpikeError { kind, message, ts: now_iso8601() })`，**不是** `state.log_event(EventKind::Skip, ...)` |
| `http_server.rs` | L67 `SPIKE_VERSION = "0.5.15"`；L137 `HealthResponse` 加 `afm_visit_latched: bool`、L525 `handle_health` 出线；`WriteOutcome::UiaSetValue` / `SendInputFallback` 两分支各下弦；4 处测试夹具补字段 |

### 4.3 门禁（日志 `.superpowers\sdd\afm-gate-test-out.txt`）

- `afm_gate_tests` 7 条全 `ok`（L141-148）
- `test result: ok. 95 passed; 0 failed`（L213）、`TEST_EXIT=0`（L227）、`BUILD_EXIT=0`（L332）
- 中途修掉一个**真缺陷**：缓存静态叫 `DISABLED`（语义=「闸门是否被关闭」），但 `afm_gate_enabled()`
  曾把原值当「是否启用」返回 ⇒ 环境变量未设时（实测 `ENV=[]`，User/Machine 也无该键）
  `afm_gate_open()` 恒真、**闩完全失效**；另有 3 条「绿」是恒真的平凡通过。
  已改为 `!*DISABLED.get_or_init(...)`（state.rs L301），并在
  `latch_starts_closed_so_no_afm_visit` 开头补判别性守卫 `assert!(AppState::afm_gate_enabled(), ...)`。
- **已知覆盖缺口**：逃生阀 `FOCUS_SYNC_AFM_GATE` 因 `OnceLock` 跨用例污染无法单测，只能真机 A/B。

### 4.4 新产物暂存

- crate `sigma-listary-spike`，`[[bin]] name = "spike"` ⇒ 产物是 `...\sidecar\target\release\spike.exe`
  （**不是** `focus-sync-sidecar.exe`），2,841,600B / 2026-11-06 09:38:55
- 已 `Copy-Item` 覆盖到打包唯一引用位 `...\extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe`，
  回读确认 `STAGED: 2841600B 2026-11-06 09:38:55`
- 打包引用点（**内层仓**）：`sigma-file-manager\src-tauri\tauri.conf.json:89`
  `"../extensions/kizemo.focus-sync/bin/focus-sync-sidecar.exe": "tools/focus-sync-sidecar.exe"`
  全仓 grep **无任何地方**引用 `installer\focus-sync-sidecar.exe`（2026-10-09 那份遗留副本，故意未覆盖）
- NSIS 侧 `src-tauri\installer\hooks.nsh` L948-1001：把 `$INSTDIR\tools\focus-sync-sidecar.exe`
  复制到 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
  （= canonical per-user 位，即 §二 里 PID 10144 跑的那份）并注册计划任务。

### 4.5 文档

- `F:\soft\00selfmade\filemanager\docs\extension-changelog.md` 新增 `## v0.5.15 (2026-11-06) —— AFM 访问闩`
  （三条写路径对照表、上弦/闸门/下弦规则、AFM 判据、改动清单、逃生阀与可观测、行为变化与取舍）
- 同文件「记录缺口」小节：本文件此前最后一条是 v0.5.8，**v0.5.9~v0.5.14 从未登记**（代码里早已到 0.5.14）。
  本轮**不回填**（超范围），只如实标注。
- 状态卡：`F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-gate-status.md`

---

## 五、下阶段计划（按优先级；**P0 / P1 均已完成**，结果见 §八点五，剩余待办见 §十一）

### P0 —— 装机验收（**已完成**，见 §八点五）

**换侧车文件即可，不必等安装包**（本次改动只在侧车 Rust 层，应用外壳一行未动）：

1. **先备份再动**：备份 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
   —— 备份后必须回读校验（字节数 + 是旧版）；改完要能还原。
2. 停旧进程（PID 10144 会随应用/计划任务重生）→ 用 `...\sidecar\target\release\spike.exe` 覆盖 canonical 位 → 起侧车
3. `GET 127.0.0.1:37421/health` **先断言 `version=0.5.15` 且 `afm_visit_latched` 字段存在**
   （这条断言没过去谈行为都是白费）
4. 验四场景：
   - ① 不碰 AFM 直接开弹窗 ⇒ **不同步**，日志出 `opened_write_back_skip_no_afm_visit`
   - ② 切到 AFM 窗口 → 切回弹窗 ⇒ **同步**，`afm_visit_latched` 由 true 变 false
   - ③ 一次访问只兑现一次：AFM → 弹窗A（同步）→ 弹窗B（**不再同步属预期**）
   - ④「Sync Now」正常
5. 逃生阀 A/B：设 `FOCUS_SYNC_AFM_GATE=0` 重启侧车，应回到 0.5.14 的行为（单测覆盖不到，只能真机验）
6. 验收后**必须还原真实运行时数据**（若有改动）并核对；别把测试状态留给用户

### P1 —— 修 `npm run tauri:build`（发版前必须能出安装包）

连续两次 exit 1、60-70 秒即退，**根因未定位**：

- 第 1 次 `.superpowers\sdd\afm-gate-build.cmd`（LF 行尾）日志 `afm-gate-build-out.txt` 只有 299B 纯 vcvars 横幅
- 第 2 次 `afm-gate-build2.cmd`（实测 `bytes=799 CR=16 LF=16`，带 `MARK_*` 标记）日志
  `afm-gate-build2-out.txt` 只有 395B / 9 行：`MARK_START` → `where npm` 两行结果
  （`D:\Program Files\nodejs\npm` 与 `npm.cmd` 都在）→ `MARK_BEFORE_VCVARS` → vcvars 横幅 4 行。
  **没有 `MARK_AFTER_VCVARS`、没有 `MARK_BEFORE_NPM`、没有任何 npm 输出、没有 `PKG_EXIT`**
  ⇒ `cmd.exe` 在 `call vcvars64.bat` 那行之后就停止执行脚本
- 两次在**同一位置**失败、行尾不同 ⇒ **行尾不是根因，别再走行尾修复这条路**
- 注意 `tauri:build` = `npm run sync-version && tauri build && node ./scripts/publish-bundle-to-release.mjs`；
  另有一条已知独立问题：`publish-bundle-to-release.mjs` 会遍历 `bundle/nsis/` 所有历史包逐个
  `rmSync`+复制，任一旧包被进程锁住 → 整步 exit 1 而 `tauri build` 本身早已成功
  （判别后单独用 `node -e "fs.copyFileSync(a,b)"` 把新包补进 `release/` 即可，不必重打包）

### P2 —— 仅当 P0 装机后行为仍不符预期，才回到代码

排查顺序建议：`/health` 的 `version` → 上弦是否发生（AFM 判据是否真命中，见 §六「推断」）→
`active_dialogs` / `first_push_received` → 侧车日志。

---

## 六、事实分级（已验证 / 推断 / 未验证）

**已验证（有硬证据）**
- ~~运行中的侧车是 0.5.14~~ → **此条已过期**：2026-11-06 换装后运行时为 **0.5.15**，
  `/health` 有 `afm_visit_latched` 字段，canonical 位 sha256 `B061D8A2…A1A7D`（见 §八点五、§十）
- 95 条单测全绿、`cargo build --release` exit 0（日志见 §4.3）
- 新产物 `spike.exe` 已暂存到 `bin\focus-sync-sidecar.exe`（2,841,600B 回读确认）
- 打包引用位是 `bin\`，`installer\` 那份无引用（`tauri.conf.json:89` + 全仓 grep）
- `FOCUS_SYNC_AFM_GATE` 三处（进程/User/Machine）均未设置 ⇒ 失败不是配置残留
- 单测里 `afm_gate_enabled()` 取反写反的缺陷已坐实并修复（改前 4 FAILED）

**推断（合理但未证实，接力时当假设对待）**
- AFM 主窗口 OS 标题以 `Alpha File Manager` 开头 —— 依据是「前端不写 `document.title`、`decorations:false`」，
  grep glob 覆盖面有限。**若真机发现标题不符，须改回进程名判据**（`alpha-file-manager.exe`，
  `src-tauri\Cargo.toml:2` 的 `name` 已确认；但 `QueryFullProcessImageNameW` 在 windows 0.62 的
  参数形态未验证 ⇒ 当初特意避开）
- ~~用户反馈的现象完全由旧二进制造成~~ → **已证实**（换装后 `version=0.5.15`，
  闩在真机上按规格生效；见 §八点五、§十）

**未验证（截至 §八点五 之前一条证据都还没有；现状态如下）**
- ~~v0.5.15 的闩在真实弹窗 + 真实窗口切换序列下的行为~~ → **已验证**（原生 `#32770` 四场景通过）
- ~~逃生阀 `FOCUS_SYNC_AFM_GATE=0` 的实际回退效果~~ → **已验证**（A/B 双向闭环）
- 被拦时的日志量 —— **仍未实测**（实测：单次开弹窗产生 4 条 `SKIP_NO_AFM_VISIT`；未测持续拦截速率）
- **非原生弹窗**（Chrome/Edge Save As 等非 `#32770`）下的闩行为 —— **仍未验证**
  （侧车 `/health` 的 `unsupported_dialog_count` 字段可用于观测）

---

## 七、证据与文件路径速查

| 用途 | 路径 |
|---|---|
| 侧车源码（4 文件） | `F:\soft\00selfmade\filemanager\sigma-file-manager\extensions\kizemo.focus-sync\sidecar\src\{state,uia_event,main,http_server}.rs` |
| 侧车产物 | `...\extensions\kizemo.focus-sync\sidecar\target\release\spike.exe` |
| 打包暂存位 | `...\extensions\kizemo.focus-sync\bin\focus-sync-sidecar.exe` |
| 运行时 canonical 位 | `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe` |
| 单测/构建门禁日志 | `F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-gate-test-out.txt` |
| 重跑门禁脚本 | `cmd /c "F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-gate-test.cmd"` |
| 打包脚本与失败日志 | `...\.superpowers\sdd\afm-gate-build{,2}.cmd` / `afm-gate-build{,2}-out.txt` |
| 状态卡 | `...\.superpowers\sdd\afm-gate-status.md` |
| changelog | `F:\soft\00selfmade\filemanager\docs\extension-changelog.md` |
| 侧车日志读取 | `F:\soft\00selfmade\filemanager\sigma-file-manager\scripts\read-sidecar-log.ps1` |
| 扩展前端（本轮**未改**） | `...\extensions\kizemo.focus-sync\dist\index.js`（38,558B；无 `src/`、无 `build.mjs`、无 TS ⇒ 不可改） |

---

## 八、本轮踩到的工具坑（直接照抄可省一轮）

- **`npm run tauri:build` 两次同点失败**：见 §P1，行尾已被证伪。
- **混合读写的一条命令会被权限层整体拒**：
  `Local hard safety policy blocked bash: recursive write/delete target cannot be verified inside the workspace`。
  ⇒ 拆成**单文件 `Copy-Item` 一条**即可通过。
- **`cd` 到内层仓后混用 `Get-Content` / `Select-String` 会被权限层拒** ⇒ 取源码事实一律用
  `read` / `grep` 配**绝对路径**。
- **`src-tauri` 与 `package.json` 只在内层仓**；用外层路径 `Get-Item` 会报「找不到路径」。
- **侧车源码路径必须写全 `extensions\kizemo.focus-sync\sidecar\src\`**；
  读 `sigma-file-manager\sidecar\src\state.rs` 会 ENOENT。
- **`extensions\kizemo.focus-sync\` 顶层只有 `icon.png` / `LICENSE` / `package.json`，没有 `RELEASE_NOTES.md`**；
  且 `$ErrorActionPreference='Stop'` 下这一条失败会让后续命令一起不跑。
- **`glob` 对 `extensions/kizemo.focus-sync/**/*.js` 返回 No files matched**（dist 被忽略规则挡掉）
  ⇒ 列清单走 `Get-ChildItem -Force`。
- **`edit` 工具参数名必须是 `file_path`**（传 `path` 报 `must have required properties file_path`）。
  工具名大小写敏感：`read` 有效、`Read` 报 `Tool not found`。
- **`error[E0063] missing field ... at src\http_server.rs:587` 只出现在 lib test**，同轮
  `cargo build --release` exit 0 ⇒ 测试红不等于生产代码红，别据此回滚。
- **取长日志结论别回灌全文**：
  `Select-String -Path '<log>' -Pattern 'TEST_EXIT|BUILD_EXIT|test result:|error\[|^error:|afm_gate' | ForEach-Object { "L$($_.LineNumber): $($_.Line.Trim())" }`
- **每次 bash 调用都要重注入 MSVC/Rust**：`cargo` 不在 PATH，在
  `C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`（⚠️ `pc-` 不能省，
  省了 `tauri build` 3 秒 exit 1）；MSVC 靠
  `call "C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat"`。
- **`.cmd` / `.ps1` 必须纯 ASCII**（cmd.exe 按 ANSI 代码页读，中文注释会变乱码当命令执行）；
  `.cmd` 的 `echo` 里不能出现 `>`（会被当重定向，日志截断 + 仓库根冒出垃圾文件）；
  **`.cmd` 运行期间绝不能编辑它**（要改先 `task_stop`，再**新建文件名**重跑）。
  后台跑**不要挂 stdout 管道**（`Tee-Object` / `| Select-String` 一断，进程树跟着死）。
- 项目级硬约束与验收坑一律见 `F:\soft\00selfmade\filemanager\AGENTS.md`（注入版被截断，需要全文就 read 源文件）。

---

## 八点五、本轮续做结果（2026-11-06 接力会话，**P0 与 P1 均已完成**）

接力会话 `mvs_39b96b715e554da5b201c7e734699a93` 完成了 §五 的 **P0（装机验收）** 与 **P1（打包修复）**。

### P0 —— 装机验收：四场景 + 逃生阀 A/B 全部通过

**换装**：canonical 位已替换为 v0.5.15，sha256 `B061D8A25445F29ED1DA7597E969700B9CD9FFDC0FC1191A2A9A3171373A1A7D`
（2,841,600 B），与 `sidecar\target\release\spike.exe` **逐字相同**；旧版 0.5.14 备份在
`.superpowers\sdd\afm-gate-backup\focus-sync-sidecar-0.5.14.exe`（sha256 `07A3EC70…7CE1`，与换装前运行时文件一致）。
`/health` 报 `version=0.5.15` + `afm_visit_latched` 字段存在。

**四场景（已验证）**，判据取侧车 JSONL 日志的 `Write` / `opened_write_back_skip_no_afm_visit` 计数：

| 场景 | 实测 |
|---|---|
| ① 不碰 AFM 开弹窗 | `skip=4` / `write=0` |
| ② AFM → 弹窗 | `opened_write_back` + `Write target=…\14AI工具` |
| ③ 一次访问只兑现一次 | A(0) → 聚焦 AFM → B(**write=1**，闩被消费) → C(0) |
| ④ Sync Now / 显式推送 | `POST /set_path` 200 → `Write target=C:\Users\Duanyi\Desktop` |

**逃生阀 A/B（已验证，双向）**：

- 闸门开启（计划任务启动）：不碰 AFM 开弹窗 → `skip=4` / `write=0`
- 逃生阀关闭（`Start-Process -Environment @{FOCUS_SYNC_AFM_GATE='0'}` 手动起侧车）→ `skip=0` / **`write=1`**
- 恢复闸门开启态后复测 → `skip=4` / `write=0`（闭环）

**AFM 判据已实证**：`is_afm_window` = class `Tauri Window` + 标题前缀 `Alpha File Manager`，真实命中
主窗口 hwnd=198532（pid 61096，标题 `Alpha File Manager`）；聚焦后 `afm_visit_latched` 立刻变 true。

**运行时已还原**：侧车由计划任务 `KizemoFocusSync` 拉起，canonical 位 sha256 与构建产物一致，
`/health` = `version=0.5.15` + `extension_alive=true` + `afm_visit_latched=false`；
测试夹具进程已杀、可见 `Save As` 弹窗已关。**本会话未修改任何产品源文件。**

状态卡：`.superpowers\sdd\afm-gate-status.md`；changelog `docs\extension-changelog.md` 的 v0.5.15 节已补真机验收结论。

### P1 —— `npm run tauri:build` 静默中断：**根因已定位并修复**

**根因**：`call "vcvars64.bat" > "%LOG%" 2>&1` 会让该日志文件在**整个 cmd.exe 生命周期内被锁住**，
紧接着重定向到同一文件的下一条命令抛
`The process cannot access the file because it is being used by another process.`，
cmd.exe 随即中止 —— 所以 `MARK_AFTER_VCVARS` / npm 输出 / `PKG_EXIT` 全部缺失。
**行尾确实不是根因**（CRLF 与 LF 两次同点失败）。

隔离探针（`.superpowers\sdd\vcprobe1..6.cmd`）：

| 探针 | 形态 | 结果 |
|---|---|---|
| probe1 | `call vcvars64.bat > nul 2>&1` | PASS |
| probe2 | `call vcvars64.bat > "%LOG%" 2>&1` | FAIL（sharing violation） |
| probe3 | `call vcvars64.bat >> "%LOG%" 2>&1` | FAIL（sharing violation） |
| probe4 | `call vcvars64.bat > nul 2>&1 && where cl` | PASS，命中 MSVC `cl.exe` |
| probe5 | vcvars→nul 后 `where cl/link` + `call npm --version >> LOG` | PASS |
| probe6 | 顶层 `cmd /c "call vcvars64.bat && where cl"` | PASS |

**修复**：vcvars 输出改送 `nul`；`LOG` / `STATUS` 只留给 npm 阶段；
npm 是 `.cmd`，必须用 `call npm …`（不 `call` 会终止调用方脚本）。
修复版脚本：`.superpowers\sdd\afm-gate-build3.cmd`（纯 ASCII，**运行期间不要编辑**）。
修复后实测 `MARK_AFTER_VCVARS=0`、`where cl` → MSVC `cl.exe`、`where link` → MSVC `link.exe`、
`MARK_BEFORE_NPM` 已打印、前端 `✓ built in 29.49s`。

**构建最终结果（已验证）**：`PKG_EXIT=0` / `ALL_DONE`，
新安装包 `src-tauri\target\release\bundle\nsis\Alpha File Manager_3.11.0_x64-setup.exe`
（45,109,105 B，sha256 `34BF9B68245AD5A9DE9B13BEB9A1CE6E75EB975A84CAA8CA18690169201F663C`），
发布落点为**内层仓** `sigma-file-manager\release\local-builds\`（同名旧包内容不同 →
脚本按设计隔离到 `local-builds/`，不覆盖已发布资产）。
**包内侧车核验**：7z 抽出 `tools\focus-sync-sidecar.exe` → 2,841,600 B /
sha256 `B061D8A25445F29ED1DA7597E969700B9CD9FFDC0FC1191A2A9A3171373A1A7D`
⇒ 与运行时 canonical 位、与 `spike.exe` **三方逐字相同**，0.5.15 确实打进安装包。

### 终态（2026-11-06 11:38）

侧车由计划任务拉起，`/health` = `version=0.5.15` + `extension_alive=true` +
`afm_visit_latched=false` + `active_dialogs=0`；系统内 `#32770` 弹窗枚举 = 0；
无夹具进程残留；运行时 canonical 位 sha256 未变；`package.json` 版本仍 3.11.0 未 bump。

### 本轮新增的工具坑

- **`call <bat> > 同一文件` 会锁文件** → 之后任何重定向到它的命令都抛 sharing violation。
  **vcvars / 任何第三方 `.bat` 的输出一律送 `nul` 或送独立文件，绝不与后续要写的日志同路径。**
- **`Start-Process -Environment @{…}` 在 pwsh 7 可用**，是验证进程级环境变量的干净路径
  （**不要**用 `Remove-Item Env:\…`，权限层会把它判成 delete 硬拦）。
- **`npm` 在 PowerShell 里直接跑可以，但放进 `.cmd` 里必须 `call npm`**，否则脚本在 npm 处终止。
- **场景③的编排陷阱**：关掉弹窗会把焦点还给 AFM → 触发上弦 → 闩重新武装 → 出现自动化假象
  （看着像 bug，其实不是）。必须「B 保持打开时直接开 C」。
- **UIA `ValuePattern` 在本项目读出的是过期文件列表**，不能作路径判据；用日志计数。
- **`Get-Process` 的 `MainWindowTitle` 对 WinForms 窗有效**，可用作中性前台锚点。

## 九、约束备忘（不要重新讨论）

- 面向用户输出一律简体中文；代码标识符 / 路径 / 命令行 / 配置键名保持原样。
- 品牌名统一 **Alpha File Manager**；受保护常量不得改动
  （mDNS 服务名、legacy 数据目录名、Tauri `identifier` = `com.sigma-file-manager.app`）。
- **真机验收必须用打包产物/真实运行时文件，不能用 `tauri dev`**（dev 构建所有路由页内容区空白）。
- 版本口径：侧车 `SPIKE_VERSION` 0.5.14 → **0.5.15**；扩展 `package.json` 仍 `0.5.9`（两者本就不同步）；
  应用版本 `3.11.0` **未 bump**。
- 扩展侧不可改：不新增设置项、不改 `syncNow`、不动 `SetPathBody`。

---

## 十、2026-11-06 第三轮：用户反馈「仍然和以前相同」的排查与结论

### 10.1 结论先说（三句话）

1. **改动已生效** —— 运行时跑的就是 0.5.15，不是旧二进制。
2. **不需要重新安装** —— 重装对侧车行为零影响（理由见 10.4）。
3. 用户那次测试「看起来和以前相同」，是因为**测试动作本身与旧行为不可区分**
   （先在 AFM 里挑目录 → 闩被上弦 → 弹窗写回，这在新旧版本里都是同步）。

### 10.2 证据链（判定「已生效」）

| 判据 | 实测值 |
|---|---|
| 运行中侧车 exe | `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`（= canonical 位 = 本次换过的那个文件），pid 16796 |
| canonical 位 sha256 | `B061D8A25445F29ED1DA7597E969700B9CD9FFDC0FC1191A2A9A3171373A1A7D`（2,841,600 B），与 `spike.exe`、安装包内 `tools\focus-sync-sidecar.exe` **三方逐字相同** |
| `/health` | `version=0.5.15`、`afm_visit_latched` 字段存在、`extension_alive=true` |
| v0.5.15 独有日志事件 | `opened_write_back_skip_no_afm_visit` 计数 **22**（旧二进制产不出该事件） |
| 安装目录 `tools\focus-sync-sidecar.exe` 不存在 | **设计行为**，不是缺失 —— 见 10.4 |

### 10.3 用户那次测试走的是哪条路（关键）

用户反馈时间 **12:36**；日志里对应的弹窗事件在 **12:34:38**（UTC `04:34:38`）：

```
DialogDetected hwnd=1642894 app=#32770
Read           path=F:\soft\03system
opened_write_back  dialog path='F:\soft\03system' current='F:\resourse_study\vibe coding\零基础AI编程实战_得到'; writing current
Write          target=F:\resourse_study\vibe coding\零基础AI编程实战_得到
```

而 **12:19–12:34 之间日志连续出现 `path_update`**（`F:\resourse_study\vibe coding` → `…\得到`），
说明**用户当时正在 AFM 里翻目录** ⇒ AFM 前台上升沿 ⇒ **闩已上弦** ⇒ 弹窗被写回。
**这正是新规格期望的行为**，也正是旧行为 ⇒ 两者在这次测试里**不可区分**。

**判别性测试的唯一形态**：*完全不碰 AFM* 就打开弹窗。
本轮 **12:47 活体复验**（重启侧车排空闩 → `latched=false` → Phase 0）：

```
HEALTH version=0.5.15 latched=False ext_alive=True
P1 latch_before=False
P1 log=DIALOG_DETECTED,READ_PATH,SKIP_NO_AFM_VISIT ×4
P1 afm_path_in_dialog=0
P1 skip_count=4 write_count=0
P1 RESULT=NOT_SYNCED_OK
```

⇒ **当下就是新行为**：不碰 AFM 开弹窗 = **不写回**（0.5.14 会写回）。
复验产物：`.superpowers\sdd\afm-gate-phase0-live2.txt`、`.superpowers\sdd\afm-gate-p1.png`。

> **⚠️ 闩是 sticky 的（务必知悉）**
> 一次 AFM 前台访问会一直上弦，直到被一次成功写回**消费掉**。
> 因此「先在 AFM 里挑目录 → 再开弹窗」**永远会同步** —— 与旧版观察到的现象完全一致。
> 若闩为 true 时直接跑 Phase 0，会得到 `write_count=1` / `SYNCED_UNEXPECTED`：
> **那是闩未排空，不是产品缺陷**。判别性验证前必须先排空闩（重启侧车，或先消耗掉一次上弦）。

### 10.4 是否需要重新安装？——**不需要**

- **侧车行为维度**：canonical 位已是 0.5.15 且正在被运行；重装只会把**包内同一份 0.5.15**
  （sha256 相同）再复制一遍到同一位置 ⇒ **行为零变化**。
- **`D:\Program Files\Alpha File Manager\tools\focus-sync-sidecar.exe` 不存在不是缺失，是设计**：
  安装钩子 `src-tauri\installer\hooks.nsh` 的 **STAGE 7** 会把 `$INSTDIR\tools\focus-sync-sidecar.exe`
  复制到 canonical 位 `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\`，
  **然后删掉 tools 里的副本**（canonical 位是唯一副本，避免出现两处 canonical）。
  ⇒ 安装目录里本来就查不到这个 exe，**不用去补文件**。
- **重装的唯一实际价值**：让安装布局与 3.11.0 新包一致（可选）。
  若要装，用 `F:\soft\00selfmade\filemanager\sigma-file-manager\release\local-builds\Alpha File Manager_3.11.0_x64-setup.exe`；
  装完计划任务会重新拉起侧车（仍是 0.5.15），行为不变。
- **应用本体仍是 11-05 的旧安装**（`D:\Program Files\Alpha File Manager\` mtime 11-05 21:28~21:32），
  但 AFM 应用代码本轮**未改动**，本次验收不依赖应用版本。

### 10.5 本轮新增的工具坑

- **闩未排空 ⇒ 判别性验证出假阴性**（见 10.3 的 sticky 警告）。**先重启侧车再测。**
- **`Add-Type -Name/-Namespace/-MemberDefinition` 定义的类型要用完整名引用**（如 `[N.W]`），
  且必须在**同一进程**里先定义再引用；否则报「找不到类型 [W]」（本轮踩到，脚本仍 exit 0）。
- **验收脚本的 exit code 不是判据**：脚本用 `powershell.exe`（5.1）跑，
  其中的 `Add-Type` 代码在 pwsh 7 里报错但不影响流程 ⇒ 一律看脚本打印的 `RESULT=` 行。
- **读侧车日志必须 `FileShare.ReadWrite`**（侧车持锁），且日志时间戳是 **UTC**（比本地少 8 小时）。

---

## 十一、剩余待办（下一步计划）

| # | 事项 | 状态 / 说明 |
|---|---|---|
| 1 | 内层仓 4 个侧车源文件改动**未提交** | `git status`：`M extensions/kizemo.focus-sync/sidecar/src/{http_server,main,state,uia_event}.rs`。**待办：评审后提交**（回滚 tag `pre-focus-sync-afm-gate-2026-11-06`） |
| 2 | `bin\focus-sync-sidecar.exe`（打包暂存位）未出现在 `git status` | 已 tracked 且内容与 0.5.15 一致；确认是否需要单独提交 |
| 3 | 非原生弹窗（Chrome/Edge Save As）下的闩行为 | **未验证**（只验了原生 `#32770`）；可用 `/health` 的 `unsupported_dialog_count` 观测 |
| 4 | 被拦时的日志量（持续拦截速率） | **未实测**（实测单次开弹窗 4 条 skip） |
| 5 | 应用版本 `package.json` = 3.11.0 **未 bump** | 侧车 0.5.15 / 扩展 0.5.9 / 应用 3.11.0 三套版本号本就不同步，本轮**刻意不 bump** |
| 6 | 是否安装 3.11.0 新包 | **可选**，见 §10.4；不装不影响已验收的侧车行为 |
| 7 | 前端门禁 `eslint .` 全量对照 | 本轮**未跑**（未改前端源码）；改动只落在 Rust 侧车 4 文件，Rust 单测此前 95 条全绿 |

### 给新会话的一句话 prompt

```
请先阅读 F:\soft\00selfmade\filemanager\AGENTS.md 与 F:\soft\00selfmade\filemanager\docs\handoff-2026-11-06-focus-sync-afm-gate.md（重点看 §八点五、§十、§十一），并顺带读 F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-gate-status.md 与 F:\soft\00selfmade\filemanager\docs\extension-changelog.md 的 v0.5.15 节，掌握本项目的背景、目标、当前进度与下阶段计划后，再动手；不要从零重做文档里已完成的 P0/P1，也不要再重复已验收的四场景与逃生阀 A/B 结论。
```

### 新会话必读文档（完整路径）

1. `F:\soft\00selfmade\filemanager\AGENTS.md`（项目硬约束 + 验收坑；注入版被截断 ⇒ 读源文件全文）
2. `F:\soft\00selfmade\filemanager\docs\handoff-2026-11-06-focus-sync-afm-gate.md`（本文档）
3. `F:\soft\00selfmade\filemanager\.superpowers\sdd\afm-gate-status.md`（状态卡）
4. `F:\soft\00selfmade\filemanager\docs\extension-changelog.md`（v0.5.15 节 = 真机验收结论）
5. `F:\soft\00selfmade\filemanager\docs\SFM-EXTENSION-DEVELOPMENT-GUIDE.md`（扩展开发指引，未跟踪）
6. 内层仓源码：`F:\soft\00selfmade\filemanager\sigma-file-manager\extensions\kizemo.focus-sync\sidecar\src\{state,uia_event,main,http_server}.rs`
7. 侧车日志：`C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log`（JSONL，UTC）
