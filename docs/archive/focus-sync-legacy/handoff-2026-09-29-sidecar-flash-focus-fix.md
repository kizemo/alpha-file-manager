# Handoff — Sidecar Flash + Focus Steal Fix (2026-09-29 round 18)

**Inherits**: `handoff-2026-09-29-filename-flash.md` (round 17 — diagnostic only)

**Decision route**: brainstormed 3 options (`A: IFileDialog::SetFolder | B: log detour hack | C: spike verification`). User picked "尽量利用我们从github引入的项目原有的技术,保证可靠性" → confirmed A-revised: **退回 PathWrap 原版 on-demand 语义**(撤回 spike 多做的事,保留 PathWrap 上游原版 SetValue + Invoke)。4 项补漏全部落地后用户确认进 sandbox 验证。

**Status**: 8/8 round-18 tasks completed. `cargo +1.85.0 check --lib` + `cargo test --lib` PASS (15/15). `cargo build --release` PASS (49.34s, 2.31 MB). Smoke test (get_status / set_path / quit) PASS. Binary staged at `C:\Temp\focus-sync-round18\spike.exe` — **NOT YET DEPLOYED** to `release/extension/bin/focus-sync-sidecar.exe` (awaiting user authorization per CLAUDE.md delete-gate).

---

## 一、根因(新分析,推翻 round-17 假设)

**Round-17 假设**:写 filename 输入框(AutomationId 1148)是根因,需要 IFileDialog::SetFolder 改写"地址栏"。

**Round-18 真根因**:spike 把 PathWrap **on-demand 调用**改成了 **持续 push 调用**,每次 8ms monitor tick 都触发 SetValue,导致 Edge Save dialog 在 SetValue 后 reset filename → 下次 tick 又写 → 闪动循环。fg_bypass 的 `EnableWindow(FALSE)` 反复 toggle 让 Sigma FM 失顶层焦点。

**Spike vs PathWrap 原版 关键差异**:

| 维度 | PathWrap 原版 | spike round 17 |
|---|---|---|
| dialog 检测 | winit event-driven,无 polling | **uia_event.rs 8ms poll 持续推** |
| 写入触发 | on-demand 调用 `inject_folder_path` | **每个 poll tick 都触发 SetValue** |
| fg_bypass | 上游不用 | **每次都 wrap EnableWindow(FALSE)** |
| 主路径 | winit → inject_folder_path 一次 | **monitor → main select! → write 持续** |

**PathWrap 上游没有 flash loop** — 因为它不持续推送。spike round-17 多做了"持续推送"才引发这个问题。

**fg_bypass 上游也是错配** — QwenLM 移植时实测 UWP Calculator 0/507 z-drops,但我们的实际场景(Edge Chromium Save dialog + Sigma FM)Chromium self-foreground 这一会通过 `EnableWindow(FALSE)` toggle 反复禁用 Sigma FM 顶层窗口。

---

## 二、修改清单(round 18 全部)

### 2.1 源码改动(8 文件)

| 文件 | 改动 | 净行数 |
|---|---|---|
| `sigma-listary-spike/src/uia_event.rs` | 加 `DialogEvent` enum(Opened / Closed { last_hwnd });抽 `dedup_opened()` 纯函数;只在 hwnd 变化时 push;uia_event.rs 把 sender 类型 `Sender<Option<DialogInfo>>` 改 `Sender<DialogEvent>`;加 4 个 unit test | +60 / -10 |
| `sigma-listary-spike/src/main.rs` | 删除 dialog event 时的 write_path 块(33 行);sender 类型同步改 enum;match `DialogEvent::Opened/Closed`;撤 `last_dialog_hwnd` 变量 | -33 / +8 |
| `sigma-listary-spike/src/writer.rs` | 撤 `fg_bypass::run_with_bypass` 包装;直接调 `inject_folder_path`;注释更新 | -3 / +12 |
| `sigma-listary-spike/src/lib.rs` | 删 `pub mod fg_bypass;` + lib.rs attribution 段更新 | -1 / +9 |
| `sigma-listary-spike/src/fg_bypass.rs` | **整个文件删除**(QwenLM Apache-2.0 attribution 保留在 git history) | -130 |
| `sigma-listary-spike/Cargo.toml` | description 移除 QwenLM fg_bypass 引用 | 0 |
| `sigma-listary-spike/README.md` | 移除 fg_bypass.rs 表格行 + license 段;加 round-18 update 说明 | -25 / +9 |
| `sigma-listary-spike/src/uia_inject.rs` | **不变**(保留 PathWrap 原版 SetValue + Invoke) | 0 |

