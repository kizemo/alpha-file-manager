# Handoff — Focus Activation UIA Port Route (Spike Reset Complete)

**会话日期**: 2026-09-27
**本次主题**: 焦点切换技术路线变更 — 从 spike 自写转为移植 PathWrap + QwenLM fg_bypass
**承接**: 无直接上一份 handoff(本次为路线重置)
**本次状态**: spike 源码 + 历史决策 doc + 9 个 spike commits 全部 reset,新路线调研材料就位,等新会话执行移植

---

## 一、本次变更背景(必读)

**之前路线**: 14-task spike 自写路线(自己摸索 UIA detector/reader/writer + TCP server + 7-app verify),从 `4749f01` 到 `41dfc83` 共 19 commits、+3018 行。**未能完成 7-app 实际覆盖率验证**(`verify_target_matrix.ps1` 交互式脚本反复卡住 Read-Host,本机只装 4 个 app,v2/7 上限 57% < 70% 必然 FAIL,即便跑通也价值有限)。

**本次调研**: 4 个并行 agent 做 GitHub 调研,产出 `.other/doc/00-04.md`(5 份报告 + 75+ 引用)。

**关键发现**:
- `inaku-Gyan/PathWrap` — 与 spike **同 `windows` 0.62 + 同栈 + MIT**,4 文件 ~1k LoC,**直接可搬**
- `QwenLM/qwen-code` cua-driver `fg_bypass.rs` — **实测数据扎实**(UWP Calculator 91% 抢焦点 → bypass 后 0/507 z-drops),Chromium 7/8 → 0。**windows 0.58**(调研报告误写 0.62,经我读源码确认是 0.58)
- 调研报告低估了 QwenLM 代码量(mod.rs 实际 1437 行 vs 报告 400 行)

**结论**: 移植优于自写,7-app 覆盖率 5-7/7 = 70-100%,工作量 3.5-4 天 vs 从零写 8-10 天。

---

## 二、本次回滚动作(已完成)

| 动作 | 命令 | 结果 |
|---|---|---|
| Reset HEAD | `git reset --hard f21c581` | 19 commits 全部丢弃,HEAD = `f21c581 docs(plan): Listary focus sync spike — 14 tasks over 5 days` |
| 删 spike 子目录 | `rm -rf sigma-listary-spike/` | 232M 浪费(target/)清理 |
| 删 release/ | `rm -rf release/` | 56M,与新路线无关 |
| 删 untracked handoff/prompt | 24 个 `rm -f` | `handoff-2026-09-25-*` 等历史接力产物全部清理 |
| 保留 `.other/` | 不动 | 62M 调研材料:报告 + clone 源码 |

**回滚后 working tree**:
```
HEAD:  f21c581
Branch: main
Tracked: 全部已清洁
Untracked: 只有 .other/(调研素材)
Tracked 历史遗留: .superpowers/、docs/、.gitignore、handoff-2026-09-25.md 等(本轮不动,属历史接力产物)
```

---

## 三、新路线规划(新会话执行)

### 3.1 必搬文件清单(3 文件,~705 行)

| 来源 | 目标 spike 路径 | 行数 | 关键能力 |
|---|---|---|---|
| `.other/src/PathWrap/src/os/dialog.rs` | `sigma-listary-spike/src/uia_inject.rs` | **176** | UIA 打分(AutomationId `"1148"` → 100 / `"1001"` → 90 / 名字含 "文件名" → 80) + **`InvokePattern::Invoke` 点 OK 按钮**(spike 缺失)+ `fallback_confirm` 用 `SendMessageW` 发 VK_RETURN(非 SendInput) |
| `.other/src/PathWrap/src/os/monitor.rs` | `sigma-listary-spike/src/uia_event.rs` | **399** | `SetWinEventHook` 3 事件(`EVENT_SYSTEM_FOREGROUND \| EVENT_OBJECT_FOCUS \| EVENT_OBJECT_SHOW`)+ **自适应 polling**(track 8ms / idle 30ms)+ mpsc 唤醒机制 + 多特征对话框识别(`#32770` + 标题 + `ComboBoxEx32` / `DirectUIHWND` / `SHELLDLL_DefView` / `DUIViewWndClassName` 组合)+ `DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` + `GetDpiForWindow` + lost_ticks 抖动恢复 |
| `.other/src/qwen-code/packages/cua-driver/rust/crates/platform-windows/src/uia/fg_bypass.rs` | `sigma-listary-spike/src/fg_bypass.rs` | **129** | **Chromium/UWP 自抢焦点绕过**:`EnableWindow(hwnd, FALSE)` RAII guard + Drop 恢复,门控 `is_xaml_host_hwnd \|\| is_chromium_target_window`,实测 0/507 z-drops |

