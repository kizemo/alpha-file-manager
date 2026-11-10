# focus-21 完整复盘与修复方案

**日期**:2026-10-07
**方法论**:`systematic-debugging (from superpowers)`
**Iron Law**:未完成 Phase 1 根因调查前不提出修复方案
**执行范围**:Phase 4(sandbox 修复,已验证成功)+ 同步失败根因调查
**文档状态**:缺陷 A/B 已用直接观测确认;缺陷 C 已确认现象、未确认成因,方案要求先诊断

---

# 0. 摘要

| # | 结论 | 状态 |
|---|---|---|
| **Phase 4** | sandbox 注释导致 extension 静默不加载 —— **已修复并验证** | ✅ 完成 |
| **假根因** | 「中文路径 mojibake」—— **不存在,是我的测量工具误差** | ⚠️ 已推翻 |
| **缺陷 A** | sidecar 对**已销毁的 HWND** 写入,产生假的 `unsupported_dialog_type` | ✅ 直接观测确认 |
| **缺陷 B** | `JSON.parse(/health)` 失败 ⇒ **`unsupported_dialog_count` 告警通道整体失效** | ✅ 直接观测确认 |
| **缺陷 C** | `GetForegroundWindow()` **恒为 HWND(0x0)** ⇒ 干净的 H1 路径从未执行,只剩会自我还原的 H2 | ⚠️ 现象确认,成因待诊断 |

**用户的核心抱怨「目录不能同步」对应缺陷 C;A 和 B 解释的是「为什么没有任何报错提示」。**

---

# 1. 头号教训:一次完全由测量工具造成的假根因

> 这一节放在最前面,因为它是本次复盘**最有价值**的产出。

## 1.1 我曾经断言的假根因

我曾断言:sidecar 收到的路径是 mojibake(`鍔炲叕鏂囦欢` 应为 `办公文件`),
根因是「UTF-8 字节被当作 GBK 解码」,并据此完成了一整套 Phase 1–3 分析、
定位到「Sigma FM 事件负载损坏」,还设计了两个候选修复方案。

## 1.2 这个结论完全错误

用正确方式解码同一份数据:

```json
{"kind":"path_update","message":"current_path=E:\\办公文件"}
/get_status → {"ok":true,"current_path":"E:\\办公文件","active_dialogs":1}
```

**中文路径完全正确。编码链路从 extension 到 sidecar 没有任何问题。**

## 1.3 错在哪

PowerShell 5.1 的 `Get-Content` / `Invoke-RestMethod` 默认按**系统 ANSI 代码页(936/GBK)**
解码输入。而 `spike.log` 是 UTF-8;sidecar 的 `/health`、`/get_status` 响应
`Content-Type: application/json` **不带 charset**。于是:

```
文件中的真实字节 (UTF-8):  E5 8A 9E  E5 85 AC  E6 96 87  E4 BB B6
被 ANSI 解码后显示为:       鍔  炲  叕   鏂  鏂  困  潫        ← 假象
```

**我看到的是我自己终端的乱码,不是程序的乱码。**

更严重的是:我当时用这份假数据去「验证」一个 GBK 往返检测算法,算法完美工作,
于是我「确认」了根因 —— **我在用自己的错误验证自己**(循环论证)。

## 1.4 代价

| 项 | 代价 |
|---|---|
| 一整套 Phase 1–3 分析 | 作废 |
| 两个候选修复方案 | 作废 |
| 用户的时间 | 多绕一轮完整交互 |

## 1.5 沉淀的规则

1. **判定「乱码」之前,必须先证明自己的读取工具没有引入乱码。**
   所有文本读取强制 UTF-8:`Get-Content -Encoding UTF8` /
   `[System.IO.File]::ReadAllText($p, [Text.Encoding]::UTF8)`;
   HTTP 响应先取 `RawContentStream.ToArray()` 再显式 UTF-8 解码。
   **两种独立方式解码同一份数据,结果一致才下结论。**
2. **`Content-Type` 不带 `charset` 时,任何客户端的默认解码都不可信。**
   这同时是一个真实产品缺陷(见 §5 修复 P0-2)。
3. **自己新写的检测工具不能用来验证自己的结论。**
   (本次 `read-focus-trace.ps1` 的判定逻辑反而是对的:它对所有节点都报
   `non-ascii, genuine`,独立否证了我的假设。错的是我手读日志那一步。)
