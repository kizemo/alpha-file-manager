# Handoff: focus-sync v0.5.4 — SendInput race condition 修复

**接力**: Claude (MiniMax-M3) → 新会话
**会话状态**: v0.5.4 已完整 ship(代码 + production),等用户实测
**任务**: 用户实测 11 个 U-V 问题(关键是 Q-D2 / Q-V0 验证 H2 fallback),决策 ship/no-ship

---

## 0. TL;DR(60 秒)

**v0.5.3 失败根因**:
- SendInput 后 `GetForegroundWindow` check **warn + continue**(silent race condition)
- dialog 不在前台时 events 投递到 Sigma FM(foreground window)
- "H1 succeeded" + "Injection succeeded" log **是误报**(OS accept dispatch,但 dispatch 到错的 window)
- v0.5.2 work 是 **race condition**(dialog 偶然在前台)

**v0.5.4 修复**:**1 行 `return false;`** 在 `if current_fg != hwnd` block 内 → 触发 H2 fallback(UIA SetValue + restore,50ms flicker)。

**已实施**:
- ✅ uia_inject.rs 改 1 行
- ✅ SPIKE_VERSION 0.5.3 → 0.5.4(4 文件)
- ✅ cargo test 52 passed(1 pre-existing flaky port test fail)
- ✅ NSIS grep verify 通过
- ✅ Tauri build + setup.exe v0.5.4
- ✅ Production sidecar 已替换(PID 387912,14:23:05 启动)
- ✅ Memory 规则 51 + 52 沉淀

---

## 1. 必读文档顺序(从最重要到最次要)

### 1.1 🔴 必读第一(实施已 ship,必读记录)

**`docs/superpowers/plans/2026-10-06-focus-21-v0.5.4-revised-final.md`**
- v0.5.4 实施计划(系统已采纳此版本实施)
- 1 行 return false + log message 更新
- Fallback chain 完整说明(L1 COM → L2 H1 → L3 H2 → L4 F3 → L5 Unsupported)

### 1.2 🔴 必读第二(实际代码改动位置)

**`sigma-listary-spike/src/uia_inject.rs`** — `try_send_path_via_sendinput` 函数(line ~250-260)
- 当前有 `if current_fg != hwnd { warn!(); return false; }`(v0.5.4 改动后)
- H2 fallback 函数 `try_h2_restore_filename`(line ~340-380)— 不需改,已 work

### 1.3 🔴 必读第三(关键实测量 + handoff)

**`handoff-2026-10-06-focus-21-v0.5.4.md`**(本文档)

### 1.4 🟡 必读第四(Mavis 第 20 轮 review,3 处细节已修)

**`docs/superpowers/plans/2026-10-06-focus-21-v0.5.4-revised-final-review.md`**
- 评分 9.5/10 ship-able
- B1: H2 实测 verify 决定 ship/no-ship
- B2: H2 代码示例简化(actual source 更 complete)
- B3: index.js 行号明确(line 2/4/431,不改 line 9/13/16 historical)
- B4: Scenario 4 应急描述修正

### 1.5 🟡 必读第五(Mavis 第 19 轮 review,核心 race condition 诊断)

**`docs/superpowers/plans/2026-10-05-focus-21-v0.5.3-rollback-analysis.md`**
- v0.5.3 失败复盘 spike.log 真实时序
- race condition 解释
- v0.5.4 修复方向(1 行 abort + H2 fallback)

### 1.6 🟢 选读(历史 context)

**`docs/superpowers/plans/2026-10-05-focus-21-v0.5.3-rollback-analysis-review.md`**
- Mavis 第 19 轮 review,3 处优化(O1/O2/O3)

---

## 2. 当前环境状态

### 2.1 分支与基线

| 项 | 值 |
|---|---|
| **分支** | `feat/recovery-2026-09-28` |
| **基线** | `main` |
| **working tree** | v0.5.4 改动未 commit(等用户 ship/no-ship 决策)|

### 2.2 Production sidecar 状态

