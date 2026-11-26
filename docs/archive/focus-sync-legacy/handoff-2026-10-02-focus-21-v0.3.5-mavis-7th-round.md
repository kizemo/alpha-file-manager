# focus-21 v0.3.5 Handoff — Mavis 7th-round review fixes

**日期**: 2026-10-02
**作者**: Claude (MiniMax-M3)
**基础**: focus-21 v0.3.4 已 ship + 第 7 轮 review(`docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev3-review.md`)新增 6 项修复
**Setup.exe**: `release/Sigma-File-Manager-2.2.0-focus-21-v0.3.5-setup.exe`(待 build)

---

## 0. TL;DR

v0.3.4 ship 后,Mavis 第 7 轮 review 又发现 6 项遗漏。本文档记录 v0.3.5 的 6 项修复 + **I-COM integration test 作为 BLOCKING user-verify gate**(Mavis N3/N9)。

---

## 1. v0.3.5 改动清单

| # | 来源 | 文件 | 改动 |
|---|---|---|---|
| **N1 [HIGH]** | Mavis §2.3 方案 B | `sigma-listary-spike/src/uia_inject.rs` | WinUI fallback 分支加 `commit_via_uia()` 当 `should_commit=true`,对称 41477 分支 |
| **N2 [MED]** | Mavis §3.3 | `release/extension/dist/index.js` | 加 `BOOT_GRACE_MS=30000` + `inGracePeriod()`,gate 3 处通知(initial ping / pingSidecarWithBody / checkHealth unsupported) |
| **N4 [LOW]** | Mavis §5 | `sigma-listary-spike/src/main.rs` | HTTP thread spawn 加 `ComGuard` RAII,`CoUninitialize` on Drop |
| **N5 [LOW]** | Mavis §6.1 | `release/extension/dist/index.js` | `resetEdgeTip` 立即重显 tip(12s)+ flag 置回 true,不再说"next activation" |
| **N7 [LOW]** | Mavis §6.3 | `release/extension/dist/index.js` | 加 `loadBool(key, default)` helper,统一 3 处 storage 读取(enabled / autoConfirm / edgeLegacyFilePickerTipShown) |
| **N8 [SKIP]** | Mavis §7.1 | (无改动) | P3 #1 累加重构维持不做,风险 > 收益 |
| **N9 [MED]** | Mavis §8 | (本文档) | I-COM integration test 列为 user-verify BLOCKING gate |
| **J1** | 版本 | `http_server.rs` + `index.js` + `register.ps1` | SPIKE_VERSION 0.3.4 → 0.3.5 |

---

## 2. ⚠️ user-verify BLOCKING gate(Mavis N3 + N9)

**为什么是 BLOCKING**:v0.3.4 ship 时没有 user-verify → 用户漏了"Auto-Confirm 时好时坏"的 bug(Mavis N1)。v0.3.5 必修前必须 user-verify,**否则 N1 修复同样可能让用户看到"看起来坏掉了"**。

### 2.1 I-COM-1: Edge WinUI Save As + Auto-Confirm ON

**步骤**:
1. 安装 v0.3.5 setup.exe(覆盖 v0.3.4)
2. 重启 spike.exe(`taskkill /F /IM spike.exe`,Scheduled Task 自动重启)
3. Sigma FM toolbar → "Focus Sync" → "Auto-Confirm" → 确认显示 "ON"
4. Edge 打开任意下载链接 → Save As dialog
5. dialog 弹出 → 不动鼠标 / 不手点 → 验证 dialog 是否**自动 Invoke Save / 关闭**

**期望**:
- ✅ Auto-Confirm ON → Save As dialog 自动关闭(Invoke Save button)
- ❌ dialog 没动 → 进入 I-COM-1-fail

**I-COM-1-fail 路径**:
- 改 uia_inject.rs WinUI fallback `commit_via_uia(None, Some(edit))` → 移除
- 改 41477 分支 `commit_via_uia(...)` → 移除(改为方案 A:两个分支都不 Invoke)
- 重 build v0.3.5.1

### 2.2 I-COM-2: Edge Legacy (IFileDialog) Save As

**步骤**:
1. Edge 地址栏 `edge://flags/#edge-legacy-file-picker` → Enabled → 重启 Edge
2. Edge 下载链接 → Save As dialog(class 应是 `#32770`,不是 WinUI)
4. Auto-Confirm ON
5. dialog 弹出 → Sigma FM 切路径 → dialog 应自动跟随路径并关闭

