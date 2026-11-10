# focus-21 Phase 4 复盘 + 同步失败根因分析与修复计划

> ⚠️ **本文档已被取代。**
> 现行版本:**`2026-10-07-focus-21-complete-retrospective-and-fix-plan.md`**
>
> 原因:本文写于发现缺陷 A(死 HWND)之后、缺陷 B(`JSON.parse` 失败导致告警通道整体失效)
> 与缺陷 C(`GetForegroundWindow()` 恒为 `HWND(0x0)`,导致 H1 从未执行)之前,
> 因此只覆盖了三个缺陷中的一个,且把「目录不能同步」归因得不够准确。
> 保留本文仅作为结论演进过程的记录。**请以现行版本为准。**

**日期**:2026-10-07
**方法论**:`systematic-debugging (from superpowers)` —— Iron Law:未完成 Phase 1 根因调查前不提出修复方案
**状态**:根因**已用直接观测确认**;修复方案待用户批准后实施

**前置文档**:
- `2026-10-07-focus-21-handoff-phase4.md`(执行依据)
- `2026-10-07-focus-21-phase4-sandbox-fix-verification.md`(Phase 4 实测记录)
- `docs/extension-changelog.md`(v0.3.0 → v0.5.5 变更史,本次新建)

---

# ⚠️ 第一部分的头号教训:一次完全由测量工具造成的假根因

**必须放在最前面,因为它是本次复盘最有价值的产出。**

## 我曾经「发现」的假根因

我曾断言:sidecar 日志里的路径是 mojibake(`鍔炲叕鏂囦欢` 应为 `办公文件`),
根因是「UTF-8 字节被当作 GBK 解码」,并据此写了一整套 Phase 1–3 分析、
定位到「Sigma FM 事件负载损坏」,还设计了两个修复方案。

## 这个结论是**完全错误**的

用正确的 UTF-8 解码后,同一份日志:

```json
{"kind":"path_update","message":"current_path=E:\\办公文件"}
/get_status → {"ok":true,"current_path":"E:\\办公文件","active_dialogs":1}
```

中文路径**完全正确**。编码链路从 extension 到 sidecar **没有任何问题**。

## 错在哪:我用了错误的工具读数据,然后把测量误差当成了产品缺陷

PowerShell 5.1 的 `Get-Content` 和 `Invoke-RestMethod` 默认按**系统 ANSI 代码页(936/GBK)**
解码输入。而 `spike.log` 是 UTF-8、`/get_status` 响应的 `Content-Type: application/json`
**没有带 `charset=utf-8`**。于是:

```
文件里的真实字节 (UTF-8)  E5 8A 9E  E5 85 AC  E6 96 87  E4 BB B6
被 ANSI 解码后显示为      鍔 炲 叕  鏂 鏂 困 潫      ← 假象
```

**我看到的是我自己终端的乱码,不是程序的乱码。**

更讽刺的是:我当时还用这个假数据去「验证」一个 GBK 往返检测算法,并且因为算法
在假数据上完美工作而「确认」了根因 —— 我验证的是自己的错误。

## 代价与教训

| 项 | 代价 |
|---|---|
| 一整套 Phase 1–3 分析 | 作废 |
| 两个候选修复方案 | 作废(其中「浏览器无 GBK 编码器」的结论本身是对的,但推理前提是错的) |
| 用户的时间 | 多绕了一轮 |

**沉淀的规则(建议加入 MEMORY.md):**

1. **判定「乱码」之前,必须先确认自己的读取工具没有引入乱码。**
   用两种独立方式解码同一份数据,结果一致才下结论。
   `.rs`/`.ts`/`.log` 全部显式 `-Encoding UTF8` 或 `[System.IO.File]::ReadAllText(path, UTF8)`;
   HTTP 响应先取 `RawContentStream.ToArray()` 再显式 UTF-8 解码。
2. **`Content-Type` 不带 `charset` 时,任何客户端的默认解码都不可信。**
   顺带发现:sidecar 的 `/health` 和 `/get_status` 都应该发
   `application/json; charset=utf-8`,否则**任何**非 UTF-8 默认环境的客户端都会看到乱码。
   这是本次副产物发现的真实兼容性缺陷。
