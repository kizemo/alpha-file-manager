# Listary-Style Global Focus Sync — Spike Spec

**日期**: 2026-09-27
**承接**: `handoff-2026-09-27-listary-focus-sync-retract.md` (1.0 撤回)
**目的**: 在投入完整实现前,用 1 周 UI Automation spike 验证 external file dialog 的 detect/read/write 三项能力在 7 个 target app 上的覆盖率。**只有覆盖率 ≥ 70% 才进入完整实现;否则承认不可行(README 增加限制说明,不进入方案 B 的杀软风险路径)。**
**Spec 范围**: 只覆盖 spike 阶段(Sigma 集成阶段另开 spec)。

---

## 1. 背景与动机

### 1.1 1.0 撤回原因(参考)

`feat/dialog-focus-sync` 分支 11 个 commit 实现了 "Sigma 内部 picker 跟随" 但无 UI 入口触发,scope 错配,撤回。
详细:见 handoff §II。

### 1.2 本次真实目标

Listary 风格:任何 Windows app 的 file dialog(Chrome 下载、Word 另存为、VS Code 打开文件夹、钉钉/飞书/微信另存为)Alt-Tab 到 Sigma 浏览新路径后,Alt-Tab 回 dialog 时自动跳到 Sigma 当前目录。

这是商业软件 10+ 年的护城河。本 spec 不假装能做到,先 spike 验证可行性。

### 1.3 上游 PR + fallback 发版策略

- 完整实现(spike 通过后)→ 提交上游 PR
- 上游不合并 → merge 到 `feat/tree-sidebar-v6-1` 分支 → 改名 release 独立发版
- 因此 spike 代码本身**不进 upstream**,但 spike 后的实现要遵循 GPL-3.0-or-later(项目已有)

---

## 2. 设计

### 2.1 Architecture

```
┌────────────────────────────────────────────────────────────────┐
│  Spike binary (sigma-listary-spike/)                           │
│                                                                 │
│  ┌──────────────────┐  ┌─────────────────┐  ┌──────────────┐ │
│  │ UIA Listener     │  │ TCP Server      │  │ JSON Logger  │ │
│  │ (main thread)    │  │ (127.0.0.1:    │  │ (stdout)     │ │
│  │ CUIAutomation +  │  │  37421)         │  │              │ │
│  │ FocusChanged     │  │ "set_path <p>"  │  │              │ │
│  │ EventHandler     │  │ "get_status"    │  │              │ │
│  └────────┬─────────┘  └────────┬────────┘  └──────┬───────┘ │
│           │                     │                   │         │
│           └──────────┬──────────┘                   │         │
│                      ▼                              │         │
│           ┌──────────────────────┐                  │         │
│           │ Dialog State Registry │                  │         │
│           │ Mutex<HashMap<        │                  │         │
│           │  HWND, DialogInfo>>   │                  │         │
│           └────────┬─────────────┘                  │         │
│                    │                                 │         │
│           ┌────────▼──────────┐                     │         │
│           │ Read/Write Engine │ ────────────────────┘         │
│           │  - UIA ValuePattern::CurrentValue                  │
│           │  - UIA ValuePattern::SetValue                      │
│           │  - SendInput Ctrl+L fallback                      │
│           └──────────────────────────────────────┘            │
└────────────────────────────────────────────────────────────────┘
```

### 2.2 Components

| 文件 | 职责 |
|---|---|
| `Cargo.toml` | windows 0.62 + tokio + serde + serde_json + anyhow + tracing |
| `src/main.rs` | entry: 启动 UIA listener + TCP server |
| `src/config.rs` | 端口 / 日志级别 / target app whitelist |
| `src/detector.rs` | `DialogDetector` — HWND → is_file_dialog |
| `src/reader.rs` | `PathReader` — UIA ValuePattern 读 path |
| `src/writer.rs` | `PathWriter` — UIA SetValue → SendInput fallback |
| `src/state.rs` | DialogState registry (Mutex<HashMap<HWND, DialogInfo>>) |
| `src/uia.rs` | UIA EventHandler 注册 + COM init/shutdown |
| `src/tcp_server.rs` | tokio TCP server (set_path / get_status / quit) |
| `src/logger.rs` | JSON stdout 日志(脚本聚合用) |
| `scripts/verify_target_matrix.ps1` | 自动化 target matrix 验证脚本 |
| `scripts/verify_target_matrix.sh` | bash 版本(WSL/Git Bash) |
| `docs/SPIKE_REPORT_TEMPLATE.md` | spike 结果报告模板 |
| `README.md` | 编译 + 跑 + 验证流程 |