### 3.2 不搬文件(明确)

| 来源 | 文件 | 不搬理由 |
|---|---|---|
| PathWrap | `window_ext.rs` (202 行) | spike 不是 overlay,WS_EX_NOACTIVATE 样式不需要 |
| PathWrap | `input_hook.rs` (174 行) | spike 不用 WH_KEYBOARD_LL(无热键场景) |
| PathWrap | `explorer.rs` (57 行) | spike 是"Sigma 作为源"模式,不需要枚举 Explorer |
| QwenLM | `mod.rs` (**1437** 行) | 通用 tree walk,spike 用不到 + API 不匹配(0.58 vs 0.62) |
| QwenLM | `cache.rs` (**353** 行) | UAF 保护,spike 单线程不需要 |
| QwenLM | `windows_enum.rs` (**1216** 行) | AI agent 通用窗口枚举,spike 用不到 |
| QwenLM | `revision.rs` / `scroll.rs` (620 + 256 行) | 单点用不到 |

### 3.3 spike 子目录重建后内容

```
sigma-listary-spike/
├── Cargo.toml              # windows 0.62 + tokio + tracing + serde + anyhow + tracing-subscriber
├── .gitignore
├── README.md               # 更新:说明移植来源 + 借鉴项目
├── src/
│   ├── main.rs             # 入口:tokio::select!(uia_event + tcp_server + poll_path_changes)
│   ├── lib.rs              # 模块导出
│   ├── config.rs           # [保留 spike 原版] CLI args + port resolve
│   ├── events.rs           # [保留 spike 原版] Event enum + Serialize
│   ├── logger.rs           # [保留 spike 原版] JSON-line logger
│   ├── state.rs            # [保留 spike 原版] AppState registry + current_path(Mutex<String>)
│   ├── tcp_server.rs       # [保留 spike 原版] line-based JSON protocol
│   ├── uia_inject.rs       # ★ 新增:从 PathWrap dialog.rs 移植(改数据类型)
│   ├── uia_event.rs        # ★ 新增:从 PathWrap monitor.rs 移植(解耦 egui::Context)
│   ├── fg_bypass.rs        # ★ 新增:从 QwenLM 移植(适配 windows 0.62 + 简化 is_chromium_target_window)
│   ├── detector.rs         # [保留 spike 原版] is_file_dialog(可与 uia_event.rs 协作)
│   ├── reader.rs           # [保留 spike 原版] read_path(可与 uia_inject 共享 ValuePattern 调用模式)
│   └── writer.rs           # [保留 spike 原版] write_path(SetValue + SendInput fallback,改成与 uia_inject 共享 InvokePattern)
├── scripts/
│   ├── verify_target_matrix.ps1  # [修 spike 原版] 加 PATH fallback wrapper(用 `F:\soft\00selfmade\filemanager\.other\scripts\run-verify-with-paths.ps1` 模式)
│   └── smoke-test.ps1            # [保留 spike 原版]
└── docs/SPIKE_REPORT_TEMPLATE.md  # [保留 spike 原版]
```

**保留 spike 原版范围** = 7 个文件(spike 写得好的部分不动)。**新增 3 个文件,从外部搬运**(实际可写,因为是 MIT/Apache-2.0 license)。

---

## 四、必交付清单(新会话产出顺序)