3. **自己新写的检测工具不能用来验证自己的结论** —— 那叫循环论证。
   (本次 `read-focus-trace.ps1` 的 mojibake 判定反而是对的:它对 trace 里所有节点都
   报 `non-ascii, genuine`,与最终结论一致。错的是我手读日志的那一步。)

---

# 第二部分:Phase 4 执行复盘

## 1. 实际执行的操作

| # | 操作 | 结果 | 符合计划? |
|---|---|---|---|
| 0 | 基线复跑扫描器 | `INVALID (1 violations)` line 7 `window.` | ✅ 与 handoff §3 一致 |
| 1 | 压缩头部注释 | `git diff --numstat` = **3 增 / 106 删** | ⚠️ 实际是 **1-109 行**,计划写 1-90 |
| 2 | 动态扫描 | `VALID (0 violations / 19 patterns)` exit 0 | ✅ |
| 3 | 部署 `%APPDATA%` | SHA256 一致 `0CEA389F…`;**部署副本独立复扫也 VALID** | ✅ 比计划多做一次 |
| 4 | Hook 1 | 端到端 exit 0;**违规 fixture 验证确实阻断** | ✅ |
| 4 | Hook 2 `manual-install.ps1` | 解析通过 | ✅ |
| 4 | Hook 2b `register.ps1` | 改为验证**已部署副本** | ⚠️ 见 §3 |
| 4 | Hook 3 `pre-commit` | 用户授权后安装;通过/拦截两分支均实测 | ✅ |
| 4 | Hook 4 | `release\` 下 1 个 index.js,VALID | ✅ |
| 5 | 用户完全退出并重启 | PID 35832 → **33188**,13:51:26 | ✅ |
| 6a | Sources worker 节点 | **出现** | ✅ H0 决定性验证通过 |
| 6d | `/health` | `last_ping_received_ts: 0` | ❌ **判据本身错** |
| 7 | 导航目录 | `first_push_received: true`,13:56:08 | ✅ 全链路打通 |

## 2. Phase 4 执行中的自我更正

### 2.1 我一度误判「存在第二阻断点」

handoff §4 Step 6d 要求 `last_ping_received_ts > 0` 才算根因确认。**判据混淆了两件事:**

```rust
// state.rs:9  模块文档自己写明了
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

### 2.2 handoff §5 的「第二阻断点」假设不成立

§5 推测是「`index.js:435` 的 `loadBool`(无 catch)」。实际该函数(`index.js:243-252`)
**有完整 catch**,且 `activate()` 从 L331 到 L421 之间无任何未捕获的 await。线索无效。

## 3. 计划本身的 4 处偏差

| # | 计划写的 | 实际 | 处置 |
|---|---|---|---|
| 1 | 注释是「第 1-90 行」 | 实际 **1-109 行** | 按实际范围替换 |
| 2 | Hook 2 插到 register.ps1 的拷贝前 | **`register.ps1` 不拷贝 index.js**(NSIS 拷的,register 由它在拷贝后调用) | 改为验证已部署副本,置于写 JSON 之前 |
| 3 | hook 直接调扫描器 | 扫描器 `sandbox.ts` 路径是 **CWD 相对**(`scan-sandbox-dynamic.cjs:27`),非仓库根目录运行会 `exit 2` **误报** | 三个 hook 全改为显式传绝对路径 |
| 4 | Step 6d 用 `last_ping_received_ts` 判定 | 它只由 `POST /set_path` 写入 | 改用 `first_push_received` + 导航动作 |

第 3 条是实施中发现的**真实缺陷**,实测:

```
在 %TEMP% 下运行
A) 不传显式路径 → [FATAL] sandbox.ts not found … EXITCODE=2   ← 误报
B) 传显式路径   → RESULT: VALID (0/19)          EXITCODE=0
```

## 4. 约束遵守自查