### 2.3 Data Flow

#### 2.3.1 Read path(外部 dialog 出现 → spike 读 path)

1. Win32 foreground change → UIA `FocusChanged` event 触发
2. Spike handler 取新 foreground HWND
3. `DialogDetector::detect(HWND)`:
   - `GetClassNameW(HWND)` → `#32770`(标准 Win32 dialog class)/`Chrome_WidgetWin_1`(Chromium)/等
   - 枚举 UIA tree 找 `AutomationId="Address"` / `Name="File name"` / `ControlType=Edit` 在 toolbar
   - 都找不到 → not a file dialog, skip
4. detect 返回 true → 进入 read
5. `PathReader::read_path(HWND)`:
   - 找 address bar element → 拿 `ValuePattern`
   - `CurrentValue` → String
   - 校验是有效 Windows path(Drive letter 或 UNC)
   - 失败 → `ReadError::NoValuePattern` / `ReadError::InvalidPath`
6. 写 `DialogState.set(hwnd, path)`
7. JSON 日志:
   ```json
   {"event":"read","hwnd":12345,"app":"chrome","path":"C:\\Users\\foo","strategy":"uia","ts":"2026-09-27T..."}
   ```

#### 2.3.2 Write path(Sigma 当前路径变化 → 写 dialog)

1. TCP server 收到 `{"cmd":"set_path","path":"C:\\Users\\foo\\new"}`
2. `state.current_path = new_path`
3. 对 `state.registry` 中每个活跃 HWND:
   a. `PathWriter::write_path(HWND, new_path)`:
      - **Try 1: UIA `ValuePattern::SetValue(new_path)`**
        - 成功 → `WriteOutcome::UiaSetValue`,日志
        - 失败(`UIA_E_NOT_SUPPORTED`/元素只读) → fallback
      - **Try 2: SendInput Ctrl+L 序列**
        - `keybd_event VK_CONTROL` down
        - `keybd_event VK_L` down/up
        - 释放 Ctrl
        - `SendInput` 输入 path chars
        - `SendInput VK_RETURN`
        - `Sleep(300ms)` 等待 dialog 更新
        - 重新 read → 校验是否匹配 `new_path`
          - 匹配 → `WriteOutcome::SendInputFallback`,日志
          - 不匹配 → `WriteError::SendInputFailed`,标记 HWND failed(不重试,避免循环)
      - 都失败 → `DialogState.mark_failed(hwnd)`
4. JSON 日志 per HWND:
   ```json
   {"event":"write","hwnd":12345,"app":"chrome","target":"C:\\new","strategy":"uia|sendinput|failed","ts":"..."}
   ```

#### 2.3.3 Focus change 触发流程(dialog 关闭)

1. UIA `FocusChanged` event → 新 HWND 不在 registry
2. 清理 `state.registry` 中消失的 HWND
3. JSON 日志:
   ```json
   {"event":"dialog_closed","hwnd":12345,"app":"chrome","last_known_path":"C:\\Users\\foo","ts":"..."}
   ```

#### 2.3.4 TCP Server Protocol

Line-based JSON,简单可靠:

| 请求 | 响应 |
|---|---|
| `{"cmd":"set_path","path":"C:\\Users\\foo"}` | `{"ok":true}` |
| `{"cmd":"get_status"}` | `{"ok":true,"current_path":"...","active_dialogs":3,"dialogs":[...]}` |
| `{"cmd":"quit"}` | `{"ok":true}`,server 关闭 |
| 任何错误 | `{"ok":false,"error":"..."}` |

