# focus-21 系统化调试 Phase 1-2 报告(遵循 systematic-debugging skill)

**日期**:2026-10-07 09:50
**方法**:superpowers systematic-debugging Phase 1(根因调查)+ Phase 2(模式分析)
**状态**:**Phase 1 未完成,禁止提出任何修复**

---

## 0. Iron Law 遵守声明

> **NO FIXES WITHOUT ROOT CAUSE INVESTIGATION FIRST**
> If you have not completed Phase 1, you cannot propose fixes.

**我此前提出的所有「修复方案」(v0.5.6 Step 2 / Step 3 / 架构重构)全部违反 Iron Law,因此全部作废。**

理由:我从未回答过 Phase 1 的第 2 条 —— **「能否稳定复现?」**

---

## 1. Phase 1:根因调查

### 1.1 Step 1 — 仔细读证据

#### 证据 A:当前 production 健康状态(实测,刚刚)

```json
GET http://127.0.0.1:37421/health
{
  "status": "ok",
  "uptime_secs": 5451,              ← 90 分钟
  "last_ping_received_ts": 0,       ← ★ 从未收到任何 ping
  "last_dialog_event_ts": 0,        ← ★ 从未检测到任何 dialog
  "active_dialogs": 0,
  "uia_healthy": true,
  "version": "0.5.5",
  "mode": "service",
  "unsupported_dialog_count": 0,
  "first_push_received": false      ← ★
}
```

**`last_ping_received_ts: 0` 是决定性证据。**

`state.rs:93-97` 的 `set_current_path` 每次 `/set_path` 都会更新这个时间戳。**它为 0 意味着:sidecar 启动至今 90 分钟内,extension 一次都没有成功调用过 `/set_path`。**

#### 证据 B:进程状态(实测)

```powershell
Get-Process -Name "focus-sync-sidecar"
  Id=12228  StartTime=2026/10/7 8:01:34  Path=D:\Program Files\...\focus-sync-sidecar.exe   ← 活着

Get-Process -Name "*sigma*"
  (无输出)                                                                                  ← ★ Sigma FM 没在运行

Get-NetTCPConnection -LocalPort 37421
  127.0.0.1:37421  Listen  OwningProcess=12228                                            ← 端口正常
```

**Sigma FM 进程不存在。extension 是 Sigma FM 里的 JS 运行时 —— Sigma FM 不运行,extension 100% 不会执行。**

#### 证据 C:extension 已正确安装(实测)

```
%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\
├── dist\index.js      26,851 B   2026/10/6 19:23:30   ← v0.5.5
├── locales\
└── package.json        2,460 B   2026/10/6 19:23:44

user-extensions.json:
  installedExtensions["kizemo.focus-sync"]
    enabled: true
    isLocal: true
    manifest.version: "0.5.5"
    manifest.main: "dist/index.js"
    manifest.extensionType: "api"
```

**extension 装好了,注册了,enabled=true,权限含 `http://127.0.0.1:37421`。**

#### 证据 D:deployed index.js 的 API 调用(实测)

```javascript
111: const SIDECAR_HTTP = 'http://127.0.0.1:37421';
172:   url: `${SIDECAR_HTTP}/set_path`,
321:   const path = await sigma.context.getCurrentPath();
560:   sigma.context.onPathChange((path) => { ... schedulePush(path); });
567:   const initialPath = await sigma.context.getCurrentPath();
568:   if (initialPath) schedulePush(normalizePath(initialPath));
```

**extension 代码路径完全正确:activate → getCurrentPath → schedulePush → POST /set_path。**

#### 证据 E:历史 spike.log(实测)

```
70 次 Write,全部 strategy:"uia"
0 次 COM SetFolder 记录
0 次 "Injection succeeded"(tracing::info!,不进 spike.log)

最后 3 条用户相关事件:
  13:09:39  DialogDetected  hwnd=590520  app=#32770  path="11.politics"
  13:09:39  opened_write_back_skip_sentinel  current=''
  13:14:23  dialog_closed
```

### 1.2 Step 2 — 能否稳定复现?

**⚠️ 当前无法复现,因为 Sigma FM 没有运行。**

这是 Phase 1 的**核心阻塞**:

| 问题 | 状态 |
|---|---|
| 用户观察到的「完全没有同步」 | 2026-10-06 21:27 报告(截图证据) |
| 我现在能否复现? | ❌ 不能 —— Sigma FM 进程不存在 |
| 90 分钟内 sidecar 收到过任何 PUSH? | ❌ `last_ping_received_ts: 0` |