**期望**:`/health` `version=0.3.5`,`unsupported_dialog_count` 不增,日志可见 `SetFolder via COM succeeded`。

### 2.3 I-COM-3: Chromium Save As

**步骤**:
1. Chrome 打开任意下载链接 → Save As dialog
2. Auto-Confirm ON
3. Sigma FM 切路径 → 验证 dialog 跟随

**期望**:`/health` `unsupported_dialog_count` 不增,日志可见 `Wrote FULL target to address bar (41477)` + `Committed via Invoke(Save button)`。

### 2.4 I-COM-4: sidecar 重启

**步骤**:
1. `taskkill /F /IM spike.exe`
2. Task Scheduler → KizemoFocusSync → Run
3. 日志观察 `HTTP server listening`（应在 1-3 秒内出现）

**期望**:Bug A 修复(CoInitializeEx)生效,无 COM 初始化错误。

---

## 3. 当前环境状态

| 项 | 值 |
|---|---|
| 分支 | `feat/recovery-2026-09-28` |
| working tree M 文件 | ~13 个(沿用 focus-18/19/20/21 政策,不主动 commit) |
| SPIKE_VERSION 当前 | **0.3.5** |
| Extension 版本 | v0.3.5 |
| Setup.exe 路径(将 build) | `release/Sigma-File-Manager-2.2.0-focus-21-v0.3.5-setup.exe` |
| cargo test | 47/48 PASS(1 预存 fail: `config::resolve_port_strict_on_conflict`) |

---

## 4. 文档链接

**输入 review(本文档基础)**:
`docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev3-review.md`(Mavis 第 7 轮)

**沿用文档**:
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`(rev.3 final)
- `handoff-2026-10-02-focus-21-v0.3.4-bug-fixes.md`(v0.3.4)
- `handoff-2026-10-02-focus-20-edge-winui-fix.md`(focus-20 v0.3.3)

**实施 + 测试**:
- `cargo test --release`(沿用 v0.3.4 流程)
- `cargo build --release spike.exe`
- `npm run tauri:build`
- copy to `release/Sigma-File-Manager-2.2.0-focus-21-v0.3.5-setup.exe`

---

## 5. 风险矩阵(v0.3.5 扩展)

| # | 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|---|
| R32-R36 | (Mavis §11) | — | — | — |
| **R37** | N1 修复后 WinUI 不响应 Enter → 改方案 A | 中 | Auto-Confirm 在 Edge 仍不 work | **I-COM-1 user-verify** + 方案 A 回退 |
| **R38** | 30s grace period 过长 → 用户看不到 sidecar down | 低 | 启动后真挂了无 banner | 用户主动点 Sync Now 触发 |
| **R39** | N1 修复让 41477 Invoke 失败(对称性副作用) | 低 | Chromium Auto-Confirm 也坏 | N1-verify-2 / N1-verify-3 BLOCKING |
| **R40** | loadBool 改 storage 语义 → 旧 storage 值失效 | 低 | 旧 user 配置丢 | 严格按 `=== true/false` 判断,等同原 `??` |

---

## 6. 决策清单

### 6.1 必修(N1/N2/N4/N5/N7 + I-COM)

- [ ] N1 commit_via_uia 对称性(已 ship)
- [ ] N2 30s boot grace period(已 ship)
- [ ] N4 RAII ComGuard(已 ship)
- [ ] N5 resetEdgeTip 立即重显(已 ship)
- [ ] N7 loadBool helper(已 ship)
- [ ] I-COM-1 Edge WinUI + Auto-Confirm ON user-verify **BLOCKING**
- [ ] I-COM-2 Edge Legacy Save As user-verify
- [ ] I-COM-3 Chromium Save As user-verify
- [ ] I-COM-4 sidecar restart resilience

### 6.2 不做(N8 推迟 focus-22+)

- [ ] P3 #1 累加重构(风险 > 收益)
- [ ] N6 Bug B flag 30s 延迟置 true(顺手但 user 没显式要)

### 6.3 follow-up 必要路径

- [ ] **如果 I-COM-1 fail → 改方案 A(两个分支都不 Invoke),重 build v0.3.5.1**
- [ ] 如果 I-COM-1 pass → ship v0.3.5 → 进入 focus-22(README + Edge flag 完整支持)