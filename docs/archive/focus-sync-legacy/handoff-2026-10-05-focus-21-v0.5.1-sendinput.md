# Handoff: focus-sync v0.5.1 SendInput 修复接力

**接力**: Claude (MiniMax-M3) → 新会话(当前会话 ~50 小时,跨 Oct 3 → Oct 5)
**日期**: 2026-10-05 10:16
**会话边界**: 当前会话已完成 13 轮 review(第 8-13 轮)+ 7 个版本 ship(v0.3.5/6/7/8/9 + v0.4.0 + v0.5.0)+ v0.5.1 plan。**会话活跃 ~50 小时且上下文已重**,建议新开会话接力。
**任务**: 实施 v0.5.1 plan + 9 处 Mavis 改进(SendInput Ctrl+L + H2 fallback)

---

## 0. TL;DR(60 秒)

**v0.5.1 是 12 轮 focus-sync 修复以来最精准的方案**:

| 项目 | 状态 |
|---|---|
| 用户核心诉求 | "dialog 真正跳转 + 不要写 filename"(从没变) |
| 当前最近 ship 版本 | v0.5.0(2026/10/4 20:39)— breadcrumb 已 work,filename 仍被改(dual-role bug)|
| v0.5.1 修复方向 | SendInput Ctrl+L 完全绕开 1001 Edit(1001 在 WinUI 3 wrapped 是双角色)|
| Mavis 第 13 轮 review | **9 处改进**(3 HIGH:SendInput helper 缺失 + ASFW_ANY 参数错 + 架构反思)|
| **当前最重要的决策** | **SendInput 应在哪个进程执行?sidecar 单独进程在 Vista+ 实际不可行**|

**下一步**(等用户授权):
1. 选 v0.5.1 实施路径(接受 v0.5.1 plan + Mavis 改进?或改架构让 extension 直接 SendInput?)
2. 实施 v0.5.1 plan(50 min 代码 + 30 min 文档)
3. build + user-verify

---

## 1. 必读文档顺序(从最重要到最次要)

新会话第一件事按顺序读这 8 个文件(每读完一个,API 在哪心里有数):

### 1.1 🔴 必读第一(本轮核心)

**`docs/superpowers/plans/2026-10-05-focus-21-v0.5.1-review.md`(本文档输入)**
- Mavis 第 13 轮 review
- 9 处改进(D1-D9)
- 风险矩阵 R72-R82
- v0.5.1 实施完整 plan

### 1.2 🔴 必读第二(Claude v0.5.1 plan)

**`docs/superpowers/plans/2026-10-05-focus-21-v0.5.1-winui3-breadcrumb-fix.md`**
- v0.5.1 systematic-debugging(本文档输入)
- 核心根因:1001 Edit 双角色(filename 显示 + Chromium 内部 address bar)
- SendInput Ctrl+L 路径
- Hypothesis 1(SendInput)+ H2(恢复 filename)备选

### 1.3 🟡 必读第三(关键证据)

**用户截图**(`E:\Users\Duanyi\.claude\uploads\ef6eb2e9-31ea-4a74-87be-06fd6bdf78d6\*.png`):
- `b291493b-*.png`:filename 被改的截图
- `a58bb8f4-*.png`:console log
- `8ba3bd13-*.png`:用户期望的示例(原始 filename + 新 breadcrumb)

**sidecar log**:`C:\Users\Duanyi\AppData\Local\kizemo\focus-sync\logs\spike.log.2026-10-05`
- hwnd=788472(2026-10-05 01:30 v0.5.0 实际行为证据)

### 1.4 🟡 必读第四(Mavis 之前 review)

**`docs/superpowers/plans/2026-10-03-focus-21-v0.4.0-review.md`(Mavis 第 12 轮)**
- v0.4.0 plan 接受 + 8 处改进(C1-C8)
- **核心论证 C1**:§预期"filename 不变"但代码 SetValue basename `evid` 是污染
- **结论**:v0.4.0 真正 fix 应"完全不动 filename Edit"