| 约束 | 状态 | 证据 |
|---|---|---|
| 不 commit git | ✅ | HEAD 仍 `16f51c7`;仅 stage 过 fixture 并已 reset;暂存区空 |
| 不装 pre-commit 除非单独获批 | ✅ | 明确问了才装 |
| 一次只改一处 | ✅ | 每次改动后单独跑扫描 |
| 不改 sidecar | ✅ | sidecar 源码/二进制零改动(本轮同样保持) |
| Step 5 必须等用户 | ✅ | 三处均用 ask_user 阻塞 |
| 每个 Step 后跑扫描 | ✅ | 全部 exit 0 |

**遗留**:`.gate-fixture/`(2 个故意违规样本)。环境无 `mavis-trash` 且策略禁止永久删除,可手动删。

---

# 第三部分:同步失败的真实根因

## 症状

> 无法把 Sigma FM 的目录同步到下载弹窗(Save As 对话框)。

## Phase 1 — 根因调查

### 步骤 1:读错误信息

sidecar 没有报「错误」,它自认为成功了 —— 直到 focus-19 加了 `WriteFailed` 才暴露。
最难查的一类:**静默的错误结果**。

### 步骤 2:稳定复现

已复现。日志里有成功也有失败,可对照。

### ⚠️ 步骤 3 的翻车:先踩了假根因

详见第一部分。**教训:先证明你的尺子没坏,再拿它量东西。**

### 步骤 4:找到真正有区分度的证据

对同一个 hwnd 的一次成功与一次失败做对照(`tracing` 日志,
`%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log.2026-10-07`):

```
✅ 06:03:12  should_commit=false
   DEBUG WinUI 3 wrapped detected (has_filename_host=true has_1001=true)
         → entering 4-layer fallback chain
   WARN  v0.5.5 SendInput: dialog hwnd=HWND(0x1a0df8) is not foreground
         (current=HWND(0x0)); aborting to trigger H2 fallback.
   WARN  v0.5.1 H1 failed: SendInput did not reach the dialog (foreground lock?)
   INFO  v0.5.1 H2: target_path written and original filename restored (29 bytes)
   INFO  Injection succeeded via UI Automation.

❌ 06:16:00  should_commit=true
   WARN  AccessibleObjectFromWindow failed; dialog is not a native IFileDialog
   WARN  COM SetFolder unavailable; falling back to UIA.
   WARN  Injection failed via UI Automation: 事件无法调用任何订户 (0x80040201)
```

**两个决定性差异:**

1. 失败那次**完全没有** `WinUI 3 wrapped detected` 这一行 —— 说明根本没走到对话框结构探测。
2. 错误码 **`0x80040201` = `EVENT_E_ALL_SUBSCRIBERS_FAILED`** ——
   UIA 往一个**已经不存在或不再响应的事件订阅者**上投递事件时的标准错误。

### 步骤 5:把假设钉死(直接观测)

```json
{"kind":"dialog_closed","message":"monitor reported dialog close hwnd=1707512
 (kept in registry for write resilience; cleared commit dedup)","ts":"06:14:24Z"}
```

14:14:24 监控判定对话框关闭,但注册表**故意保留了它**。
14:16:00 的推送仍然对这个 hwnd 发起了写入。

直接验证 HWND 是否还活着:

```
hwnd 1707512  IsWindow        = False      ← 窗口对象已销毁
hwnd 1707512  IsWindowVisible = False
hwnd 1707512  GetClassName    = (空)
当前存活的 #32770 对话框      = (none)     ← 一个都没有
```

而 sidecar 的 `/get_status` 仍然报 **`active_dialogs: 1`**。

## ✅ 根因(已确认)

> **sidecar 把已关闭的对话框留在注册表里,并在没有任何存活性检查的情况下
> 继续向这个已死的 HWND 写入。UIA 对死 HWND 投递事件返回
> `EVENT_E_ALL_SUBSCRIBERS_FAILED (0x80040201)`,被映射成
> `WriteOutcome::Failed("unsupported_dialog_type")`,
> 并累加 `unsupported_dialog_count`。**

**证据链:**