默认端口 37421。被占用时自动尝试 37422..37499,全占用则 panic。

### 2.4 Error Handling

| 错误类型 | 严重度 | 处理 |
|---|---|---|
| `UIA_E_NOT_SUPPORTED`(SetValue 不支持) | ⚠️ Warning | 自动 fallback SendInput,日志记 strategy="sendinput" |
| `UIA_E_ELEMENTNOTAVAILABLE`(dialog 已关闭) | ℹ️ Info | 清理 state entry,日志 event="dialog_closed" |
| SendInput 被拒绝(Ctrl+L 无响应) | ⚠️ Warning | 写入失败,标记 HWND failed,**不重试** |
| TCP 连接断开 | ℹ️ Info | server 继续运行 |
| 端口被占用 | ⚠️ Warning | 自动尝试下一端口 |
| Cargo build 失败 | ❌ Fatal | panic + stderr |
| COM init 失败 | ❌ Fatal | panic + stderr |
| UIA EventHandler 注册失败 | ❌ Fatal | panic + stderr |

**Failover 原则**:
- Read:失败一次就放弃,等下次 focus change 触发
- Write:失败一次就放弃(避免重复 Ctrl+L 把 dialog 弄乱)
- Detect:失败不写入 registry

**Spike binary 退出码**:
- `0` = 正常退出(用户 quit / Ctrl+C / 自动化脚本完成)
- `1` = 致命错误
- `2` = target matrix 验证完成且覆盖率 < 70%(自动化脚本用)

**Dev vs Release**:
- dev build 加 `debug_assert!` 校验 UIA tree 假设
- release build 关掉 debug_assert(diagnostic tool 不 crash 在用户机器)

### 2.5 Testing & Acceptance

#### 2.5.1 Spike 阶段测试方法

**手动 smoke test**(开发者本机):
```bash
cargo run --release
# 另一终端:启动 Chrome 下载 → 触发 "另存为" dialog
echo '{"cmd":"set_path","path":"C:\\Users\\foo"}' | nc 127.0.0.1 37421
# 观察 dialog 跳到 C:\Users\foo
# 观察 spike stdout JSON 日志
```

**自动化 target matrix 验证**(`scripts/verify_target_matrix.ps1`):
```powershell
# 启动 spike (后台)
# 对每个 target app 启动 + 触发 dialog + 等待稳定
# 通过 TCP 发送 set_path 命令
# 读取 spike 日志,提取 read/write event
# 判定每个 app 的 detect/read/write 3 项能力是否 pass
# 统计覆盖率 = pass_count / (apps × 3)
# 输出 PASS_RATE + 退出码 (>= 70% = exit 0, < 70% = exit 2)
```

#### 2.5.2 Target Matrix Acceptance Criteria

每个 app 3 项能力(detect / read / write)必须**全 pass** 才算该 app 通过。

| App | detect 特征 | read 路径 | write 路径 |
|---|---|---|---|
| **Chrome 下载** | class=#32770 或 Chrome_WidgetWin_1 + UIA tree 有 Address toolbar | UIA ValuePattern 读出 path 形如 `C:\Users\foo\Downloads` | SetValue 或 SendInput 让 dialog 跳到新 path,再 read 验证 |
| **Edge 下载** | 同 Chrome(Chromium 内核相同) | 同 Chrome | 同 Chrome |
| **Word 另存为** | class=#32770 + Title 含 "另存为"/"Save As" | UIA ValuePattern 读出 path | 同 Chrome |
| **VS Code 打开文件夹** | class=Chrome_WidgetWin_1(Electron)+ Title 含 "Open Folder" | UIA ValuePattern 读出 path | 同 Chrome |
| **钉钉 另存为** | class=Chrome_WidgetWin_1(Electron)+ Title 含 "Save"/"保存" | UIA ValuePattern 读出 path | 同 Chrome |
| **飞书 另存为** | 同钉钉 | 同钉钉 | 同钉钉 |
| **微信 另存为** | class=WeChatMainWndForPC(单独)+ 自定义 dialog | UIA ValuePattern 读出 path | 同 Chrome |