1. **Task 0 — 更新 plan/spec 文档**:`docs/superpowers/plans/2026-09-27-listary-focus-sync.md` 改为新路线;`docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md` 调整或归档
2. **Task 1 — Cargo.toml**: spike 子目录重新初始化,`windows = "0.62"` + tokio + tracing + serde + anyhow + tracing-subscriber + clap
3. **Task 2 — 移植 PathWrap dialog.rs → `uia_inject.rs`**: 数据类型适配(spike 用 u32 hwnd,PathWrap 用 isize)、保留打分函数、保留 InvokePattern
4. **Task 3 — 移植 PathWrap monitor.rs → `uia_event.rs`**: **关键难点** — 替换 `egui::Context` 为 spike 的 tokio 同步原语(建议用 `tokio::sync::Notify` 或 `Arc<AtomicBool>`),保留三事件 hook + mpsc 唤醒
5. **Task 4 — 移植 QwenLM fg_bypass.rs → `fg_bypass.rs`**: 适配 windows 0.58 → 0.62 API(主要是 `BOOL` / `HWND::is_null()` vs `is_invalid()`);**简化门控条件**,spike 只需要 `is_chromium_target_window`(去掉 UWP/XAML 分支,spike 没有 cua-driver 的 `is_xaml_host_hwnd` 实现)
6. **Task 5 — cargo build --release**: **关键验证点**(windows 0.58 → 0.62 实际兼容性)
7. **Task 6 — cargo test --lib**: 8/8 通过(spike 原版的 state / config / events 单测应继续过)
8. **Task 7 — 集成到 main.rs**: 替换原 `detector.rs / reader.rs / writer.rs` 的 UIA 调用为新模块的 API;但保留这些文件作为兼容层
9. **Task 8 — 实跑 verify_target_matrix.ps1**: 用 PATH fallback wrapper(模板已存在,见 `.other/scripts/`),真实填 7-app 矩阵
10. **Task 9 — 填 spike-report + decision doc**: 预期 PASS(覆盖率 5-7/7)
11. **Task 10 — 更新 sigma-file-manager/README.md**: 基于真实覆盖率

---

## 五、环境与基线

- **Working dir**: `F:\soft\00selfmade\filemanager\`
- **Branch**: `main`(新路线不打算开 feature branch,直接进 main)
- **HEAD**: `f21c581 docs(plan): Listary focus sync spike — 14 tasks over 5 days`
- **Tracked 状态**: 已清洁
- **Untracked**: 仅 `.other/`(调研素材)
- **`feat/tree-sidebar-v6-1`**: HEAD `8caf14ae`,6 commits,259 unit tests pass — **不动**
- **`.other/` 内容**:
  - `doc/00-research-overview.md`(5 句话)
  - `doc/01-uia-windows-rust-csharp.md`(PathWrap / QwenLM / Lertaro)
  - `doc/02-ahk-script-based.md`(ALTRun / folder-jump)
  - `doc/03-tauri-commercial-refs.md`(Lertaro / Listary SDK)
  - `doc/04-macos-linux-powertoys.md`(Hammerspoon / PowerToys)
  - `src/PathWrap/`(git clone,完整源码)
  - `src/qwen-code/`(sparse-checkout,只含 `packages/cua-driver/rust/crates/platform-windows/`)

---

## 六、避坑提示(累积,新会话必读)

### 6.1 PathWrap `dialog.rs` 移植注意

- Cargo.toml 包名是 **`PathWarp`**(注意拼写),不影响移植
- `thread_local IUIAutomation` 实例(RefCell):**符合 windows crate COM !Send 约束,必须保留**
- 打分函数:`filename_edit_score` 看 `AutomationId == "1148"` → 100 / `"1001"` → 90 / 名字含 "文件名" / "file name" → 80;`confirm_button_score` 看 `AutomationId == "1"` (IDOK) → 100 / 名字含 "打开" / "保存" / "open" / "save" → 80
- `fallback_confirm`: 用 `SendMessageW(native, WM_KEYDOWN, Some(WPARAM(VK_RETURN)), ...)` + WM_KEYUP 发到 edit 的 native window handle — **不是 SendInput**,**不抢焦点**,比 spike 现状更稳
- `windows 0.62` 类型适配:`HWND(hwnd_isize as *mut core::ffi::c_void)` 真实构造(spike 之前的 plan 里写的 `HWND_VALIDATION` 是占位假类型)
- `BSTR::from(target_path)` 直接传入

### 6.2 PathWrap `monitor.rs` 移植注意

- **强耦合 `egui::Context`**(在 `start_monitor` 签名里传 `ctx: egui::Context`,且 `ctx.request_repaint()` 调用):**必须替换为 spike 的 tokio 原语**
- 推荐方案: 把 `egui::Context` 替换为 `Arc<tokio::sync::Notify>`,`request_repaint()` 替换为 `notify.notify_one()`;main 循环里 `notify.notified().await` 触发 polling
- **`SetWinEventHook` callback 在独立线程**: 与主循环通信用 `mpsc::channel`,callback 持全局 `OnceLock<Mutex<Option<Sender>>>` 发 `()` 唤醒
- 三事件 hook: `EVENT_SYSTEM_FOREGROUND | EVENT_OBJECT_FOCUS | EVENT_OBJECT_SHOW`,全部用 `WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS`
- 自适应 poll:`TRACKING_POLL_INTERVAL_MS = 8` / `IDLE_POLL_INTERVAL_MS = 30`,**已经够细**,不必再优化
- **lost_ticks 抖动恢复**: 3 次丢失才认为 dialog 消失(防止抖动)
- 对话框识别多重特征(防止误检 `#32770` 系统对话框):
  - 必须含 `#32770` 类名
  - 标题关键词:`打开` / `保存` / `另存为` / `选择` / `open` / `save` / `select`
  - 控件结构(任一): `ComboBoxEx32 + (DirectUIHWND | SHELLDLL_DefView | DUIViewWndClassName)` OR `DirectUIHWND + SHELLDLL_DefView` OR `DirectUIHWND + DUIViewWndClassName`
  - 弱匹配: 标题匹配 + 任一控件存在