**`docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-review.md`(Mavis 第 11 轮)**
- v0.3.9 is_winui3_wrapped 检测的接受 + B1-B3 改进
- 实施时序教训:v0.3.8 ship 时未含全部改动

### 1.5 🟢 必读第五(架构复盘)

**`docs/superpowers/plans/2026-10-03-focus-21-v0.4.0-retrospective-and-fix-plan.md`(Claude v0.4.0 复盘)**
- 12 轮完整时间线
- v0.3.9 是 regression 的论证
- "filename 被改" 是 UX 不是架构 的核心洞察

**`docs/superpowers/plans/2026-10-03-phaseA1-verification-report.md`(Phase A1 验证)**
- v0.3.9 + Win32 dialog 实测 work
- v0.3.8 + Edge WinUI 3 历史日志 work
- v0.3.9 + Edge WinUI 3 fail(v0.3.9 引入的 is_winui3_wrapped)

### 1.6 🟢 选读(前 12 轮 review 历史)

按时间倒序:
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-review-v2.md`(Mavis 第 10 轮)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-review.md`(Mavis 第 9 轮)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.7-L1-review.md`(Mavis 第 8 轮)

---

## 2. 当前环境状态

### 2.1 分支与基线

| 项 | 值 |
|---|---|
| **分支** | `feat/recovery-2026-09-28`(用户当前开发分支) |
| **基线** | `main` |
| **working tree M 文件** | ~13 个(沿用 user authorization policy,**不主动 commit**) |
| **rustc** | `1.90.0-x86_64-pc-windows-msvc` |
| **SPIKE_VERSION 当前** | 0.5.0 → 目标 **0.5.1** |
| **Extension 版本号 当前** | v0.5.0 → 目标 **v0.5.1** |

### 2.2 已 ship 版本清单

```
Sigma-File-Manager-2.2.0-focus-21-v0.3.5-setup.exe    17,075,216 bytes   2026/10/2  23:58
Sigma-File-Manager-2.2.2-focus-21-v0.3.6-setup.exe    17,067,842 bytes   2026/10/3  00:31
Sigma-File-Manager-2.2.2-focus-21-v0.3.7-setup.exe    17,076,361 bytes   2026/10/3  09:17  ← L1 .no_proxy() 修复
Sigma-File-Manager-2.2.2-focus-21-v0.3.8-setup.exe    17,074,464 bytes   2026/10/3  11:14  ← 1001 SetValue + A9 should_commit=true
Sigma-File-Manager-2.2.2-focus-21-v0.3.9-setup.exe    17,071,814 bytes   2026/10/3  11:47  ← is_winui3_wrapped 检测(regression)
Sigma-File-Manager-2.2.2-focus-21-v0.4.0-setup.exe    17,078,988 bytes   2026/10/3  13:17  ← 回滚 + basename 策略(§预期与代码矛盾)
Sigma-File-Manager-2.2.2-focus-21-v0.5.0-setup.exe    17,071,360 bytes   2026/10/4  20:39  ← Sep 27 baseline 改回(dual-role bug)
Sigma-File-Manager-2.2.2-focus-21-v0.5.1-setup.exe    ← 即将 ship
```

### 2.3 13 轮 review 完整链路

| 轮次 | Mavis 文档 | 关键发现 |
|---|---|---|
| 第 8 轮 | `2026-10-03-focus-21-v0.3.7-L1-review.md` | L1 proxy/502 是 BLOCKING 根因 |
| 第 9 轮 | `2026-10-03-focus-21-v0.3.8-review.md` | inventory log 漏看 1001 |
| 第 10 轮 | `2026-10-03-focus-21-v0.3.8-review-v2.md` | A1-A9 改进 + UX 倒退警告 |
| 第 11 轮 | `2026-10-03-focus-21-v0.3.9-review.md` | v0.3.9 接受 + B1-B3 防御 |
| 第 12 轮 | `2026-10-03-focus-21-v0.4.0-review.md` | v0.4.0 §预期矛盾 + C1-C8 |
| 第 13 轮 | `2026-10-05-focus-21-v0.5.1-review.md` | SendInput 路径接受 + D1-D9 |

---

## 3. 13 轮失败链路总结(2 行简表)

```
focus-17 写路径 / focus-18 Auto-Commit / focus-19 删 SetValue / focus-20 恢复 SetValue
focus-21 v0.3.5 L2-1 不写 / v0.3.6 L2-1 + PUSH=502 / v0.3.7 L1 修 / v0.3.8 1001 + Auto-Commit
v0.3.9 is_winui3_wrapped(regression)/ v0.4.0 basename(预期矛盾)/ v0.5.0 Sep 27 baseline(dual-role)
v0.5.1 SendInput Ctrl+L(本次 plan)
```

**用户对 "focus-sync 应该做什么" 的稳定诉求(横跨 13 轮)**:
> "**不要把路径写进 filename 字段**。dialog 真正跳转(导航到目标目录),但 filename 保持原始。"

---

## 4. v0.5.1 plan + Mavis 第 13 轮改进(完整合并)

### 4.1 v0.5.1 核心(Claude)

1. **SendInput Ctrl+L 路径**:
   ```rust
   SetForegroundWindow(dialog)              // 让 dialog 接收键盘
   AllowSetForegroundWindow(ASFW_ANY)      // 获取前台权限
   SendInput Ctrl+L                          // Chromium 聚焦地址栏
   SendInput path                            // 输入完整路径
   SendInput Enter                           // 导航
   ```
2. **H2 备选**:写入 target_path → 立即 SetValue 原始 filename 回去(breadcrumb 保留)

### 4.2 Mavis 第 13 轮 9 处改进(D1-D9)

| # | 改进 | 严重度 |
|---|---|---|
| **D1** | AllowSetForegroundWindow ASFW_ANY = 0xFFFFFFFF(不是 0x00000001) | 🔴 HIGH |
| **D2** | SendInput `make_key` + `make_unicode` helper 完整实现(没提供) | 🔴 HIGH |
| **D3** | **架构反思**:sidecar 单独进程 SendInput 在 Vista+ 实际不可行 | 🔴 HIGH |
| D4 | H2 必须前置 verify Chromium breadcrumb 行为 | 🟡 MEDIUM |
| D5 | 多层 fallback chain(SendInput → H2 → v0.5.0 → Unsupported) | 🟡 MEDIUM |
| D6 | 多次 PUSH 触发多次 SendInput(PUSH dedup)| 🟡 MEDIUM |
| D7 | H2 50ms 闪烁接受 + U-V3 verify | 🟢 LOW |
| D8 | User-verify 任务扩展(U-V1..U-V7) | 🟢 LOW |
| D9 | dialog type detection 区分 SendInput 范围 | 🟢 LOW |

### 4.3 v0.5.1 + Mavis 第 13 轮 总工作量

| 类型 | 工作量 |
|---|---|
| 代码改动(D1-D9) | **50 min** |
| 架构反思文档 | 30 min |
| 集成测试 | 30 min |
| User-verify(BLOCKING)| 30-45 min |
| **总计** | **2 h + 30-45 min user** |

---

## 5. 决策树(用户必选)

```
v0.5.1 修复 SendInput 路径:
├── 接受 v0.5.1 plan(Claude)+ Mavis 改进(D1-D9)？
│   ├── 是 → 实施 v0.5.1 计划 + 改进
│   │         风险:SendInput 在 sidecar 实际不可行(D3)
│   │         fallback chain:D3 H1 → D4 H2 → D5 v0.5.0 → Unsupported
│   │
│   └── 否(架构调整)→ 让 Sigma FM extension 直接 SendInput
│                     需要 manifest 权限 + 架构改动
│                     优点:Sigma FM extension 有 foreground 权限
│                     缺点:复杂架构改动(超出 v0.5.1 范围)
│
└── 直接回退到 v0.5.0(已知 breadcrumb work 但 filename 污染)
              + Edge legacy flag 通知
              + 不再迭代 UIA