**特殊处理**:
- 钉钉/飞书/微信:Electron 包装 Chromium,Chromium 自己 dialog class 可能是 `Chrome_WidgetWin_1` 或 wrapper 自定义 class。Spike 需枚举两种可能性
- 微信:Windows 客户端混合架构,部分 dialog 可能用 `GetOpenFileName` 老 API(无 UIA 支持),spike 阶段需记录并标注

#### 2.5.3 覆盖率判定

```python
coverage = sum(1 for app in apps if app.detect and app.read and app.write) / len(apps)
# 5/7 ≈ 71.4% → PASS (>= 70%)
# 4/7 ≈ 57.1% → FAIL (escalate 到 README 限制说明)
```

**Pass**:进入 Sigma 集成阶段(另开 spec + plan)。
**Fail**:
- README 增加 "Listary hook: UIA 覆盖不够,需要商业软件复杂度(DLL 注入)" 限制说明
- 不进入方案 B(DLL 注入,杀软风险)或方案 C(Hybrid)
- 公开记录失败原因与覆盖率数据,留给上游参考

---

## 3. 关键决策与依据

### 3.1 为什么 spike 单独成 meta-repo 子目录?

**候选**:
- (A) `meta-repo/sigma-listary-spike/`(本 spec 选)
- (B) sigma-file-manager workspace 临时 binary
- (C) 独立 Git repo
- (D) sigma Cargo workspace binary + gitignore

**选 A 因为**:
- spike 代码与 Sigma 实现解耦,spike 通过后 Sigma 集成用全新代码,不污染 spike 历史
- 不进 upstream,所以 git history 不需与 upstream 同步
- 但 spike 结果需要进 meta-repo docs/(与 PR strategy spec 等并列),子目录共享父目录 git history

### 3.2 为什么 spike 用 UIA + SendInput 双 fallback?

**候选**:
- (A) UIA ValuePattern::SetValue + SendInput fallback(本 spec 选)
- (B) 仅 SendInput
- (C) UIA 读 + WM_COPYDATA 写

**选 A 因为**:
- UIA SetValue 是最快、最 atomic 的方式(无 user 输入干扰)
- 但 Chrome/Edge 的地址栏可能 UIA lock,SendInput 是通用 fallback
- WM_COPYDATA 需要向目标进程发消息,部分 EDR 报警,且 spike 阶段不需冒这个风险
- 双 fallback 让 spike 验证每 app 实际走哪条策略 — 数据驱动未来 Sigma 集成实现

### 3.3 为什么失败 fallback 是 "承认不可行"?

**候选**:
- (A) 承认不可行 + README 限制说明(本 spec 选)
- (B) 走 Hybrid 方案 C
- (C) 走 DLL 注入方案 B
- (D) 重新选 target matrix

**选 A 因为**:
- 方案 B (DLL 注入) 几乎所有杀软标记为可疑,需要 EV 代码签名证书,Windows Defender Application Control (WDAC) 会拒绝。商业软件级别的护城河,fork 不应走这条路
- 方案 C (Hybrid) 等于放弃 UIA 写的优势,SendInput 100% 依赖,UX 体验下降(每次 ~500ms 延迟 + 非 atomic)
- 重新选 target matrix 等于自我欺骗,降低门槛不是真正成功

### 3.4 为什么 TCP 控制而非 Sigma 进程内集成?

**候选**:
- (A) CLI 参数 + TCP server(本 spec 选)
- (B) 纯 stdin
- (C) Sigma 进程作为 spike 的 client(直接 spawn + socket fd)

**选 A 因为**:
- spike binary 需独立可执行(开发期手动测、自动化脚本测、未来 Sigma 集成测都复用同一 binary)
- TCP 端口可被外部脚本/PowerShell 触发,验证灵活
- 未来 Sigma 集成阶段:TCP server 替换 transport(unix domain socket 或 named pipe),binary 本身或 spawn 为子进程