- DPI 感知:`DwmGetWindowAttribute(DWMWA_EXTENDED_FRAME_BOUNDS)` + `GetDpiForWindow`(不是 GetWindowRect)
- 新会话也可以参考我读过的源码,在 spike 子目录用 `dia::structural_match(hwnd) -> bool` 形式抽出这部分

### 6.3 QwenLM `fg_bypass.rs` 移植注意

- **windows 0.58 → 0.62 API 差异**(可能遇到的):
  - `EnableWindow`: 在 `windows::Win32::UI::Input::KeyboardAndMouse`,**API 一致**
  - `BOOL` 类型: 0.58 用 `winapi::shared::minwindef::BOOL`,0.62 用 `windows::core::BOOL`(新类型);`as_bool()` 方法名一致
  - `HWND::is_null()` vs `HWND::is_invalid()`: **0.62 用 `is_invalid()`**,0.58 用 `is_null()` — 实际编译时如果失败,把 `hwnd.0.is_null()` 改成 `hwnd.is_invalid()`
  - `GetAncestor`: 仍在 `windows::Win32::UI::WindowsAndMessaging`,API 一致
  - `GA_ROOT`: 同上
- **门控条件简化**:QwenLM 的 `is_xaml_host_hwnd` 在 `crate::input` 模块里,**spike 没有这些**。新会话移植时只需要保留 `is_chromium_target_window`(简单实现:`get_class_name(hwnd).starts_with("Chrome_WidgetWin_")`)。**UWP/XAML 分支可以先删掉**,spike 实际场景中飞书/钉钉/微信 不是 UWP
- **`DisabledHwndGuard` RAII pattern**:Drop 时 `EnableWindow(self.hwnd, self.was_enabled)` 恢复,即使 panic 也安全(用 `armed: bool` 标记)
- **只包 UIA Invoke 调用**:`SetValue` 不需要(只 Invoke 是抢焦点的入口);用 `run_with_uwp_bypass(host_hwnd, || { invoke.Invoke()? })` 模式
- 实测效果: 不用 bypass → Chromium 7/8 background ax-bg 抢焦点;用 bypass → 0/507(Calculator + Clock + Settings)

### 6.4 调研报告局限性

- **QwenLM windows 版本错了**:报告说 0.62,实际 0.58(我已确认,见 Cargo.toml)
- **报告低估代码量**: QwenLM mod.rs 实际 1437 行(报告说 ~400 行),windows_enum.rs 1216 行(报告未提)
- **报告未提的 QwenLM 能力**: `cache_uaf_repro.rs`(测试)、revision.rs / scroll.rs(单点不需要)
- 建议:新会话**第一步先 Read 我已读过的 4 个文件**(dialog.rs / monitor.rs / fg_bypass.rs / Cargo.toml),我已确认可照搬

### 6.5 历史经验(从前次 spike 接力沉淀)