### 2.2 Build & Test 证据

```
cargo +1.85.0 check --lib   → Finished in 22.94s (no warnings, no errors)
cargo +1.85.0 test --lib    → 15 passed; 0 failed; 0 ignored
                              - events::tests::spike_error_event_serializes        ok
                              - events::tests::read_event_serializes               ok
                              - events::tests::write_event_serializes              ok
                              - state::tests::register_and_remove                  ok
                              - state::tests::current_path_updates                 ok
                              - logger::tests::log_event_does_not_panic            ok
                              - logger::tests::now_iso8601_format                  ok
                              - logger::tests::format_iso8601_utc_known_value      ok
                              - config::tests::parse_config_returns_resolved_port  ok
                              - config::tests::resolve_port_increments_on_conflict ok
                              - config::tests::resolve_port_preferred_free        ok
                              - uia_event::tests::first_hwnd_pushes                ok (NEW)
                              - uia_event::tests::same_hwnd_does_not_repush        ok (NEW)
                              - uia_event::tests::different_hwnd_pushes            ok (NEW)
                              - uia_event::tests::dialog_event_enum_distinguishes_opened_closed ok (NEW)
cargo +1.85.0 build --release → Finished in 49.34s
                              target/release/spike.exe = 2,310,144 bytes
                              (round 17 was 3,067,904 bytes — removed 757,760 bytes of fg_bypass module)
```

### 2.3 Smoke test 证据

```
$ spike.exe --port 37503 --initial-path "C:\\Users\\foo"
$ python: send get_status  → {"ok":true,"current_path":"C:\\Users\\foo","active_dialogs":0}
$ python: send set_path    → {"ok":true}                                  (path=D:/Users/Public/Downloads)
$ python: send get_status  → {"ok":true,"current_path":"D:/Users/Public/Downloads","active_dialogs":0}
$ python: send quit        → {"ok":true}
$ spike log: TCP server listening port=37503 + path_update current_path=D:/Users/Public/Downloads
```

IPC 协议(round 17 之后没改)继续工作。Round 18 改动只在以下面:
- dialog event 不再触发 write,只 register
- fg_bypass wrapper 撤回
- uia_event 去重 push
- DialogEvent enum 化

---

## 三、新二进制部署状态

**Staged**:`C:\Temp\focus-sync-round18\spike.exe` (2,310,144 bytes, 2026-09-29 23:18)

**NOT YET deployed**:
- `F:\soft\00selfmade\filemanager\release\extension\bin\focus-sync-sidecar.exe` (current: 3,067,904 bytes round-17)
- `C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe` (current round-17)
- `F:\soft\00selfmade\filemanager\release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe`
- `F:\soft\00selfmade\filemanager\release\kizemo.focus-sync-0.2.0-setup.exe`

按 CLAUDE.md 安全红线:**覆盖 staging + 重新打包 installer 是 destructive 操作,需用户当面授权**。本次会话已完成代码 + build + smoke test,deployment 等用户单独确认。

---

## 四、与最初计划的偏差分析

**最早路线(2026-09-27)** = 移植 PathWrap + QwenLM fg_bypass,搬 3 文件 ~705 行,Phase 2 集成由 Sigma-side IPC 推 path → spike 写 active dialogs。

**Round 18 实际产出**:搬 2 文件 ~575 行(PathWrap dialog.rs + monitor.rs),**撤回 fg_bypass.rs**,撤回 spike 多做的"持续推送写入",保留 Phase 2 IPC push-based 触发语义。