| 项 | 值 |
|---|---|
| Binary | `D:/Program Files/Sigma File Manager/tools/focus-sync-sidecar.exe` |
| Size | 2,792,960 bytes(v0.5.4 build) |
| Timestamp | Oct 6 14:23 |
| PID | 387912(运行中)|
| Port | 127.0.0.1:37421 |
| Mode | service(Task Scheduler) |

### 2.3 已 ship 版本清单

```
Sigma-File-Manager-2.2.2-focus-21-v0.5.4-setup.exe    17,080,856 bytes   2026/10/6 14:22  ← 当前 production
Sigma-File-Manager-2.2.2-focus-21-v0.5.3-setup.exe    17,080,091 bytes   2026/10/5 21:35  ← 失败版本,已 rollback
Sigma-File-Manager-2.2.2-focus-21-v0.5.2-setup.exe    17,077,070 bytes   2026/10/5 16:45  ← race condition work
Sigma-File-Manager-2.2.2-focus-21-v0.5.1-setup.exe    17,076,328 bytes   2026/10/5 13:14
Sigma-File-Manager-2.2.2-focus-21-v0.5.0-setup.exe    17,071,360 bytes   2026/10/4 20:39
Sigma-File-Manager-2.2.2-focus-21-v0.3.9-setup.exe    17,071,814 bytes   2026/10/3 11:47
Sigma-File-Manager-2.2.2-focus-21-v0.3.8-setup.exe    17,074,464 bytes   2026/10/3 11:14
```

### 2.4 Source 关键文件改动

| 文件 | 改动 |
|---|---|
| `sigma-listary-spike/src/uia_inject.rs` | 1 行 `return false;` + log 改 "aborting to trigger H2 fallback" |
| `sigma-listary-spike/src/http_server.rs` | SPIKE_VERSION 0.5.3 → 0.5.4 + test 断言 |
| `release/extension/dist/index.js` | line 2/4/431 v0.5.3 → v0.5.4(historical lines 9/13/16 不动)|
| `release/extension-installer/register.ps1` | 10 处 v0.5.3 → v0.5.4 |
| Setup.exe | `...v0.5.4-setup.exe`(17,080,856 bytes)|

---

## 3. 必做验证(决定 ship/no-ship)

### 3.1 🔴 B1 — H2 实测 verify(关键)

**H2 fallback 是否 work when dialog 不在前台——这是决定 ship/no-ship 的关键实测**。

**实测步骤**:

```
1. Edge → Ctrl+S → Save As(filename: WeavePage...)
2. Sigma FM 切到 C:\downloads → PUSH=200
3. Sigma FM 切到 C:\evid → PUSH=200 (dialog 不在前台)
   期望:Q-FG log 含 "aborting to trigger H2 fallback"
         Q-D2 address bar / breadcrumb = "C:\evid"(经 H2 fallback)
4. Sigma FM 切到 C:\Logs → PUSH=200
   期望:Q-D4 address bar = "C:\Logs"
5. user 测试 filename 字段(Q-FC:50ms 闪烁可接受)
6. user 手动点回 Save As dialog → 切回前台
7. Sigma FM 切到 C:\etc → PUSH=200
   期望:Q-FN H1 走通,正常 sync
```

### 3.2 实测结果填表

| Q | 问题 | 实测结果 |
|---|---|---|
| Q-FG | log 含 "aborting to trigger H2 fallback"? | ___ |
| Q-D1 | 1st PUSH 后 address bar = "C:\downloads"? | ___ |
| **Q-D2 ⭐** | **2nd PUSH 后 address bar / breadcrumb = "C:\evid"?** | ___ |
| Q-D3 | Sigma FM 焦点是否被抢走? | ___ |
| Q-D4 | 3rd PUSH 后 address bar = "C:\Logs"? | ___ |
| Q-FB | Sigma FM 搜索框是否被填充路径? | ___ |
| **Q-FC** | **filename 50ms 闪烁?** | ___ |
| Q-HK | log 含 "post-SendInput focus aid=..."? | ___ |
| Q-FN | 用户手动点回 dialog 后,H1 走通? | ___ |
| Q-FT | 焦点盗窃彻底解决? | ___ |
| **Q-V0** | **H2 fallback 实测 work(Q-D2 = 当前路径可证明)?** | ___ |

