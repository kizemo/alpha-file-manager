# focus-21 v0.3.9 Handoff — 12 轮 review 完整总结

**日期**: 2026-10-03
**作者**: Claude (MiniMax-M3)
**状态**: v0.3.8 已 ship 但部分失败(filename 中间态污染),v0.3.9 plan 已写但**未实施**

---

## 0. 一句话总结

**12 轮 focus + 5 轮 review 全部失败的真因** = WinUI 3 wrapped dialog 接受 UIA SetValue 到 inner 1001/1148 Edit,**但显示在 filename 字段 1-6 秒后才关闭**(Invoke Save 实际 work 但用户看到中间态)。**v0.3.9 fix**:用 `automation_id='FileNameControlHost'` 精确检测 WinUI 3 wrapped,有则不写任何东西,只 return Unsupported + 通知。

---

## 1. 12 轮 review 全链路(从 focus-17 到 v0.3.8)

| 轮次 | ship 版本 | 改动 | 结果 |
|---|---|---|---|
| focus-17/18 | v0.2.x / v0.3.0 | SetValue + Auto-Commit (focus-18) | "啥也没干" |
| focus-19 | v0.3.1/2 | 删 SetValue | "啥也没干" |
| focus-20 | v0.3.3 | 恢复 SetValue | **"filename 被改"** |
| focus-21 v0.3.4 | v0.3.4 | Bug A/B/C/D + README | "filename 被改" |
| focus-21 v0.3.5 | v0.3.5 | N1 commit_via_uia 对称性 | "filename 被改" + **PUSH=502 (L1 bug)** |
| focus-21 v0.3.6 | v0.3.6 | L2-1 WinUI 不写 | "filename 仍被改"(因 PUSH=502,sidecar 没收到) |
| focus-21 v0.3.7 | v0.3.7 | **L1 .no_proxy()** 修复 | "dialog 不动"(L1 修了,但 L2-1 删 1001 SetValue 还在) |
| focus-21 v0.3.8 | v0.3.8 | A9 should_commit=true + 1001 SetValue 复活 | "filename 又被改"(6 秒中间态) |
| **focus-21 v0.3.9 (待)** | (未 ship) | **is_winui3_wrapped helper** + 末尾检测 | 期待:**U-V1 filename 不被改** |

---

## 2. 当前环境状态(2026-10-03 11:14 后)

| 项 | 状态 |
|---|---|
| **分支** | `feat/recovery-2026-09-28` |
| **working tree** | 12+ 个 M 文件 + 多个 untracked(未 commit,沿用 focus-18 起政策) |
| **v0.3.8 setup.exe** | `release/Sigma-File-Manager-2.2.2-focus-21-v0.3.8-setup.exe`(17,074,464 bytes,2026-10-3 11:14)|
| **v0.3.7 setup.exe** | `release/Sigma-File-Manager-2.2.2-focus-21-v0.3.7-setup.exe`(17,076,361 bytes,2026-10-3 09:17)|
| **v0.3.6 setup.exe** | `release/Sigma-File-Manager-2.2.2-focus-21-v0.3.6-setup.exe`(17,067,842 bytes,2026-10-3 00:31)|
| **sidecar 运行** | PID 188400(从 2026-10-3 01:25:04 启动,v0.3.8 binary)|
| **L1 状态** | `.no_proxy()` 在 `extension_http.rs:46` — PUSH=200 ✅ |
| **A9 状态** | `should_commit=true` 默认 — v0.3.8 binary 行为 |
| **L2 状态** | `find_ifiledialog_edit` 接受 1001+1148 + Auto-Commit → 写 filename |

---

## 3. v0.3.9 必读文档链(按顺序)

### 3.1 必读第一 — 12 轮 review 真相

**`docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-winui3-detection.md`**
- 完整 sidecar log 时序(hwnd=30019418)
- `FileNameControlHost` 检测原理
- v0.3.9 实施 plan
- 5 个 user-verify 场景

### 3.2 必读第二 — v0.3.8 错误的完整复盘

**`docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-evaluation-v2.md`**
- 5 项新发现 + A9 UX 倒退警告
- v0.3.8 改进叠加分析
- v0.3.8 plan(已废弃,不要按此实施)

### 3.3 必读第三 — 11 轮 review 链