4. **sidecar 侧排查必须同时读两个日志文件**:`spike.log`(JSONL 事件)和
   `spike.log.YYYY-MM-DD`(tracing 原因)。只看前者永远看不到失败的真实原因。
   已封装为 `scripts/read-sidecar-log.ps1`。

---

# 2. Phase 4 执行复盘(计划符合性审计)

## 2.1 实际操作与结果

| # | 操作 | 结果 | 符合计划? |
|---|---|---|---|
| 0 | 基线复跑扫描器 | `INVALID (1 violations)` line 7 `window.` | ✅ 与 handoff §3 一致 |
| 1 | 压缩头部注释 | `git diff --numstat` = **3 增 / 106 删** | ⚠️ 实际 **1-109 行**,计划写 1-90 |
| 2 | 动态扫描 | `VALID (0 violations / 19 patterns)` exit 0 | ✅ |
| 3 | 部署 `%APPDATA%` | SHA256 一致;**部署副本独立复扫 VALID** | ✅ 超出计划 |
| 4 | Hook 1 `build-with-sidecar.ps1` | 端到端 exit 0;违规 fixture 验证**确实阻断** | ✅ |
| 4 | Hook 2 `manual-install.ps1` | 解析通过,置于拷贝前 | ✅ |
| 4 | Hook 2b `register.ps1` | 改为验证**已部署副本** | ⚠️ 见 §2.3 |
| 4 | Hook 3 `pre-commit` | 用户授权后安装;通过/拦截两分支均实测 | ✅ |
| 4 | Hook 4 | `release\` 下 1 个 index.js,VALID | ✅ |
| 5 | 完全退出并重启 | PID 35832 → 33188,13:51:26 | ✅ |
| 6a | Sources worker 节点 | **出现** | ✅ H0 决定性验证 |
| 6d | `/health` | `last_ping_received_ts: 0` | ❌ **判据本身错** |
| 7 | 导航目录 | `first_push_received: true`,13:56:08 | ✅ 全链路打通 |

## 2.2 执行中的两次自我更正

### (a) 一度误判「存在第二阻断点」

handoff §4 Step 6d 要求 `last_ping_received_ts > 0` 才算根因确认。**该判据混淆了两件事:**

```rust
// state.rs:9 模块文档自己写明了
//! - `last_ping_received_ts: AtomicU64` — last `/set_path` (Unix millis, 0 if never)
// state.rs:99-113 —— 唯一写入点
pub fn set_current_path(&self, p: String) { ... last_ping_received_ts.store(ts, ...) }
```

`/health` 处理函数(`http_server.rs:321-335`)**只读快照,从不调用 `set_current_path`**。

| extension 侧 | HTTP | 会写该字段吗 |
|---|---|---|
| `pingSidecar()` / `checkHealth()` | `GET /health` | **不会** |
| `pushNow()` ← `onPathChange` | `POST /set_path` | 会 |

所以它为 0 只说明**用户还没切换过目录**。

### (b) handoff §5 的「第二阻断点」假设不成立

§5 推测是「`index.js:435` 的 `loadBool`(无 catch)」。实际(`index.js:243-252`)
**有完整 catch**,且 `activate()` 从 L331 到 L421 无任何未捕获的 await。该线索无效。

## 2.3 计划本身的 4 处偏差

| # | 计划写的 | 实际 | 处置 |
|---|---|---|---|
| 1 | 注释是「第 1-90 行」 | 实际 **1-109 行** | 按实际范围替换 |
| 2 | Hook 2 插到 register.ps1 拷贝前 | **`register.ps1` 不拷贝 index.js**(NSIS 拷的,register 在拷贝后被调用) | 改为验证已部署副本,置于写 JSON 之前 |
| 3 | hook 直接调扫描器 | 扫描器 `sandbox.ts` 路径 **CWD 相对**(`scan-sandbox-dynamic.cjs:27`),非仓库根目录运行会以 `exit 2` **误报** | 三个 hook 全改为显式传绝对路径 |
| 4 | Step 6d 用 `last_ping_received_ts` | 它只由 `POST /set_path` 写入 | 改用 `first_push_received` + 导航动作 |

**第 3 条是实施中发现的真实缺陷**,实测:

```
在 %TEMP% 下运行
A) 不传显式路径 → [FATAL] sandbox.ts not found … EXITCODE=2   ← 误报
B) 传显式路径   → RESULT: VALID (0/19)          EXITCODE=0
```

## 2.4 Phase 4 门禁的最终成果

4 个 hook 全部实测过**通过**与**拦截**两个分支(只会放行的 gate 毫无价值):

| Hook | 位置 | 通过 | 拦截 |
|---|---|---|---|
| 1 构建 | `sigma-file-manager/scripts/build-with-sidecar.ps1` | exit 0 | ✅ 阻断 |
| 2 部署 | `release/extension-installer/manual-install.ps1` | — | 拷贝前验证 |
| 2b 注册 | `release/extension-installer/register.ps1` | — | fail-soft(打包安装无仓库时跳过) |
| 3 提交 | `.git/hooks/pre-commit`(已授权) | exit 0 | ✅ BLOCKED |
| 4 体检 | `scripts/scan-sandbox-dynamic.cjs` | VALID | ✅ |

---

# 3. 同步失败的完整证据链

## 3.1 症状

> 无法把 Sigma FM 的目录同步到下载弹窗(Save As 对话框);
> 且**没有**任何「这个对话框不受支持」的提示。

## 3.2 修复前,extension → sidecar 这一段是好的

trace(15 个节点,跨重启保留):

```
#7   N05.path.raw        "E:/办公文件old"        [non-ascii, genuine]
#8   N06.push.normalized "E:\办公文件old"        [non-ascii, genuine]
#9   N07.push.req        bodyLen=43               [genuine]
#10  N08.push.resp       status 200
```

sidecar 侧确认收到:

```
{"kind":"path_update","message":"current_path=E:\\办公文件"}
/get_status → {"current_path":"E:\\办公文件old","active_dialogs":2}
```

⇒ **编码与传输链路正常**。故障全部在 sidecar → 对话框这一段。

## 3.3 缺陷 A —— 对已销毁的 HWND 写入

**一次推送同时遍历两个注册表条目,结果泾渭分明:**

```
14:20:39  WRITE-FAIL  hwnd=1707512  reason=unsupported_dialog_type   ← 14:02 的死窗口
14:20:39  WRITE       hwnd=3870478 -> 'E:\办公文件old' [uia]          ← 活窗口,成功
```

**死窗口的直接观测:**

```
IsWindow(1707512)        = False
IsWindowVisible(1707512) = False
GetClassName(1707512)    = (空)
当前存活 #32770          = (none)
/get_status              → active_dialogs: 2
```

日志里对应:

```
{"kind":"dialog_closed","message":"monitor reported dialog close hwnd=1707512
 (kept in registry for write resilience; cleared commit dedup)"}
```

UIA 往死 HWND 投递事件 → `EVENT_E_ALL_SUBSCRIBERS_FAILED (0x80040201)` → 映射为
`WriteOutcome::Failed("unsupported_dialog_type")`(`writer.rs:84`)→
累加 `unsupported_dialog_count`。

**注意命名误导**:v0.3.7 设计中 `unsupported_dialog_type` 专门表示
「WinUI 3 有意不支持」;现在它被**完全不同的原因**触发。同一错误码承载两种语义,
导致误报 + 污染 focus-19 的告警阈值。

## 3.4 缺陷 B —— 告警通道整体失效

**这是「为什么没有任何提示」的答案。**

```
#5  N04.health.resp  |  status: 200
#6  N04.health.err   |  SyntaxError: Unexpected non-whitespace character after JSON at position 3
```

`JSON.parse(r.body)` **失败**。而 `checkHealth()` 的告警块结构是:

```js
try { parsed = JSON.parse(r.body); } catch (e) { parsed = null; }
if (parsed && typeof parsed.unsupported_dialog_count === 'number') {
    /* 通知逻辑全在这里面 */
}
```

解析失败 ⇒ `parsed = null` ⇒ **整块跳过** ⇒ 通知**永远不会触发**。

实测计数器从 **0 → 1 → 4**,而用户**零提示**。缺陷 B 完全解释这一现象。

> **待确认**:`r.body` 到底是什么。`N12` 节点已加,重启后 15 秒内可拿到响应体指纹。

## 3.5 缺陷 C —— H1 干净路径从未执行 ★

**这是「目录不能同步」的直接原因。**

同一次注入的完整日志:

```
DEBUG WinUI 3 wrapped detected (has_filename_host=true has_1001=true)
      → entering 4-layer fallback chain
WARN  v0.5.5 SendInput: dialog hwnd=HWND(0x1a0df8) is not foreground
      (current=HWND(0x0)); aborting to trigger H2 fallback.
WARN  v0.5.1 H1 failed: SendInput did not reach the dialog (foreground lock?)
INFO  v0.5.1 H2: target_path written and original filename restored (29 bytes)
INFO  v0.5.1 H2 succeeded: target_path written and filename restored (user saves manually)
INFO  Injection succeeded via UI Automation.
```

**关键事实:`current=HWND(0x0)` —— `GetForegroundWindow()` 每次都返回 NULL。**

这**不是**「对话框偶尔不在前台」:用户操作时前台应是 Sigma FM,而不是空。
多次尝试、多个对话框,**无一例外都是 0**。

**后果链(对应 changelog v0.5.1 的设计):**

| 层 | 设计意图 | 实际 |
|---|---|---|
| H1 | `Ctrl+L` + 路径 + Enter,**不碰 filename 字段**,最干净 | **被永久禁用** |
| H2 | 写入 target_path,**再把原文件名还原**(50ms 间隔) | 唯一实际执行的路径 |
| 结果 | 用户看到对话框被导航到新目录 | 写入后又还原 ⇒ **净效果可能为零** |

而 sidecar 却报 `Injection succeeded` —— 这是一个**假成功**:
它报告的是「H2 的两次 SetValue 都执行了」,而不是「对话框真的导航到了目标路径」。

**已知的部署配置(已实测):**

```
Task KizemoFocusSync
  State            : Running
  UserId           : Duanyi
  LogonType        : Interactive     ← 是交互式,不是 session 0
  RunLevel         : Limited         ← ★ 非提权
  WorkingDirectory : (空)
sidecar PID 12228  SessionId 1
```

`LogonType: Interactive` + `session_id: 1` 说明它**确实在交互桌面内**,
因此「跑在 session 0 看不到前台窗口」这个最常见的解释**被排除**。
剩下的候选:`RunLevel: Limited`(非提权)相关的 UIPI 限制、
窗口站/桌面附着问题,或注入时刻恰好处于窗口切换窗口期。

> **按 Iron Law:现象已确认,成因未确认 —— 因此修复方案必须先诊断,不能盲改。**

---

# 4. 修复方案

> 全部涉及 sidecar(`sigma-listary-spike`)。按 handoff 硬约束 #5「不改 sidecar」,
> **全部待用户批准后实施**。遵守「一次只改一处」:每个 P0 独立提交、独立验证。

## P0-1 —— 写入前做 HWND 存活性检查(修缺陷 A)

**位置**:`src/writer.rs::write_path` 入口

```rust
// 任何写入之前先确认对话框还活着
if !IsWindow(hwnd) {
    state.remove_dialog(hwnd);          // 顺手清掉注册表噪声
    return WriteOutcome::Failed("dialog_gone".into());
}
```

要点:

1. **必须用独立错误串 `dialog_gone`**,不得复用 `unsupported_dialog_type`。
   否则又会把「对话框已关」和「WinUI 3 不支持」两种语义混在一起(见 §3.3)。
2. **`dialog_gone` 不得累加 `unsupported_dialog_count`。**
   它不是「不支持的对话框类型」,累加会持续污染 focus-19 的阈值逻辑,
   最终让真正的 WinUI 3 告警被淹没。
3. `IsWindow` 是极轻量用户态调用,性能可忽略。
4. 配套:`handle_dialog_closed` 不再无脑保留条目,或改为「保留但标记 dead,写入前跳过」。

**验收:**关闭对话框后再切换 Sigma FM 目录 → 不得再出现
`unsupported_dialog_type`,`unsupported_dialog_count` 不得增加。

## P0-2 —— HTTP 响应补 charset(修 §1.5 的真实产品缺陷)

**位置**:`src/http_server.rs::respond_json`

```
Content-Type: application/json
  →  Content-Type: application/json; charset=utf-8
```

**为什么是真 bug**:不带 charset 时,任何默认代码页非 UTF-8 的客户端
(中文 Windows 上的 PowerShell 5.1、部分脚本工具)都会把 UTF-8 响应解码成乱码。
**它已经直接导致我误判根因,还会误导后来者。**

**验收:**`read-sidecar-log.ps1` 与任何 GBK 默认环境客户端均能正确显示中文。

## P0-3 —— 先诊断,再修缺陷 C(★ 关键)

**不要直接改 H1。** 分两步:

### 步骤 1:诊断(先做,不改变任何行为)

在 `uia_inject.rs` 注入前记录:

```
GetProcessWindowStation() → 窗口站名(GetUserObjectInformationW)
GetThreadDesktop()        → 桌面名
GetForegroundWindow()     → 原始值(十六进制)
dialog hwnd               → IsWindow / IsWindowVisible / GetAncestor(GA_ROOT)
sidecar 进程完整性级别     → 是否与目标对话框一致
```

以及**在连续多次推送中采样**,确认 `GetForegroundWindow()` 是**恒为 0**
还是**间歇为 0**(间歇则说明是窗口切换竞态,修法完全不同)。

### 步骤 2:按诊断结果二选一

| 诊断结果 | 修复方向 |
|---|---|
| 窗口站/桌面不正确 | 启动时显式 `SetProcessWindowStation(WinSta0)` + `SetThreadDesktop(Default)` |
| 完整性级别不匹配(UIPI) | sidecar 提权运行**或**放弃 H1,改进 H2 的验证逻辑 |
| 间歇为 0(竞态) | 注入前轮询等待前台稳定(条件等待,不用固定 sleep) |

**验收:**H1 在真实对话框上成功执行一次,日志出现
`H1 succeeded` 而非 `H1 failed: SendInput did not reach the dialog`。

## P1 —— 让 H2 不再报假成功

当前 `H2 succeeded` 只表示「两次 SetValue 都执行了」,不表示「路径生效」。
建议:

1. H2 完成后**读回**地址栏/路径字段,与 target 比对;
2. 不一致则返回 `WriteOutcome::Failed("path_not_persisted")`,
   并同样**不**累加 `unsupported_dialog_count`;
3. 这样 `/health` 与用户通知才能反映真实成功率。

## P1-2 —— 厘清 `should_commit` 与 `auto_confirm` 的关系

日志出现 `auto_confirm=false commit=true`,两者关系不清晰。
建议在 `write_path` 入口打一条说明 `commit` 如何计算出来的日志。

## P2 —— 通知路径的防御性加固(extension 侧,不涉及 sidecar)

`checkHealth()` 目前在 JSON 解析失败时**静默跳过整块逻辑**。建议:

- 解析失败时也 trace/上报一次(`N12.health.parse.err` 已加),避免再次静默;
- 考虑对 `r.body` 做 `String(r.body)` 归一,兼容 `Uint8Array` 返回。

---

# 5. 回归防护

| 项 | 动作 |
|---|---|
| 单测:死 HWND | `write_path` 传入已销毁 HWND → `Failed("dialog_gone")`,且 `unsupported_dialog_count` **不**增加 |
| 单测:活对话框 | 行为与现状完全一致 |
| 单测:编码往返 | `办公文件` 判非 mojibake;`鍔炲叕鏂囦欢` 能还原 |
| 不变式:ASCII | `E:\resource` 逐字节不变 |
| 单测:charset | `/health` 响应头含 `charset=utf-8` |
| 端到端 | 开 Save As → Sigma FM 切目录 → 对话框真的落到目标路径 |
| 端到端 | 关掉对话框后再切目录 → **不再**出现 `unsupported_dialog_type`,计数不增 |
| 端到端 | H1 在真实对话框上出现 `H1 succeeded` |
| 沙箱 gate | 每次改 extension 后 `scan-sandbox-dynamic.cjs` 必须 VALID |
| **日志读取纪律** | 所有日志读取强制 UTF-8(见 §1.5 规则 1) |

---

# 6. 本次新增的诊断能力

针对「每次调试都能准确判断问题出现的节点」这一要求:

## 6.1 extension 侧持久化节点级 trace

- 落地:`sigma.storage` 的 `__focus_sync_trace`
  → `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`
- **不需要新增 manifest 权限**(曾考虑 `sigma.fs.writeFile`,但它需要
  `fs.write` 权限 + 目录授权,为一条日志扩大权限面不值)
- 最近 60 条环形覆盖,写入节流 800ms,关键节点强制落盘,全程 try/catch

| 节点 | 含义 |
|---|---|
| `N01.activate.start` | extension 真的被激活 |
| `N02.storage.probe` | `sigma.storage` 能不能用 |
| `N03.health.resp` | 健康响应:状态、字段名、body 类型/长度/指纹 |
| `N04.health.err` | 探活异常 |
| `N05.path.raw` | **Sigma FM 交给我们什么** |
| `N06.push.normalized` | 归一化后 |
| `N07.push.req/skip` | 即将 POST / 被禁用跳过 |
| `N08/N09.push.resp/err` | sidecar 响应 / 传输失败 |
| `N10.ctx.initialPath` | 启动初始路径 |
| `N11.deactivate` | 退出 |
| `N12.health.parse[.err]` | **JSON 解析成败 + 响应体指纹(修缺陷 B 的关键)** |
| `N13.notify.decide` | **通知决策:是否增长/是否在宽限期/节流剩余** |
| `N14.notify.shown/err/suppressed` | **通知实际发出或被抑制** |

### 编码判据存成数字 ★

**如果把原始字符串直接存进日志,正在追查的乱码 bug 本身就会污染证据。**
所以路径节点额外存 **UTF-16 码元数值数组**,不受任何字符集往返影响。
本次该判定独立地否证了我的 mojibake 假设 —— 工具是对的,我手读日志是错的。

## 6.2 sidecar 链路读取器 `scripts/read-sidecar-log.ps1`

- **强制 UTF-8 解码**,共享文件句柄(sidecar 长期占用日志文件)
- 同时呈现 `spike.log`(JSONL 事件)与 `spike.log.YYYY-MM-DD`(tracing 原因)
- 实时拉取 `/health` + `/get_status`
- **枚举当前存活的 `#32770` 并与 `active_dialogs` 并排对比** ——
  两者不一致就说明注册表里是死句柄
- 附「怎么读」图例

## 6.3 已验证

| 检查 | 结果 |
|---|---|
| ESM 语法 `node --check` | exit 0 |
| 沙箱扫描(每次改完必跑) | `VALID (0 violations / 19 patterns)` |
| 部署副本扫描 + 哈希 | 一致 |
| trace 跨重启保留 | ✅ 实测 26 条完整落盘 |
| mojibake 判别算法 | 真实样本 + 2 组对照全部正确 |
| `read-sidecar-log.ps1` | 实测正确解码中文并定位死句柄 |

---

# 7. 约束遵守自查

| 约束 | 状态 | 证据 |
|---|---|---|
| 不 commit git | ✅ | HEAD 仍 `16f51c7`;仅 stage 过 fixture 并已 reset;暂存区空 |
| 不装 pre-commit 除非单独获批 | ✅ | 明确询问后才装 |
| 一次只改一处 | ✅ | 每个 P0 独立;每次改动后单独跑扫描 |
| **不改 sidecar** | ✅ | **本次仍未改 sidecar 任何一行**,方案待批准 |
| Step 5 必须等用户 | ✅ | 四处均用 ask_user 阻塞 |
| 每个 Step 后跑扫描 | ✅ | 全部 exit 0 |

**遗留物**:`.gate-fixture/`(2 个故意违规样本,用于证明 gate/hook 会拦截)。
本环境无可用 `mavis-trash` 且策略禁止永久删除,可手动删除该目录。

---

# 8. 文件清单

| 文件 | 类型 | 说明 |
|---|---|---|
| `docs/extension-changelog.md` | 新建 | v0.3.0 → v0.5.5 变更史 + 沙箱约束(**永不回填 index.js**) |
| `scripts/read-focus-trace.ps1` | 新建 | extension trace 读取 + 确定性编码判定 |
| `scripts/read-sidecar-log.ps1` | 新建 | sidecar 链路读取(强制 UTF-8 + 存活对话框枚举) |
| `release/extension/dist/index.js` | 修改 | +trace 基础设施、19 个节点调用点;头部加 changelog 指针 |
| `sigma-file-manager/scripts/build-with-sidecar.ps1` | 修改 | +Hook 1 构建门禁 |
| `release/extension-installer/manual-install.ps1` | 修改 | +Hook 2 部署门禁 |
| `release/extension-installer/register.ps1` | 修改 | +Hook 2b 注册门禁(fail-soft) |
| `.git/hooks/pre-commit` | 新建 | +Hook 3 提交门禁(已授权) |
| `docs/.../2026-10-07-focus-21-phase4-sandbox-fix-verification.md` | 新建 | Phase 4 实测记录 |
| `%APPDATA%\…\kizemo.focus-sync\dist\index.js` | 部署 | 哈希一致,沙箱 VALID |
| `sigma-listary-spike/**` | **未改动** | 修复方案待批准 |