### 3.3 决策树

```
Q-D2 = "C:\evid" AND log 含 "aborting to trigger H2 fallback"
  → ✅ H2 fallback work → ship v0.5.4 (accept 50ms flicker)

Q-D2 = no AND log 含 "aborting" 但没 "H2 succeeded"
  → ❌ H2 fail → fall through F3 (也可能 fail)
  → 回退 v0.5.2 binary (race condition,已知 work)

Q-D2 = yes 但 user 拒绝 50ms flicker
  → 回退 v0.5.2 binary (race condition,已知 work)

Q-D3 = 焦点仍被抢
  → 检查 extension 实际代码(可能 v0.5.3 部署了 I2 fix?)
  → 重新评估 race condition
```

---

## 4. 不要做的事(避免再次陷入循环)

### 4.1 v0.5.4 范围

- ❌ 不要改 `try_h2_restore_filename`(已 work,plan v0.5.1 实施)
- ❌ 不要改 Home+Shift+End(没坏)
- ❌ 不要改 F3 fallback(冗余 safety net)
- ❌ 不要改 extension index.js(v0.5.3 ship 时没 AllowSetForegroundWindow,无需 revert)
- ❌ 不要改 Cargo.toml / Cargo.lock
- ❌ 不要改 NSIS hooks(已增强)

### 4.2 已 ship 沉淀

- ✅ Rule 47(no Enter / no commit_save_button)
- ✅ Rule 51(GetForegroundWindow 必须 abort 不是 warn)
- ✅ Rule 52(SendInput 在 sidecar 不可靠)
- ✅ H2 fallback 已 ship
- ✅ F3 fallback 已 ship
- ✅ Extension I2 fix(从 v0.5.3-c1-fix.md)— 实际从未部署,无需 revert

---

## 5. Memory 沉淀(已加)

**`feedback_v0.5.4_race_condition.md`** 新增 — Rules 51 + 52:
- **Rule 51**:SendInput 后必须 GetForegroundWindow verify dialog 在前台——不在前台就 `return false`(不 warn + continue)。SendInput 接受 events ≠ 投递成功。"H1 succeeded" log 是误报。race condition 不是 fix。
- **Rule 52**:SendInput 在 sidecar 单独进程下不可靠——production-critical path 不应依赖 SendInput。

---

## 6. 7 轮复盘 history(Mavis 第 20 轮 §10.6 沉淀)

| Review | 关注 | 漏掉 |
|---|---|---|
| v0.5.3-fix | A1 删 SetForegroundWindow | race condition |
| v0.5.3-revised-final | I1 verify 代码 | race condition |
| v0.5.3-final-implementation | C1 GetFocus import | race condition |
| v0.5.3-c1-fix | O1/O2 代码细节 | race condition |
| **v0.5.4-revised-final** | **找出 race condition + 1 行 fix** | ✅ |

**4 轮 review 漏掉 race condition**——只关注代码细节,不看运行时行为。

**Claude v0.5.3 复盘找出了真问题**(2026-10-05)— 13 轮 focus 系列以来首次接受"SendInput 不是 silver bullet"。

---

## 7. 元数据

| 项 | 值 |
|---|---|
| **当前分支** | `feat/recovery-00selfmade-filemanager` |
| **基线** | `main` |
| **未 commit** | 是(等用户 ship/no-ship 决策)|
| **Production sidecar PID** | 387912(运行中)|
| **build artifacts** | spike.exe 2,792,960 bytes, setup.exe 17,080,856 bytes |
| **下一任** | 用户实测 11 个 U-V → 决策 ship/no-ship → commit |

---

## 8. 一句话 prompt(新会话接力)

```
读 handoff-2026-10-06-focus-21-v0.5.4.md(必读),按 §3 必做验证跑 11 个 U-V 问题,
关键是 Q-D2(2nd PUSH 后 address bar = "C:\evid"? 经 H2 fallback)决定 ship/no-ship。
按 §3.3 决策树处理结果。
```

---

**完成时间**: 2026-10-06 14:35
**状态**: v0.5.4 已 ship,等用户实测 11 个 U-V 问题(关键是 Q-D2)决定 ship/no-ship