- v0.3.7 systematic-debugging + Mavis 第 8 轮 L1 review
- v0.3.6 systematic-debugging
- v0.3.8 inventory-1001(Claude 第 9 轮)+ Mavis 第 9 轮 review
- v0.3.8 review-v2(Mavis 第 10 轮)
- 5 轮 focus handoffs(`handoff-2026-10-01-*.md` / `handoff-2026-10-02-*.md`)

### 3.4 必读第四 — 实际源码

- `sigma-listary-spike/src/uia_inject.rs`(L347-700,核心)
  - L472-525:当前 v0.3.8 fix-A(1001/1148 SetValue + Auto-Commit)
  - L605-690:helper functions(find_automation_id_edit, find_ifiledialog_edit, path_matches, verify_best_button_is_save)
  - **需要加 is_winui3_wrapped helper + 末尾检测分支**
- `sigma-listary-spike/src/writer.rs` L56(should_commit=true)
- `sigma-file-manager/src-tauri/src/extensions/extension_http.rs` L46(`.no_proxy()`)
- `release/extension/dist/index.js` L356-358(A3 toolbar 文案)

---

## 4. v0.3.9 实施步骤(20 min 代码 + 30 min 测试 + 30-45 min user-verify)

### 4.1 代码改动

1. **加 helper** `is_winui3_wrapped(elements)` 检测 `automation_id='FileNameControlHost'`
2. **修改 `inject_via_uia` 末尾分支**:在 `find_ifiledialog_edit` 之前先检查 WinUI 3 wrapped,有则 return Unsupported
3. **沿用 v0.3.8 改进**:
   - L1 `.no_proxy()`(`extension_http.rs:46`)✅
   - A1 Edit ControlType 限定(`find_automation_id_edit`)
   - A2 read back verify(`path_matches`)
   - A4 1001+1148 helper(`find_ifiledialog_edit`)
   - A6 path tolerance
   - A7 best_button name verify
   - A8 fallback B Invoke 优先
4. **Version bump 0.3.8 → 0.3.9**(3 文件)

### 4.2 user-verify BLOCKING

| # | 任务 | 期望 |
|---|---|---|
| **U-V1** | Edge WinUI 3 Modern Save As(无 flag) | PUSH=200 + **filename 不被改** + 弹 unsupported 通知 |
| **U-V2** | Edge Legacy(启 flag) | dialog 自动跳(COM SetFolder)+ Auto-Commit |
| **U-V3** | Chromium Save As | 41477 SetValue + Invoke + 关闭 |
| **U-V4** | Notepad Save As | 1001 SetValue + Auto-Commit + 关闭 |
| **U-V5** | Photos Save As | 1001 SetValue + Auto-Commit + 关闭 |

**U-V1 是最关键**(用户 12 轮 reject 的痛点)
**U-V4 / U-V5 是 sanity check**(不要误杀 true IFileDialog)

### 4.3 失败 fallback

- **如果 U-V1 fail**:helper 加多 1-2 个 WinUI 3 检测标志(如 `microsoft.windows.commonFileDialog`)
- **如果 U-V4 fail**:回退 v0.3.8(全部 1001 都写)— 用户失去"不污染 WinUI 3"诉求
- **如果 U-V1 + U-V4 都 fail**:**架构问题,需重新评估**。但此概率极低,因 `FileNameControlHost` 是稳定 WinUI 3 标志

---

## 5. 关键洞见(给未来会话的)

### 5.1 用户核心诉求(稳定 12 轮)

> "不要把路径写进 filename 字段"

任何 SetValue 到 filename 字段的方案,都需要先 user-verify 用户是否能接受 1-6 秒中间态。

### 5.2 WinUI 3 vs true IFileDialog 区分

| 标志 | true IFileDialog | WinUI 3 wrapped |
|---|---|---|
| Outer window class | `#32770` | `#32770`(也是)|
| `automation_id='1001'`(Edit) | ✅ | ✅(inner content) |
| `automation_id='1148'`(combo) | ✅(Vista+) | ❌(通常 1001) |
| `automation_id='FileNameControlHost'`(Pane) | ❌ | ✅(WinUI 3 wrapper)|
| COM SetFolder | ✅ work | ❌ fail(AccessibleObjectFromWindow fail)|
| UIA SetValue on 1001 | ✅ | ✅(但显示污染)|
| UIA Invoke on Save button | ✅ | ✅(实际 work 但慢)|