---

## 4. 范围与限制

### 4.1 In Scope

- Spike binary 实现(UIA listener + TCP server + read/write 引擎)
- 7 个 target app 的 detect/read/write 验证
- 覆盖率判定与目标 70% 门槛
- 自动化验证脚本(PowerShell + bash)
- Spike 结果报告(docs/SPIKE_REPORT_TEMPLATE.md)
- README 增加 spike 流程说明

### 4.2 Out of Scope

- Sigma 集成阶段(另开 spec)
- 多 dialog 并存 stability 长跑测试(Sigma 集成阶段做)
- 焦点快速切换 stress test(Sigma 集成阶段做)
- DLL 注入方案 B(商业软件护城河,不进入)
- macOS / Linux 平台(Win11 first)
- 旧 dialog API(`GetOpenFileName`)支持(部分 Win32 legacy dialog 无 UIA,spike 阶段记录不支持的 app)

### 4.3 假设

- Win11 22H2+ 作为测试平台(UIA 现代 API 行为稳定)
- 7 个 target app 都是 Win11 上可获取的应用(Chrome/Edge 默认;Office 假设用户有许可;VS Code 用户主动安装;钉钉/飞书/微信假设用户主动安装)
- `windows` crate 0.62 与现有 Sigma 依赖版本兼容(已在用)
- TCP 端口 37421 在用户机器空闲

---

## 5. 验收交付物

Spike 完成后必须交付:

1. **Spike binary release**(`sigma-listary-spike/target/release/spike.exe`,sha256 记录)
2. **Spike 报告**(`docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md`):
   - target matrix 表格(detect/read/write 每 app 的 PASS/FAIL + 失败原因)
   - 覆盖率统计(PASS_RATE + 通过/总 app 数)
   - 结论(PASS 进入 Sigma 集成 / FAIL 走 README 限制说明)
3. **更新 README.md / README.zh-CN.md**:
   - Pass 情况:加 "Listary-style focus sync (Experimental)" 段,说明 limitation + 启用方式
   - Fail 情况:加 "Listary hook 限制说明" 段,说明 UIA 覆盖率不足 + 不进入 DLL 注入
4. **Decide 文档**:`docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md` 记录 go/no-go 决策

---

## 6. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| windows crate 0.62 与 tokio mpsc 集成有死锁 | 中 | spike 不稳 | spike 早期用单线程 event handler,后续按需拆 |
| UIA 在某些 Win11 版本行为不一致 | 中 | 覆盖率波动 | spike 测试至少 2 个 Win11 build |
| SendInput 被 dialog 主动拒绝(Ctrl+L 屏蔽) | 中 | 写入失败 | 记录失败 app,作为覆盖率损失数据 |
| 钉钉/飞书/微信是 Electron,UIA 暴露可能不完整 | 高 | 3/7 app 失败 | 接受覆盖率 < 70% 时按决策 §3.3 承认不可行 |
| Spike binary 误把普通窗口当 file dialog | 中 | 误写 | detector 多条件 AND,避免 false positive |
| 自动化脚本启动 target app 路径不一致 | 低 | 测试 flaky | 脚本用 start-process + wait,加超时 |

---

## 7. 时间线(1 周 = 5 工作日)

| Day | 任务 |
|---|---|
| Day 1 | Cargo project 初始化 + windows crate 编译通过 + UIA EventHandler 注册 demo |
| Day 2 | DialogDetector + PathReader 实现 + Chrome 单 app smoke test |
| Day 3 | PathWriter 实现(UIA SetValue + SendInput fallback)+ Chrome smoke test 全闭环 |
| Day 4 | 7 个 target app 自动化验证脚本 + 跑第一轮 + 收集数据 |
| Day 5 | spike 报告撰写 + README 更新 + decide 文档 + handoff |

---

## 8. 必读文件