**Iron Law 要求「Reproduce Consistently」。我做不到 —— 缺少一个可复现的环境。**

### 1.3 Step 3 — 检查最近变更

```powershell
git log --oneline -3
16f51c7 fix(focus-sync): v0.5.5 — 41477 exclusion + first_push_received flag
        + /health expose + SPIKE_VERSION sync + repackage full installer
286287f docs(recovery): pivot Phase 2a from Tauri sidecar to Sigma plugin route
```

**v0.5.5 一个 commit 改了 12 处**:
- Fix A(41477 排除)
- Fix B/B2(`first_push_received` gate)
- Fix D(skip sentinel log)
- Fix E(`/health` 暴露)
- Fix F(`default_value = ""`)
- Fix G/SPIKE_VERSION
- Fix H(5 处 runtime log)
- Fix I(package.json)
- 重新打包完整 installer

**`last_ping_received_ts: 0` 的一个可能解释**:Fix F 把 `default_value` 改成 `""`,加上 Fix B 的 gate,使得 `/set_path` 在**收到** PUSH 时会 `set_current_path(path)` → 更新 `last_ping_received_ts`。**但如果 PUSH 根本没来,这个时间戳就永远是 0。**

**所以 `last_ping_received_ts: 0` 无法区分两种情况**:
- (a) extension 根本没跑(因为 Sigma FM 没开)
- (b) extension 跑了但 `sigma.context.onPathChange` 没 fire(用户截图的 TDZ 错误)

**这是日志设计缺陷 —— 和 Rule 66 是同一个问题:目标状态不可观测。**

### 1.4 Step 4 — 数据流追踪

```
Sigma FM 进程
  └─(不存在)→ extension host
                └─ index.js activate()
                     ├─ sigma.i18n.mergeFromPath()
                     ├─ sigma.context.getCurrentPath()  ──(截图显示 TDZ 错误在这里)→ 抛异常
                     └─ [未执行] schedulePush → POST /set_path
                          └─(未到达)→ state.set_current_path()
                                            └─ last_ping_received_ts 应更新,但实际 = 0
```

**TDZ 错误的位置(`navigator-DJVMws3e.js:2:37962`)早于 `activate()` 里的 API 调用。** 如果整个 extension worker 因未捕获异常而崩溃,`getCurrentPath()` 就永远不会执行。

### 1.5 Phase 1 结论

**已确认的事实:**

| # | 事实 | 证据强度 |
|---|---|---|
| F1 | sidecar 活着,端口正常,`uia_healthy: true` | 实测 |
| F2 | **Sigma FM 进程不存在** | 实测 |
| F3 | **90 分钟内 0 次 PUSH**(`last_ping_received_ts: 0`) | 实测 |
| F4 | extension 已安装、注册、enabled、权限正确 | 实测 |
| F5 | extension 代码路径正确(activate → getCurrentPath → POST /set_path) | 实测 |
| F6 | 用户截图显示 Sigma FM 内部 TDZ 错误 | 用户提供 |
| F7 | 历史上 70 次 Write 全部 `strategy:"uia"`(写文本,非真导航) | 实测 |

**未确定的关键问题:**

| # | 问题 | 为什么重要 |
|---|---|---|
| **Q1** | Sigma FM 启动后,extension 的 `activate()` 是否执行到 `getCurrentPath()`? | 决定是 (a) 环境问题 还是 (b) 代码问题 |
| **Q2** | TDZ 错误是否真的阻止了 extension 激活? | 用户截图不能证明因果关系 |
| **Q3** | 那 70 次 `strategy:"uia"` 的 Write,用户当时看到了什么? | 决定「写文本」是否被用户接受过 |

---

## 2. Phase 2:模式分析

### 2.1 找可工作的参照

#### 参照 1:PathWrap 原始实现(176 行)

MIT licensed,来源 `github.com/inaku-Gyan/PathWrap/blob/main/src/os/dialog.rs`。

**我没有读过这个文件的原文** —— 这是 Phase 2 Step 2 明确要求的:

> Compare Against References: If implementing pattern, read reference implementation COMPLETELY

**这是一个必须补的证据缺口。** 我一直在讨论「我们做了什么」,没有读过「原设计是什么」。

#### 参照 2:项目自述的「曾经工作」