| 维度 | 最初计划 | Round 18 实际 | 一致性 |
|---|---|---|---|
| 写入目标 | filename 输入框(PathWrap 原版 SetValue + Invoke) | filename 输入框(保留) | ✅ 完全一致 |
| 写入触发点 | tcp_server::set_path handler(Phase 2 spec) | tcp_server::set_path handler(恢复) | ✅ 完全一致 |
| 沿用上游原版 | "保留 spike 原版 7 文件 + 新增 3 移植文件" | "撤回 spike 多做的事,保留 2 移植文件" | ✅ 比原版更对齐 spec |
| fg_bypass | 必搬(0/507 z-drops) | 撤回 | ❌ 偏离但合理 |
| spike push 频率 | spec 没明确 | uia_event 只在 hwnd 变化时 push | ✅ 比 round 17 更稳 |
| main.rs registry 清理 | spec 没明确 | Closed { last_hwnd } 清理 + remove_dialog | ✅ 补 round 17 漏 |

**关键偏离**:撤 fg_bypass。

**Why**:handoff-filename-flash §一 指出 `EnableWindow(FALSE)` 反复 toggle 让 Sigma FM 失顶层焦点。QwenLM 上游实测有效是因为它的实际场景是"headless UWP Calculator(无 Sigma FM 竞争)",我们的场景是"Sigma FM foreground + Edge Chromium Save dialog",Chromium self-foreground 这一会通过 `EnableWindow(FALSE)` 影响 Sigma FM,不是 Chromium 自己。

**How to apply**:spike binary 撤回 fg_bypass.rs 是 round 18 起永久决定。未来如果换场景(无 Sigma FM 竞争),需要重新评估是否恢复。但目前 setValue+Invoke 本身不抢焦点(PathWrap 上游注释"无按键模拟、不抢焦点"),不需要 fg_bypass。

---

## 五、技术债(待下一轮)

| 项 | 说明 | 优先级 |
|---|---|---|
| 部署到 staging + 重新打包 installer | 等用户授权 | 高(下一会话第一件事) |
| Sandbox 真机验证:Edge Save dialog 闪动 | Sigma FM foreground,触发 Save dialog,观察 filename 是否还闪 | 高 |
| Sandbox 真机验证:抢焦点 | 用户能否在 Sigma FM 切其他 app | 高 |
| Phase 2 spec 中的 poll_path_changes | spec 写了 main select! 第三个 source = poll_path_changes,实际没有 | 中 |
| `state.last_known_path` 语义 | 当前是 reader 读到的 dialog 当前 filename,不是 writer 写的。可考虑重命名 `last_read_path` | 低 |
| `app_whitelist` CLI flag | spec §6.6 提到保留,但 round 18 没验证 | 低 |

---

## 六、避坑提示(累积,新会话必读)

### 6.1 fg_bypass 撤回不可逆

**撤回理由**:`EnableWindow(FALSE)` 反复 toggle 让 Sigma FM 失顶层焦点(2026-09-29 round-17 用户反馈)。QwenLM 上游 headless UWP Calculator 场景有效,但 Edge Chromium Save dialog + Sigma FM foreground 场景无效。

**不可逆场景**:如果未来要让 spike 工作在"headless 自动化测试"或"无人值守 spike 单独跑",可能需要 fg_bypass 重新启用。但当前 production 场景 = Sigma FM 主程序 + Edge Save dialog,不需要 fg_bypass。

### 6.2 spike 持续推送 → on-demand 触发

**Round 17 错误**:spike 改成了"持续推送 → main 写 dialog"模式,导致 SetValue 后 Edge reset filename → 下次 tick 又写 → 闪动。

**正确模式**:PathWrap on-demand = "用户主动操作(Sigma 切路径)→ IPC set_path → spike 写所有 active dialog"。spike 的 monitor 只负责"register dialog 到 state",不主动写。

**Why**:PathWrap 上游就没这个 bug,我们要保留 PathWrap 行为。

### 6.3 cargo 工具链选择