- `state.current_path` API:**现在是 `Mutex<String>`**(不是 `String`),访问用 `set_current_path(&self, p)` + `get_current_path(&self) -> String`(镜像 `registry` 模式)— spike Task 9 fix
- `app:"unknown"` 占位:`poll.rs:53` 每次检测都发 `"unknown"`(class-name → app-name 映射未实现)— verify 脚本走 HWND 匹配
- `tokio::select!` drop order:`main.rs:93-115` 的 `select!` 返回 → 4 个分支按声明顺序 drop → `_auto` drop → `CoUninitialize`;任何分支都不能在 select body 内持有 `state.lock()` 跨过返回点
- `app_whitelist` flag:CLI args `--list-app` 空数组 = 检测所有(spike 阶段新增)— 新路线保留
- Unit test:`state::tests`(2 个)、`config::tests`(3 个)、`events::tests`(3 个)— 新路线 cargo test 应继续通过 8/8

### 6.6 不要做的事

- ❌ 不要开 `feat/dialog-focus-sync` 分支(死代码,counter-example,从未存在过)
- ❌ 不要 rebase `feat/tree-sidebar-v6-1` (HEAD `8caf14ae`,已发稳定)
- ❌ 不要走 14-task 自写 spike 路线(本次已废)
- ❌ 不要试图在本轮做 macOS / Linux(Windows only)
- ❌ 不要碰 release/ 目录(已删,重建要上游下载)
- ❌ 不要现在改 `sigma-file-manager/README.md`(等 Task 10 才改)
- ❌ 不要跳过 cargo build --release 验证(Task 5 是关键验证点)

---

## 七、必读顺序(新会话第一件事)

1. **本 handoff**(你正在读)
2. `git rev-parse HEAD` — 确认在 `f21c581`
3. `git status --short` — 确认 working tree 只有 `?? .other/`
4. **Read** `.other/doc/00-research-overview.md`(5 句话执行摘要)
5. **Read** `.other/src/PathWrap/src/os/dialog.rs`(176 行,直接搬)
6. **Read** `.other/src/PathWrap/src/os/monitor.rs`(399 行,移植要解耦 egui)
7. **Read** `.other/src/qwen-code/packages/cua-driver/rust/crates/platform-windows/src/uia/fg_bypass.rs`(129 行,适配 windows 0.62)
8. **Read** `.other/src/PathWrap/src/os/mod.rs`(5 行,模块导出)
9. **Read** `.other/src/PathWrap/Cargo.toml`(确认 features 与 spike 一致 + 多了几个)
10. **Read** `.other/src/qwen-code/packages/cua-driver/rust/crates/platform-windows/Cargo.toml`(确认 QwenLM 实际 windows 版本 = 0.58)
11. **Read** `docs/superpowers/plans/2026-09-27-listary-focus-sync.md`(旧版 spike 路线,需要更新)
12. **Read** `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md`(旧版 spike 设计,需要基于新路线调整)

---

## 八、新会话第一句话(引用本 handoff + prompt)

> 承接 `handoff-2026-09-27-focus-activation-uia-port.md`,当前 HEAD `f21c581`,新路线是基于 PathWrap + QwenLM fg_bypass 移植,不走 14-task 自写 spike。读完 handoff §四必交付清单 + §六避坑提示后,从 §七必读顺序开始执行。

---

## 九、Out of scope(继续不做)

- macOS / Linux(UIA / AXUIElement 是 Windows 专有,调研报告建议 v3+)
- DLL 注入(DLL injection,AV / WDAC 风险,Plan §1 禁止)
- 重新开 `feat/dialog-focus-sync` 分支(死代码,counter-example)
- rebase `feat/tree-sidebar-v6-1`(稳定已发)
- Phase 2(命名管道、3 进程架构、IFileAppPlugin contract)— 当前 spike 验证 PASS 后再考虑

---

## 十、tree 功能当前状态(继续可用)

- `feat/tree-sidebar-v6-1` HEAD `8caf14ae`,6 commits,259 unit tests pass
- `v2.2.0-tree.1` release 仍在 GitHub upstream(本地 `release/` 目录被本轮 reset 删除,但上游可下载)
- 新路线在 spike 子目录,不影响 `sigma-file-manager/` 主项目