`MEMORY.md`:
> v0.3.8 (Oct 3 03:23 hwnd=30019418) **DID write Edge WinUI 3 successfully**

`feedback_phase2_giveup.md:14`:
> v0.3.9 added `is_winui3_wrapped` detection that **deleted** working capability

**但 v0.3.9 的代码没有 git commit**(`MEMORY.md` 明确写了 "no git commit")。

### 2.2 对比工作与失败

| 维度 | v0.3.8(声称成功) | 当前 v0.5.5 | 差异 |
|---|---|---|---|
| 触发条件 | 未知 | `first_push_received` gate | v0.5.5 新增 |
| 元素选择 | 未知 | Fix A 排除 41477 | v0.5.5 新增 |
| 初始 path | `"C:\\"` | `""`(Fix F) | v0.5.5 改 |
| 环境 | Sigma FM 在运行? | **Sigma FM 未运行** | 不可比 |

**关键:两者环境状态不同,无法做有效对比。**

### 2.3 Phase 2 结论

**无法完成 Phase 2,因为:**
1. 参照实现(PathWrap 176 行)未读
2. 唯一声称成功的历史版本(v0.3.8)无 commit、无日志留存、环境不可比

---

## 3. 缺失的证据(必须先补齐)

| # | 缺失 | 怎么补 | 阻塞什么 |
|---|---|---|---|
| **E1** | PathWrap `dialog.rs` 原文 | 读 GitHub 或仓库内 vendored 副本 | 不知道「原设计」是什么 |
| **E2** | Sigma FM 启动后的 extension 激活日志 | 开 Sigma FM → DevTools → 看 `activate()` 是否执行 | 回答 Q1/Q2 |
| **E3** | `sigma.context.getCurrentPath()` 的独立测试 | 在 DevTools console 手动调用 | 区分「API 不可用」vs「TDZ 连锁」 |
| **E4** | 用户看到 70 次 Write 时的实际观感 | 问用户:那些时候地址栏/目录有变化吗? | 回答 Q3 |

---

## 4. Phase 3:单一假设(待 E2 后才能验证)

**我现在的最佳假设(未验证)**:

> **H0:extension 从未激活,原因不是 sidecar 侧的任何改动,而是 Sigma FM 的 TDZ 错误在 extension host 启动阶段就中断了 JS 运行时。**
>
> 依据:
> - `last_ping_received_ts: 0`(90 分钟零 PUSH)
> - 用户截图的 TDZ 错误发生在 `navigator-DJVMws3e.js`(Sigma FM 自身模块),早于 extension 的 `activate()`
> - 历史上「extension 正常工作」的时间点(v0.3.8, Oct 3)与现在不同

**证伪条件**:如果 Sigma FM 启动后 extension 正常激活并发出 PUSH,`last_ping_received_ts` 会 > 0,H0 被推翻。

**证实后的含义**:sidecar 侧的所有改动(12 个 Fix)都是在修一个**不存在的问题** —— gate 只是让症状更明显。

---

## 5. 下一步(Phase 1 未完成,不做修复)

### 5.1 唯一的下一步动作

**请用户执行**:

```
1. 启动 Sigma FM
2. 打开 DevTools(如果 extension 有 devtools 入口)
3. 在 Console 看是否出现:
   - "[focus-sync] v0.5.5 activate START ..."   ← extension 激活了
   - "ReferenceError: Cannot access 'U' ..."    ← TDZ 仍在
4. 手动在 Console 执行:
   await sigma.context.getCurrentPath()
   → 看是返回路径还是抛异常
5. 观察 sidecar:
   curl 127.0.0.1:37421/health
   → last_ping_received_ts 是否从 0 变成非 0
```

**这一步的产出决定后续一切。**

### 5.2 我不会做的事

- ❌ 提任何修复方案(Iron Law)
- ❌ 实施 v0.5.6(它假设了「gate 是问题」,而 H0 未被证实或推翻)
- ❌ 改架构(同上)
- ❌ 声称任何百分比或结论

---

## 6. 对此前 20+ 轮的直接反省

### 6.1 违反 Iron Law 的次数

| 轮次 | 违反内容 |
|---|---|
| v0.5.1~v0.5.5 | 每轮都提修复方案,从未完整跑完 Phase 1 |
| R22-R26 我的 5 轮 review | 全是静态代码审查,**没有一次实测运行时行为** |
| v0.5.6 Step 2 | 提了「回退到 v0.5.0」,而 v0.5.0 本身是被否决的设计 |
| 架构重构提案 | 基于一个我编造的「45% 部署 bug」数据 |