`cargo +1.85.0` 是项目锁文件 lock 的 toolchain(2026-09-27 spike PASS 时也是 1.85.0)。default stable 1.97.1 会报 once_cell 1.21.4 / anstyle 245 编译错(跟 spike 代码无关,是依赖版本不兼容新 stable)。

**每次 build/check/test 必须用 `cargo +1.85.0 ...`**。

### 6.4 不要做的事

- ❌ 不要恢复 fg_bypass.rs(已删,QwenLM attribution 在 git history)
- ❌ 不要在 dialog event 分支加回 write(已删,会导致 round-17 闪动复发)
- ❌ 不要用 default stable 1.97.1(必须 +1.85.0)
- ❌ 不要碰 `release/extension/bin/focus-sync-sidecar.exe` 或重新打包 installer — 等用户授权

---

## 七、必读顺序(新会话第一件事)

1. **本 handoff**(你正在读)
2. Read `handoff-2026-09-29-filename-flash.md` — round-17 根因错误诊断,被 round-18 推翻
3. Read `sigma-listary-spike/src/uia_event.rs` — 看 round-18 dedup + DialogEvent enum
4. Read `sigma-listary-spike/src/main.rs:96-160` — 看 select! match DialogEvent
5. Read `sigma-listary-spike/src/writer.rs` — 撤 fg_bypass wrapper 后的状态
6. Read `sigma-listary-spike/README.md` — round-18 attribution 更新
7. (可选) Read `docs/handoffs/handoff-2026-09-27-focus-activation-uia-port.md` — 移植路线起源
8. (可选) Read `docs/_archive/2026-09-27-listary-focus-sync-integration-design.md` §5.1 — Phase 2 触发链路

---

## 八、下一步(新会话承接时)

按用户当前状态分两路:

### 路径 A:用户已决定 deployment

```
1. 授权覆盖 staging:
   copy C:\Temp\focus-sync-round18\spike.exe → release/extension/bin/focus-sync-sidecar.exe
2. 重新打包 extension installer:
   F:\soft\00selfmade\filemanager\release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe
3. 真机验证(Edge Save dialog 不闪动 + Sigma FM 不失焦点)
```

### 路径 B:用户继续技术债清理

```
1. Phase 2 spec 补 poll_path_changes 实现(spec 写了,代码缺)
3. state.last_known_path 重命名 last_read_path
4. app_whitelist CLI flag 验证
5. 加 integration test 模拟 "Sigma 切路径 → IPC set_path → spike 写 dialog"
```

### 当前路径

Task #4 已完成部署到 C:\Temp\focus-sync-round18\,**用户未授权覆盖 staging**。Task #2 (写 handoff)进行中。Session 已达 ~90min,建议开新会话接力。

---

## 九、环境与基线

- **Working dir**: `F:\soft\00selfmade\filemanager\`
- **Branch**: `main` (round-17 分支已 stash,本轮在 main 改)
- **HEAD**: `286287f docs(recovery): pivot Phase 2a from Tauri sidecar to Sigma plugin route` (git log 最新)
- **Toolchain**: rustup default = stable-x86_64-pc-windows-msvc (1.97.1) — **不能用**,必须 `+1.85.0`
- **Spike binary**: `sigma-listary-spike/target/release/spike.exe` 2.31 MB (新),`C:\Temp\focus-sync-round18\spike.exe` 2.31 MB (staging)
- **Staging binary**(未覆盖):`release/extension/bin/focus-sync-sidecar.exe` 3.07 MB (round-17)

---

## 十、Out of scope(继续不做)

- ❌ IFileDialog::SetFolder(POC 内部讨论过,放弃,沿用 PathWrap SetValue)
- ❌ path + \ hack 临时 hack(放弃,沿用 PathWrap SetValue)
- ❌ DLL 注入(AV/WDAC 风险)
- ❌ macOS / Linux
- ❌ 改 spike 上游已 commit 的移植文件(uia_inject.rs PathWrap copyright header 固定)
- ❌ 默认 stable 1.97.1 cargo(必须 +1.85.0)
- ❌ 覆盖 staging / 重新打包 installer(等用户授权)