**关键**:`FileNameControlHost` 是稳定 WinUI 3 标志(MS 设计,不依赖 Windows 版本)。

### 5.3 12 轮失败总结(教学价值)

| 错误模式 | 出现次数 | lesson |
|---|---|---|
| 没看 sidecar log 推断行为 | 7 轮 | 截图只显示 PUSH 状态,log 显示 sidecar 实际行为 |
| 用户 reject "filename 中间态" 没被重视 | 6 轮 | "working 路径" 不等于 "用户接受" |
| architecture 错误(focus-19) | 1 轮 | focus-19 误删 SetValue |
| architecture 错误(v0.3.8) | 1 轮 | v0.3.8 误复活 SetValue |
| 上游 bug 误判(focus-21) | 1 轮 | 5 轮 focus 改 focus-sync,实际是 Sigma FM sandbox |
| Phase 4.5 架构质疑延迟 | 5 轮 | 应该 3+ 失败就触发 |

### 5.4 给未来会话的硬规则(扩展 28 条)

**规则 31**(12 轮新增):**任何 SetValue 方案在 ship 前,必须先 user-verify 用户是否能接受 1-6 秒中间态**。**`FileNameControlHost` 检测** 是消除中间态的标准做法。

**规则 32**(12 轮新增):**focus-18 的 "working" 不可信**。focus-18 用户测试时间短没看到中间态。**12 轮 reject 才是真相**。

---

## 6. 文档完整链(13 个文件)

```
v0.3.9 winui3-detection.md (本轮 — 必读第一)
  ├─ v0.3.8 evaluation-v2.md (Mavis 第 10 轮 — 已废弃)
  │    └─ v0.3.8 review-v2.md (Mavis 第 10 轮)
  │         └─ v0.3.8 review.md (Mavis 第 9 轮)
  │              └─ v0.3.8 inventory-1001.md (Claude 第 9 轮)
  │                   └─ v0.3.7 systematic-debugging.md (Mavis 第 8 轮 L1)
  │                        └─ v0.3.6 systematic-debugging.md
  │                             └─ 5 轮 focus handoffs
  │                                  ├─ handoff-2026-10-01-focus-18-...md
  │                                  ├─ handoff-2026-10-02-focus-20-...md
  │                                  └─ handoff-2026-10-02-focus-21-...md
  └─ 本 handoff 文档
       └─ setup.exe 状态
```

---

## 7. 链接

**关键文档**(按必读顺序):
1. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-winui3-detection.md` ⭐
2. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-evaluation-v2.md`
3. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-review-v2.md`
4. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-inventory-1001.md`
5. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.7-systematic-debugging.md`
6. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.6-systematic-debugging.md`
7. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.7-L1-review.md`(Mavis 第 8 轮)
8. `docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev3-review.md`(Mavis 第 7 轮)

**关键源码**:
- `sigma-listary-spike/src/uia_inject.rs`(L472-525 v0.3.8 fix,需加 WinUI 3 检测)
- `sigma-listary-spike/src/writer.rs` L56
- `sigma-file-manager/src-tauri/src/extensions/extension_http.rs` L46
- `release/extension/dist/index.js`

**sidecar log 关键证据**:
- `C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log`
- hwnd=30019418 时序:03:23:35-49(v0.3.8 行为:Write 成功 + 6 秒后 dialog_closed)

**已 ship 版本**:
- v0.3.8(17,074,464 bytes, 2026/10/3 11:14)— 当前用户装的版本
- v0.3.7(17,076,361 bytes, 2026/10/3 09:17)— 可回滚
- v0.3.6(17,067,842 bytes, 2026/10/3 00:31)

---

## 8. 一句话 prompt(给下一个会话)

```
实施 focus-21 v0.3.9:读 docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-winui3-detection.md 全文(必读),加 is_winui3_wrapped helper(检测 automation_id='FileNameControlHost'),在 uia_inject.rs inject_via_uia 末尾分支前插入检测(WinUI 3 wrapped → return Unsupported,不写 1001/1148),version 0.3.8 → 0.3.9,cargo test + cargo build + Tauri build + 复制到 focus-21-v0.3.9-setup.exe,U-V1(Edge WinUI 3 filename 不被改)BLOCKING,完成后不 commit(等用户授权)。
```