### 6.2 系统性问题

| # | 问题 | 后果 |
|---|---|---|
| 1 | 从未验证「extension 到底有没有在跑」 | 所有 sidecar 侧修复都在打空气 |
| 2 | 从未读过 PathWrap 原实现 | 不知道「正确设计」是什么 |
| 3 | 用静态 review 代替实测 | R25 才第一次 probe binary |
| 4 | 编造数据支撑结论 | 「45%」 |
| 5 | 混淆「写入」与「生效」 | 70 次 Write 被当作「同步成功」 |

### 6.3 沉淀规则

```markdown
### 规则 67 — 修复前必须验证「上游依赖是否真的活着」

**触发**:2026-10-07 systematic-debugging Phase 1

**事实**:sidecar 改了 12 处、上百轮,但 `last_ping_received_ts: 0`
说明 extension 90 分钟内一次都没调用过 sidecar。
所有 sidecar 侧修复都在修一个可能不存在的问题。

**规则**:
1. 修 A→B 之前,先证明 A 真的在动
   (本例:extension 是否在 PUSH?)
2. 如果上游没动,下游的任何修改都只是改变了「症状的表现形式」
3. 「症状消失」不等于「问题解决」——
   v0.5.5 的 gate 让症状从「污染」变成「无动作」,但没解决任何根因
```

```markdown
### 规则 68 — 声称「曾经工作过」必须有可复核的证据

**触发**:MEMORY.md 记录 v0.3.8 "DID write successfully",但 no git commit

**规则**:
1. 引用「某版本曾经工作」时,必须附带:commit hash / 日志片段 / 截图
2. 无 commit 的版本,其日志随日志轮转丢失,不可作为参照
3. 声称「回归」前,必须能 diff 出具体哪一行导致了回归
```

---

## 7. 元数据

| 项 | 值 |
|---|---|
| **方法** | superpowers systematic-debugging Phase 1-2 |
| **Iron Law 状态** | ❌ Phase 1 **未完成** —— 无法稳定复现(Sigma FM 未运行) |
| **已确认事实** | F1~F7(见 §1.5) |
| **关键未决** | Q1(extension 是否激活)/ Q2(TDZ 是否阻断)/ Q3(用户是否接受过写文本) |
| **最佳假设** | H0:extension 从未激活,sidecar 侧修复可能在打空气 |
| **缺失证据** | E1(PathWrap 原文)/ E2(激活日志)/ E3(API 独立测试)/ E4(用户历史观感) |
| **此前方案** | **全部作废** —— 未完成 Phase 1 |

---

## 8. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-systematic-debugging-phase1.md(必读)。

当前状态:systematic-debugging Phase 1 未完成,禁止提修复。

决定性证据(实测):
  GET /health → last_ping_received_ts: 0, last_dialog_event_ts: 0, first_push_received: false
  Get-Process "*sigma*" → 无输出(Sigma FM 没运行)
  sidecar 活着(PID 12228,端口 37421 Listen,uia_healthy: true)
  extension 已装、enabled、权限含 http://127.0.0.1:37421
  deployed index.js 路径正确(activate → getCurrentPath → POST /set_path)

最佳假设 H0:extension 从未激活,sidecar 侧 12 个 Fix 可能在修一个不存在的问题。
证伪条件:启动 Sigma FM 后 last_ping_received_ts 是否从 0 变为非 0。

唯一下一步(请用户执行,不写代码):
  1. 启动 Sigma FM
  2. DevTools Console 看是否出现 "[focus-sync] v0.5.5 activate START"
  3. 手动 await sigma.context.getCurrentPath() 看返回还是抛异常
  4. curl 127.0.0.1:37421/health 看 last_ping_received_ts 是否变化

缺失证据(需补):
  E1: PathWrap dialog.rs 176 行原文(至今没读过)
  E2: Sigma FM 启动后的 extension 激活日志
  E3: getCurrentPath 独立测试(区分 API 不可用 vs TDZ 连锁)
  E4: 用户在 70 次 Write 时的实际观感

沉淀 Rule 67(修前验证上游活着)+ Rule 68(声称曾工作需可复核证据)。
此前 v0.5.6 / 架构重构方案全部作废。
```