```

**用户必须决定**:实施 SendInput(可选 sidecar 或 extension),或回退 v0.5.0。

---

## 6. 给下一任会话的硬规则(13 轮沉淀)

### 6.1 流程规则

- **规则 1-39**: 见各 review 文档末尾(从 13 轮 review 提取)
- **规则 40**(新增):SendInput 必须在有 foreground 权限的进程内执行 — Scheduled Task 启动的 sidecar 在 Vista+ 无此权限
- **规则 41**(新增):任何"X 进程做 Y 操作"的方案,先 verify 进程是否有 Y 操作的权限 — SetForegroundWindow 需要 foreground 历史;SendInput 需要 active foreground

### 6.2 代码实施规则

- **规则 42**(新增):每次 ship 必须 grep 关键字符串验证 setup.exe 包含 — v0.3.8 / v0.4.0 都发生过 ship 时 source 不完整
- **规则 43**(新增):SendInput helper(`make_key` / `make_unicode`)必须完整实现 — 不能用 `/* ... */` 占位符
- **规则 44**(新增):ASFW_ANY 常量必须用 `0xFFFFFFFF`,不是 `0x00000001` — windows crate API 限制

### 6.3 User-verify 规则

- **规则 45**(新增):user-verify 必做项包括"send_path_via_sendinput 实际成功"(不只 UI 看 dialog 跳转)— 需要 verify spike.log
- **规则 46**(新增):H2 fallback 必须前置 verify Chromium breadcrumb 行为 — 不能假设 breadcrumb 在 filename 回写时保持

---

## 7. 必做不做的(避免再次陷入循环)

### 7.1 不要做(13 轮历史教训)

- ❌ **不要继续加 UIA helper**(`is_winui3_v2` / `find_address_bar_v2` / `find_breadcrumb_bar` 等)— SendInput 是新方向,不是 UIA 增强
- ❌ **不要假设 breadcrumb 可以 SetValue**(它是 buttons 不是 Edit)
- ❌ **不要假设 41477 在 WinUI 3 wrapped 中存在**(inventory 已证明不存在)
- ❌ **不要假设 SendInput 在 sidecar 单独进程一定能成功**(Vista+ foreground 限制)
- ❌ **不要做更多 read-back verify 上的 UIA 微调**(UIA 反馈准确,真正问题是 SendInput)

### 7.2 必须做

- ✅ **先读 v0.5.1 plan + Mavis 第 13 轮 review**(按 §1 必读顺序)
- ✅ **每个 helper 完整实现**(不能有 `/* ... */` 占位符)
- ✅ **多层 fallback chain**(SendInput → H2 → v0.5.0 → Unsupported)
- ✅ **PUSH dedup**(避免重复触发)
- ✅ **每次 ship grep verify**(关键字符串必须在 setup.exe 中)
- ✅ **不 commit git**(等用户授权)
- ✅ **User-verify U-V1..U-V7 BLOCKING**

---

## 8. 当前 spike.exe 状态

| 项 | 值 |
|---|---|
| **当前运行版本** | v0.5.0(spike.exe PID 79484 → 现在可能不同,需 ps) |
| **当前 listen 端口** | 127.0.0.1:37421 |
| **生产 sidecar PID** | 需 `tasklist \| findstr spike.exe` 确认 |
| **用户操作环境** | Edge Win11 22H2+ Save As dialog |

**Task Scheduler 配置**:`KizemoFocusSync`(AtLogOn, --service flag)

---

## 9. 文档链接(13 轮完整)

### 9.1 本轮核心(必读)
- `docs/superpowers/plans/2026-10-05-focus-21-v0.5.1-winui3-breadcrumb-fix.md`(Claude v0.5.1)
- `docs/superpowers/plans/2026-10-05-focus-21-v0.5.1-review.md`(Mavis 第 13 轮)
- `docs/superpowers/plans/2026-10-04-focus-21-rollback-to-sep27-plan.md`(v0.5.0 plan)

### 9.2 历史复盘
- `docs/superpowers/plans/2026-10-03-focus-21-v0.4.0-retrospective-and-fix-plan.md`(12 轮复盘)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.4.0-revised-plan.md`(v0.4.0 plan)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.4.0-review.md`(Mavis 第 12 轮)
- `docs/superpowers/plans/2026-10-03-phaseA1-verification-report.md`(Phase A1)
- `docs/superpowers/plans/2026-10-03-focus-21-architecture-reinvestigation.md`(收回误判)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-architecture-retrospective.md`(误判历史)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-review.md`(Mavis 第 11 轮)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-winui3-detection.md`(v0.3.9 plan)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-evaluation-v2.md`(v0.3.8 self-eval)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-review-v2.md`(Mavis 第 10 轮)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-inventory-1001.md`(v0.3.8 plan)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.8-review.md`(Mavis 第 9 轮)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.7-systematic-debugging.md`(v0.3.7 plan)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.7-L1-review.md`(Mavis 第 8 轮)
- `docs/superpowers/plans/2026-10-03-focus-21-v0.3.6-systematic-debugging.md`(v0.3.6 plan)

