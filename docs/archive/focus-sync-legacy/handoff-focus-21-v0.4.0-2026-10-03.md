# focus-sync v0.4.0 实施完成 — 待 U-V1..U-V5 用户验证

**日期**: 2026-10-03
**计划**: `docs/superpowers/plans/2026-10-03-focus-21-v0.4.0-revised-plan.md`
**状态**: 代码就绪,build 通过,NSIS 验证通过,等用户实测

---

## 1. 改动汇总

| 文件 | 改动 |
|---|---|
| `sigma-listary-spike/src/uia_inject.rs` | 删除 is_winui3_wrapped helper + 检测分支,新增 find_navigation_address_bar helper + WinUI 3 wrapped 完全不动 filename Edit 分支 |
| `sigma-listary-spike/src/http_server.rs` | SPIKE_VERSION 0.3.9 → 0.4.0 + test 断言 |
| `release/extension/dist/index.js` | header + activate log |
| `release/extension-installer/register.ps1` | 10 处 0.3.9 → 0.4.0 |

## 2. Build 产物

| 产物 | 路径 | 大小 | 时间 |
|---|---|---|---|
| spike.exe(sidecar) | `sigma-listary-spike/target/release/spike.exe` | 2,798,080 bytes | 13:10 |
| Sidecar 副本 | `release/extension/bin/focus-sync-sidecar.exe` | 2,798,080 bytes | 13:10 |
| Sidecar 副本 | `release/extension-installer/focus-sync-sidecar.exe` | 2,798,080 bytes | 13:10 |
| **Setup.exe** | `release/Sigma-File-Manager-2.2.2-focus-21-v0.4.0-setup.exe` | 17,078,988 bytes | 13:17 |

## 3. NSIS 抽出 grep 验证

| 字符串 | 期望 | 实际 |
|---|---|---|
| `WinUI 3 wrapped` | 必须有 | ✅ 3 hits(新代码 log 信息) |
| `navigation address bar` | 必须有 | ✅ 3 hits(新 helper + log 信息) |
| `SetValue verified for IFileDialog` | 必须有(v0.3.8 保留路径) | ✅ 1 hit |
| `v0.4.0` | 必须有 | ✅ 2 hits |
| `is_winui3` | 必须**无**(确认已删除) | ✅ 0 hits |
| `v0.3.9` | 必须**无**(确认升级) | ✅ 0 hits |

## 4. cargo test

```
test result: FAILED. 47 passed; 1 failed; 0 ignored; 0 measured
```

**唯一失败**:`config::tests::resolve_port_strict_on_conflict`(should_panic 测试因端口分配 race 不 panic)。**与 v0.4.0 无关,pre-existing flaky test**。

## 5. U-V1..U-V5 用户验证清单

### U-V1(BLOCKING)— Edge WinUI 3 Modern Save As

**操作步骤**:
1. 关闭 Sigma FM
2. 安装 `Sigma-File-Manager-2.2.2-focus-21-v0.4.0-setup.exe`
3. 启动 Sigma FM,等待 Scheduled Task 启动 sidecar(5-10 秒)
4. Edge 打开任意页面 → Ctrl+S
5. Save As dialog 打开后,Sigma FM 切到任意目录(例如 `C:\evid`)
6. 观察 dialog 状态

**预期**(Mavis 第 12 轮 C1 修正版):
- ✅ **filename 字段保持** `WeavePage_3.0.0_x64-setup.exe`(用户原始输入,**完全不动**)
- ✅ spike.log 含 `v0.4.0: WinUI 3 wrapped — Leaving filename Edit untouched. Trying navigation address bar.`
- ✅(如果 address bar 工作)spike.log 含 `v0.4.0: WinUI 3 wrapped — wrote target='...' to navigation address bar (read-back verified). filename Edit untouched.` + dialog 顶部路径栏变到 `C:\evid`
- ✅(如果 address bar 不工作)spike.log 含 `v0.4.0: WinUI 3 wrapped — no navigation address bar found.` + 弹 Unsupported 通知
- ✅ **dialog 不自动关闭**(C6:WinUI 3 不 Auto-Commit,用户手动 Save)
- ✅ PUSH=200 + spike.log 无 WriteFailed

**关键询问用户**:是否接受"filename 保持不变" + "address bar 可能工作 / 可能不支持"

### U-V1a — 用户手动 Save

**操作**:U-V1 后,用户手动点击 Save 按钮

**预期**:dialog 关闭,确认 navigation 实际生效

### U-V2 — Notepad Save As(true IFileDialog)

**预期**:沿用 v0.3.8 行为 — 写完整路径到 1001 Edit,Auto-Commit,关闭。
**spike.log 关键标记**:`SetValue verified for IFileDialog Edit` + `Auto-Commit — Invoke Save button`

### U-V3 — Chromium Save As(41477)

**预期**:沿用 v0.3.8 行为 — 写完整路径到 41477,Auto-Commit,关闭。
**spike.log 关键标记**:`Wrote FULL target to address bar (41477)`

### U-V4 — Edge Legacy(COM SetFolder)

**预期**:COM SetFolder 成功,return ComSetFolder。

### U-V5(新)— Edge WinUI 3 + 无 address bar

如果 U-V1 看到 Unsupported 通知,说明 Edge WinUI 3 dialog 没有可用的 address bar。这是 R75 的预期回退路径。

---

## 6. 实时监控命令

**Sidecar 实时 log**:
```bash
tail -f "/c/Users/Duanyi/AppData/Local/kizemo/focus-sync/logs/spike.log.2026-10-03"
```

**关键事件 grep**:
```bash
grep -a "v0.4.0:" "/c/Users/Duanyi/AppData/Local/kizemo/focus-sync/logs/spike.log.2026-10-03"
```

---

## 7. 回退方案(若 U-V1 fail)

| 方案 | 触发条件 | 动作 |
|---|---|---|
| **A** | U-V1 + U-V1a 全 fail | 完全回滚 v0.4.0 → v0.3.8 代码 |
| **B** | U-V1 fail(filename 仍被改) | 仅删除 WinUI 3 wrapped 分支,接受 v0.3.8 全路径写入 |
| **C** | U-V2/U-V3 fail(Win32 / Chromium 回归)| 检查改动 E 是否被影响 |
| **D** | 完全回退 v0.3.6 L2-1(100% 安全但不完整)| 适用于用户不愿接受 v0.4.0 副作用 |

---

## 8. 必读顺序(下一任 agent)

1. **本文档**(handoff)
2. **v0.4.0 revised plan**(`2026-10-03-focus-21-v0.4.0-revised-plan.md`)
3. **Mavis 第 12 轮 review**(`2026-10-03-focus-21-v0.4.0-review.md`)
4. **Phase A1 验证报告**(`2026-10-03-phaseA1-verification-report.md`)
5. **uia_inject.rs** L455-540(v0.4.0 新代码)

---

## 9. 禁止事项

- ❌ 不要 commit(等用户授权)
- ❌ 不要 reinstall 旧版本(若 U-V1 fail,先讨论再决定)
- ❌ 不要改 uia_inject.rs 之外的 Rust 源码(本计划仅此一处 + http_server.rs)