1. 本 spec
2. `handoff-2026-09-27-listary-focus-sync-retract.md` — 1.0 撤回原因
3. `docs/superpowers/plans/2026-09-26-dialog-focus-sync.md` — 1.0 失败的 plan(反例参考,**不要按它做**)
4. `docs/superpowers/specs/2026-09-26-upstream-pr-strategy-design.md` — PR 沟通策略(对话模板仍适用)
5. `sigma-file-manager/feat/dialog-focus-sync` 分支 — 代码作为 reference,**不要扩展/cherry-pick/release**

---

## 10. Route Change — Port over Self-Write (2026-09-27)

**Original self-written spike (Tasks 1-14 in old `plans/2026-09-27-listary-focus-sync.md`)**: 19 commits / +3018 LoC. Reset on 2026-09-27 because 7-app matrix verification was structurally infeasible (interactive scripts blocked, only 4/7 apps installed locally, max coverage 57% < 70% threshold).

**New port route** (current `plans/2026-09-27-listary-focus-sync.md`): 11 tasks focused on lifting 3 files (~705 LoC total) from open-source projects that already validate the technique:

| File | Status | Implementation |
|---|---|---|
| `src/uia_inject.rs` (176 LoC) | **Port** from `inaku-Gyan/PathWrap/src/os/dialog.rs` (MIT) | UIA `ValuePattern::SetValue` + `InvokePattern::Invoke` + `SendMessageW(WM_KEYDOWN, VK_RETURN)` fallback |
| `src/uia_event.rs` (399 LoC) | **Port** from `inaku-Gyan/PathWrap/src/os/monitor.rs` (MIT) | `SetWinEventHook` (3 events) + adaptive polling (8/30ms) + lost-tick recovery; **replace `egui::Context` with `tokio::sync::Notify`** |
| `src/fg_bypass.rs` (129 LoC) | **Port** from `QwenLM/qwen-code/.../fg_bypass.rs` (Apache-2.0) | `EnableWindow(FALSE)` RAII guard during UIA Invoke for Chromium hosts |

**Why port over self-write:**
- PathWrap uses identical `windows` 0.62 stack — direct compatibility
- QwenLM `fg_bypass` empirically validated: Chromium 7/8 → 0 z-drops with shield
- Coverage target 5-7/7 = 70-100% (vs original self-write max 57%)
- 3.5-4 day ETA vs 8-10 days from scratch

**License obligation:** per-file header preserving MIT/Apache-2.0 attribution. Both licenses permit redistribution with attribution. Spike binary remains internal meta-repo artifact (not pushed to upstream `kizemo/sigma-file-manager`).

**Files modified by route change:**
- `docs/superpowers/plans/2026-09-27-listary-focus-sync.md` — replaced 14-task self-write with 11-task port
- `sigma-listary-spike/src/uia_inject.rs`, `uia_event.rs`, `fg_bypass.rs` — new (ported, not self-written)
- `sigma-listary-spike/src/{config,events,logger,state,tcp_server,detector,reader,writer}.rs` — supporting scaffold (reconstructed from old plan since spike subdir was reset)

**Reference research:** `.other/doc/00-research-overview.md` + `.other/doc/{01..04}-*.md` (4 parallel research agents, 75+ GitHub citations).

---

## 9. 严禁事项

1. **绝对不要**跳过 spike 直接开始完整实现(违反 brainstorming 共识)
2. **绝对不要**扩展 / cherry-pick / release `feat/dialog-focus-sync` 分支(死代码,反例保留)
3. **绝对不要**rebase / squash `feat/tree-sidebar-v6-1` 分支(稳定已发布,HEAD 8caf14ae)
4. **绝对不要**走 DLL 注入方案 B(杀软风险 + EV 证书 + WDAC 拒绝,商业软件护城河)
5. **绝对不要**让 spike binary 进 upstream PR(spike 是 meta-repo 内部验证工具)
6. **绝对不要**spike 覆盖率 < 70% 时降门槛通过(按 §3.3 承认不可行,README 增加限制说明)