### 9.3 早期历史(选读)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-rev3-review.md`(Mavis 第 7 轮)
- `docs/superpowers/plans/2026-10-02-focus-20-retrospective-architecture-failure-rev3.md`(rev.3 final)
- 其他 focus-17/18/19 文档

### 9.4 实际源码(关键文件)

| 文件 | 当前状态 |
|---|---|
| `sigma-listary-spike/src/uia_inject.rs` | **v0.5.0**(Sep 27 baseline 改回)— 待 v0.5.1 实施 |
| `sigma-listary-spike/src/writer.rs` | **v0.5.0**(A9 should_commit=true 沿用)|
| `sigma-listary-spike/src/http_server.rs` | SPIKE_VERSION = "0.5.0" |
| `sigma-file-manager/src-tauri/src/extensions/extension_http.rs` | **L1 .no_proxy()(v0.3.7 已实施)** |
| `release/extension/dist/index.js` | **v0.5.0**(沿用)|

---

## 10. 一句话 prompt(新会话接力用)

```
读 handoff-2026-10-05-focus-21-v0.5.1-sendinput.md(必读),
按 §1 必读文档顺序读 v0.5.1 plan + Mavis 第 13 轮 review,
实施 v0.5.1 + Mavis D1-D9 改进(共 50 min 代码 + 30 min 架构反思文档),
build + user-verify U-V1..U-V7 BLOCKING。
```