| 环节 | 证据 | 强度 |
|---|---|---|
| 编码链路正常 | trace N05/N06/N07 全部 `genuine`;sidecar `current_path=E:\办公文件` | 直接观测 |
| 对话框已死 | `IsWindow(1707512) = False`,当前存活 #32770 为 0 | 直接观测 |
| 注册表仍持有 | `/get_status` → `active_dialogs: 1` | 直接观测 |
| 写入死 HWND | `06:16:00 WriteFailed ... 0x80040201` | 直接观测 |
| 计数器已累加 | `/health` → `unsupported_dialog_count: 1` | 直接观测 |
| 错误映射 | `writer.rs:84` → `Failed("unsupported_dialog_type")` | 源码核对 |

## 为什么之前的 3 次写入是成功的

14:03 的三次写入(`06:03:02 / 06:03:12 / 06:03:17`)发生在对话框**还活着**的时候,
全部走通 H2 回退(写入路径后还原文件名,用户手动保存)。
也就是说:**对话框活着时同步是好的;对话框死后,每次推送都会撞上这个假失败。**

这解释了用户的主观感受:大部分时候能用,但会莫名其妙地「不生效」并弹
「这个对话框不受支持」。

## 为什么 `unsupported_dialog_type` 这个名字具有误导性

v0.3.7 的设计意图是:**WinUI 3 Save As 有意返回 Unsupported**,以避免污染 filename 字段
(changelog 原文)。但现在这条路径被**完全不同的原因**触发 —— 对死 HWND 写入。

同一个错误码承载了两种完全不同的语义,导致:
- 用户被告知「这个对话框不受支持」,但实际上只是对话框早关了
- `unsupported_dialog_count` 混入噪声,污染 focus-19 的告警阈值逻辑

---

# 第四部分:新增的诊断能力

## 之前的问题

extension 只有 `console.log`,且跑在 **Worker scope**;重启即丢失。
这就是同一问题反复排查 86 篇文档的原因。

## 现在的方案

### 1. 持久化节点级 trace

- 落地:`sigma.storage` 的 `__focus_sync_trace`
  → `%APPDATA%\com.sigma-file-manager.app\user-data\user-extensions.json`
- **不需要新增 manifest 权限**。(曾考虑 `sigma.fs.writeFile`,但它要 `fs.write` 权限 +
  目录授权,为一条日志扩大权限面不值。)
- 最近 60 条环形覆盖,写入节流 800ms,关键节点强制落盘,全程 try/catch。

### 节点表

| 节点 | 含义 |
|---|---|
| `N01.activate.start` | extension 真的被激活 |
| `N02.storage.probe` | `sigma.storage` 能不能用 |
| `N03/N04.health.req/resp/err` | 探活结果 |
| **`N05.path.raw`** | **Sigma FM 交给我们什么** |
| `N06.push.normalized` | 归一化后 |
| `N07.push.req/skip` | 即将 POST / 被禁用跳过 |
| `N08/N09.push.resp/err` | sidecar 响应 / 传输失败 |
| `N10.ctx.initialPath` | 启动初始路径 |
| `N11.deactivate` | 退出 |

### 2. 编码判据存成数字 ★

**如果把原始字符串直接存进日志,正在追查的乱码 bug 本身就会污染证据。**
所以路径节点额外存 **UTF-16 码元数值数组**,不受任何字符集往返影响。

**这次的验证结果(关键)**:

```
#7  N05.path.raw       "E:/办公文件old"  [non-ascii, genuine]
#8  N06.push.normalized "E:\办公文件old"  [non-ascii, genuine]
#9  N07.push.req       bodyLen=43         [genuine]
#10 N08.push.resp      status 200
```

`genuine` 判定**独立地**否证了我的 mojibake 假设。工具是对的,我手读日志是错的。