---

## 11. 风险与提醒

### 11.1 SendInput 实际可行性

**核心风险**:**sidecar 单独进程 SendInput 在 Vista+ 实际不可行**
- spike.exe 是 Scheduled Task 启动的独立进程
- 无 foreground 上下文
- `SetForegroundWindow` 在 Vista+ 受限
- `AllowSetForegroundWindow` 需要接收方调用
- **实际**:SendInput 应在 Sigma FM extension 内执行(有 foreground 权限)

**新会话决策点**:是实施 SendInput in sidecar(尽力),还是改架构让 extension 直接 SendInput?

### 11.2 实施时序

**13 轮 review 教训**:
- v0.3.8 ship 时未含全部改动(实施时序错)
- v0.5.0 ship 后才发现 dual-role bug

**v0.5.1 实施必须**:
1. 完整 source 修改 → cargo test → cargo build
2. **grep verify**(关键字符串必须在 setup.exe 中)
3. 然后才能 ship

### 11.3 13 轮 review 的 meta 教训

**用户对 v0.5.1 决策**(Mavis 第 13 轮 §6.3):
- 第 1 个精准修复是 v0.3.9(WinUI 3 wrapped 检测)— 但 over-conservative
- 第 2 个是 v0.4.0(回滚 + basename)— 但 §预期与代码矛盾
- **第 3 个是 v0.5.1(SendInput Ctrl+L)— 核心根因最精准**