### 3. 一条命令读回

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\read-focus-trace.ps1 -Tail 12
```

### 4. 已完成的验证

| 检查 | 结果 |
|---|---|
| ESM 语法 `node --check` | exit 0 |
| 沙箱扫描(改完必跑) | `VALID (0 violations / 19 patterns)` |
| 部署副本扫描 | VALID,SHA256 一致 |
| trace 调用点 | 15 处 |
| 真实运行 | 14 条 trace 全部落盘,节点齐全,跨重启保留 ✅ |
| mojibake 判别算法 | 对真实日志样本 + 2 个对照组判定全部正确 |

---

# 第五部分:修复计划(**待批准,尚未实施**)

## P0-1 — sidecar 写入前做 HWND 存活性检查(根治)

**位置**:`sigma-listary-spike/src/writer.rs::write_path` 入口

```rust
// 写入任何对话框之前,先确认它还活着
if !IsWindow(hwnd) {
    state.remove_dialog(hwnd);   // 顺手清掉注册表噪声
    return WriteOutcome::Failed("dialog_gone".into());
}
```

要点:
- 用**独立错误串** `dialog_gone`,不要复用 `unsupported_dialog_type` ——
  否则又会把两种语义混在一起(见第三部分的误导性分析)
- `dialog_gone` **不**累加 `unsupported_dialog_count`(它不是「不支持的对话框类型」,
  累加会污染 focus-19 的告警阈值)
- `IsWindow` 是极轻量的用户态调用,性能可忽略

**配套**:`handle_dialog_closed` 不再无脑保留注册表条目,或改为「保留但标记 dead,
写入前跳过」。

## P0-2 — HTTP 响应补 charset(本次副产物发现的真实缺陷)

**位置**:`sigma-listary-spike/src/http_server.rs::respond_json`

```
Content-Type: application/json  →  Content-Type: application/json; charset=utf-8
```

**为什么这是真 bug**:不带 charset 时,任何默认代码页非 UTF-8 的客户端(中文 Windows 上的
PowerShell 5.1、某些脚本工具)都会把 UTF-8 响应解码成乱码。**这次它直接导致我误判了根因**,
下次它会误导别人。

## P1 — 拆开 `should_commit` 与 `auto_confirm` 的语义

日志显示 06:16:00 是 `auto_confirm=false commit=true`。
这两个参数当前的关系不清晰,建议在 `writer.rs` 入口打一条明确的日志说明
`commit` 是怎么算出来的,否则下次还会在这里迷路。

## 回归防护

| 项 | 动作 |
|---|---|
| 单元测试 | `write_path` 传入已销毁 HWND → 返回 `Failed("dialog_gone")`,且 `unsupported_dialog_count` **不**增加 |
| 单元测试 | 活对话框 → 行为与现在完全一致(H2 回退仍成功) |
| 编码往返测试 | `办公文件` 判非 mojibake;`鍔炲叕鏂囦欢` 能还原 |
| ASCII 不变式 | `E:\resource` 逐字节不变 |
| 端到端 | 打开 Save As → 导航 → 路径正确;**关闭对话框后再导航** → 不应再出现 `unsupported_dialog_type` |
| 沙箱 gate | 每次改 extension 后 `scan-sandbox-dynamic.cjs` 必须 VALID |
| **日志读取纪律** | 所有日志读取必须显式 UTF-8(见第一部分规则 1) |

## 需要用户确认的一个问题

端到端验证需要一个**存活的 Save As 对话框**。目前机器上一个 #32770 都没有。
请在下次验证时:打开一个下载/另存为对话框 → 在 Sigma FM 里切换目录 → 观察是否同步。
关闭后再切一次,确认**不再**出现「这个对话框不受支持」的误报。

## 不做的事

- ❌ 不改 Sigma FM 上游(要重建整个应用)
- ❌ 不加 `fs.write` 权限
- ❌ 不动 N-2 / N-3 / P2-1 / P2-2 休眠项
- ❌ 不自动 commit
- ❌ **不在没批准前改 sidecar**(沿用 handoff 硬约束 #5)

---

## 附:本次新增/修改文件

| 文件 | 说明 |
|---|---|
| `docs/extension-changelog.md` | **新建** —— v0.3.0 → v0.5.5 变更史 + 沙箱约束(永不回填 index.js) |
| `release/extension/dist/index.js` | +trace 基础设施、15 个节点调用点;头部加 changelog 指针 |
| `scripts/read-focus-trace.ps1` | **新建** —— trace 读取 + 确定性编码判定 |
| `docs/superpowers/plans/2026-10-07-focus-21-phase4-sandbox-fix-verification.md` | Phase 4 实测记录 |
| `%APPDATA%\…\kizemo.focus-sync\dist\index.js` | 已部署,哈希一致,沙箱 VALID |