**SendInput 是真正解决问题的方法,不是再"改进"UIA helper**。

---

## 12. 最后的话

**当前会话已经 50 小时,跨 3 天**(Oct 3 → Oct 5),**强烈建议新开会话**。

**handoff 文档已完成**,包含:
- 必读文档顺序(8 个文档,按重要性排序)
- 当前环境状态(分支、版本、文件)
- 13 轮失败链路总结
- v0.5.1 + Mavis 第 13 轮改进合并 plan
- 决策树(用户必选)
- 13 轮硬规则(40-46)
- 不要做的清单(13 轮教训)
- 必做的清单
- 当前 spike.exe 状态
- 文档链接(13 轮完整)
- 一句话 prompt

**新会话只需 4 步**:
1. 读 handoff §1 必读文档(按顺序)
2. 读 v0.5.1 plan + Mavis 第 13 轮 review
3. 实施 v0.5.1 + 改进(50 min)
4. build + user-verify

**不需要重新做 13 轮复盘**。handoff 已经包含所有必要上下文。

---

## 13. 附录:用户核心诉求(从没变)

**横跨 13 轮 focus 改动 + 13 轮 review**:

```
1. ✅ focus-sync 完全不动 dialog(返回 Unsupported + 通知)— 用户接受
2. ❌ focus-sync 写 filename + Auto-Commit(用户看到中间态 → reject)— 12 轮 reject
3. ❌ focus-sync 写 filename + 不 Auto-Commit(dialog 不动 → reject)— 12 轮 reject
4. ✅ focus-sync 写 filename + 立即恢复 + breadcrumb 更新— 用户可能接受(v0.5.1 H2)
5. ✅ focus-sync 用 SendInput Ctrl+L 导航— 完全不污染 filename(用户最期望)
```

**用户最期望(从未变)**:
> "**dialog 真正跳转**(导航到目标目录),但 **filename 保持原始**。**不要把路径写进 filename 字段**。"

**v0.5.1 SendInput Ctrl+L 是 13 轮以来第 1 个真正符合用户诉求的方案**(完全不写 filename,只导航)。

---

## 14. 给下一任 Claude 的话

**13 轮 review 的核心教训**:
- **不要在用户截图上推断,要读 inventory log 真实数据**(Mavis 第 8 轮教训)
- **不要假设边角情况,必须实测验证**(Mavis 第 9-12 轮)
- **§预期必须与代码验证范围一致**(Mavis 第 12 轮 §预期矛盾)
- **不要继续在错的层修问题**(Mavis 第 10 轮 A9 UX 倒退)

**v0.5.1 SendInput 是新方向**,不要:
- ❌ 继续加 UIA helper
- ❌ 假设 SendInput 一定能成功
- ❌ 假设 sidecar 有 foreground 权限

**真正方向**:
- ✅ 完整实现 SendInput helper
- ✅ 多层 fallback chain
- ✅ PUSH dedup
- ✅ grep verify build
- ✅ User-verify U-V1..U-V7 BLOCKING

**如果 SendInput 在 sidecar 实际不可行**:
- 考虑让 Sigma FM extension 直接 SendInput(架构改动)
- 或回退到 v0.5.0(已知部分 work)

**最终目标**:达成用户的核心诉求 — "dialog 真正跳转 + 不要写 filename"。

---

**完成时间**: 2026-10-05 10:16
**会话状态**: 已交接 50 小时持续工作的所有上下文
**下一步**: 用户决定 H1/H2/回退 + 新会话实施
