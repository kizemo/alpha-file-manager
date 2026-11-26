# 对 Claude v0.5.6 整合方案的评估与优化

**日期**:2026-10-07
**评估对象**:`2026-10-07-focus-21-v0.5.6-final-integration-plan.md`
**评估人**:Mavis(MiniMax-M3)
**结论摘要**:

> **Claude 的方案在两处是错的,且核心改动(1/2/3)有实测证据表明会重新引入一个已记录的回归。**
> 同时,**我对「缺陷 C」的定性也是错的** —— 新证据显示 H1 并非结构性失效。
> 双方都指错了方向。真相在下面 §3,新证据把它坐实了。

---

## 0. 一句话结论

| 项 | 判断 |
|---|---|
| Claude Rule 58(`SetValue ≠ 导航`) | ✅ **最有价值的一条**,比我原方案深一层 |
| Claude P0-1(死 HWND + `dialog_gone`) | ✅ 同意,与我一致 |
| Claude P0-2(charset) | ✅ 同意,与我一致 |
| Claude Rule 55 / 56 / 61 / 64 / 65 | ✅ 同意,均为正确原则 |
| **Claude「Fix B + Fix F 共谋」是 v0.5.5 退步根因** | ❌ **被实测证据证伪** |
| **Claude 改动 1/2/3(回退 gate + 退回 sentinel)** | ❌ **会重新引入已记录的回归** |
| **Claude「v0.5.6 ship 后 sync 可用」** | ❌ **不成立 —— 核心导航机制未被触碰** |
| Claude 改动 7(env var 门控诊断) | ⚠️ 设计缺陷:在出问题的环境里诊断根本不会运行 |
| **我原方案的「缺陷 C:H1 永久禁用」** | ❌ **我自己也错了**,H1 历史上成功 48 次 |

---

## 1. 我完全同意的部分

先说清楚的同意,这些不需要争辩。

### 1.1 Rule 58 —— 本次评估中最有价值的一条

> `ValuePattern::SetValue` 到 1001(filename Edit)= 把字符串塞进 filename 框,
> **不改变 dialog 当前目录**。只有 `IFileDialog::SetFolder`(COM)才能真正导航。

我逐字核对了源码,**Claude 是对的**:

```rust
// uia_inject.rs:363  try_h2_restore_filename
let value: IUIAutomationValuePattern = ... edit.GetCurrentPatternAs(UIA_ValuePatternId)
value.SetValue(&BSTR::from(target_path))   // ← 写进 filename 控件
```

H2 确实是往 filename 控件里写路径,然后靠 **Chromium 自己解析并提交 breadcrumb**。
源码注释也承认这是有风险的行为(`Mavis D4 [MEDIUM]`)。

**这比我原方案深一层**:我停在「H2 会自我还原,净效果可能为零」,
Claude 指出的是更本质的问题 —— **就算不还原,H2 也从来不是导航**。
我原方案 §P1 提的「H2 read-back 验证」方向对,但表述得不够狠。

### 1.2 P0-1 / P0-2 与我完全一致

- `IsWindow` 检查 + **独立错误码 `dialog_gone`**
- `Content-Type: application/json; charset=utf-8`

补充一条我核对过的实现细节:Claude 的代码调用 `state.remove_dialog(hwnd_u32)` ——
我确认该方法**已存在**(`state.rs:153`),会编译通过。

### 1.3 Rule 55 / 56 / 61 / 64 / 65

- **Rule 55**(gate 之后还剩什么可用路径)—— 非常好的设计约束
- **Rule 56**(改动生效 ≠ 功能可用)—— 正是本项目 100+ 轮的病根
- **Rule 61**(「环境归因」是逃避)—— 直指 focus-19/20 的思维定式
- **Rule 64**(测量工具不能验证自己的结论)—— 准确概括了我的教训
- **Rule 65**(charset)—— 已是产品事实

### 1.4 Rule 63(不要把诊断严格度当 ship 阻塞)

部分同意,但要加限定条件,见 §4.4。

---

## 2. 我反对的部分(附实测证据)

### 2.1 ❌ 核心根因主张被证伪

Claude §1.1 的核心论断:

> **Fix B + Fix F 共谋**:`current = ""` + `first_push_received=false`
> → `should_opened_write_back` 永远 false → `opened_write_back` 永远 skip → 用户零 sync

**实测证据与之矛盾:**

| Claude 的预言 | 实测结果 |
|---|---|
| `current = ""` | `/get_status` → `"current_path":"E:\\办公文件old"` —— **非空** |
| `first_push_received=false` | `/health` → `"first_push_received":true` —— **已置位** |
| `opened_write_back` 永远 skip | spike.log **实际有** `{"kind":"opened_write_back","message":"dialog path='11.politics' current='E:\\resource'; writing current"}` |
| 用户零 sync | 多次 `Write strategy="uia"` 事件,累计 **H1 成功 48 次** |

**gate 不是「永远 false」,它按设计工作**:extension 首次推送前阻止把 sentinel 写进
新对话框(这是 `state.rs:57-63` 明确记录的、06:41:12 实际发生过的竞态),
首次推送后正常放行。**Claude 把一个正确的安全门当成了 bug。**

### 2.2 ❌ 改动 1/2/3 会重新引入已记录的回归

若按 Claude 方案把 gate 简化为 `!initial_path.is_empty()`,同时把
`default_value` 退回 `"C:\\"`,那么:

> extension 尚未推送时,`current_path = "C:\\"`,任何新打开的对话框都会被写入 `C:\`

这正是 Fix B 当初要修的 bug,`state.rs:57-63` 原文:

> *"Until this is true, opened_write_back MUST skip — otherwise it'd write the
> init sentinel `"C:\\"` to every new dialog (**race condition observed in
> spike.log 06:41:12 etc.**)"*

**Claude 自己的 Rule 57(审查 Fix 之间的交互效应)恰好适用于它自己的改动 1+2 组合,
但它没有对自己应用这条规则。**

附带:`initial_path != current` 这个去重条件被删掉后,
`main.rs:349-351` 注释里记录的 round-17 回归
(「spike wrote on every 8ms tick → Edge Save dialog reset filename → flash loop」)
的防护也一并消失。

### 2.3 ❌ v0.5.6 不会修复用户的核心抱怨

用户抱怨的是「**目录不能同步到下载弹窗**」。逐项核对 Claude 的 8 项改动:

| 改动 | 影响用户抱怨吗? |
|---|---|
| 1/2/3 gate + sentinel | ❌ gate 本来就在正常放行 |
| 4 死 HWND | ❌ 只消除**误报**,不改变导航能力 |
| 5 charset | ❌ 只影响**显示** |
| 6 H2 read-back(**可选**) | ⚠️ 只是把「假成功」变成「诚实的失败」,仍不导航 |
| 7 缺陷 C 诊断 | ❌ 只观测,不修复 |
| 8 版本号 | ❌ |

**没有任何一项让对话框真正导航。** 按 Claude 自己的 Rule 58(SetValue ≠ 导航),
v0.5.6 ship 后用户会得到一个「更诚实、更干净、但依然不同步」的版本。

### 2.4 ⚠️ 改动 7 的 env var 门控是设计缺陷

```rust
if std::env::var("FOCUS_SYNC_DEBUG_WINDOW_STATION").is_ok() { ... }
```

sidecar 由 Scheduled Task `KizemoFocusSync` 以 `Interactive` 方式启动,
**任务定义里不会设置这个环境变量** ⇒ 诊断在真正出问题的生产环境里**永远不运行**。
一个只在手动设了环境变量时才生效的诊断,拿不到生产证据。

修正:诊断日志应当**默认开启**(它只在注入路径上,开销可忽略),
需要时可由环境变量**关闭**而不是开启。

---

## 3. 新证据:H1 的真实行为模式(同时推翻我自己的缺陷 C)

我在评估 Claude 方案时,统计了**全部历史 sidecar 日志**:

```
COM SetFolder succeeded   : 0      (共 107 次 unavailable)
H1 succeeded              : 48
H1 failed                 : 8
H2 succeeded              : 8
```

**H1 历史上成功了 48 次** —— 它**不是结构性失效**。
我原方案里「H1 被永久禁用」的定性是**错的**,在此更正。

### 3.1 时间线揭示了完美的相关性

```
06:02:47  H1-OK                    ← 对话框「刚打开」(opened_write_back)
06:02:58  NOT-FG → H1-FAIL → H2    ← /set_path 推送
06:03:02  NOT-FG → H1-FAIL → H2    ← /set_path 推送
06:03:12  NOT-FG → H1-FAIL → H2    ← /set_path 推送
06:03:17  NOT-FG → H1-FAIL → H2    ← /set_path 推送
06:20:23  H1-OK                    ← 对话框「刚打开」
06:20:27  NOT-FG → H1-FAIL → H2    ← /set_path 推送
06:20:33  NOT-FG → H1-FAIL → H2    ← /set_path 推送
06:20:39  NOT-FG → H1-FAIL → H2    ← /set_path 推送
```

**规律毫无例外:**

| 触发来源 | 对话框是否前台 | H1 结果 |
|---|---|---|
| `opened_write_back`(对话框刚打开) | **是** | ✅ 成功(48/48) |
| `/set_path`(用户在 Sigma FM 里导航) | **否**(前台是 Sigma FM) | ❌ 必然失败 |

### 3.2 这不是 bug,是架构错配

> **用户在 Sigma FM 里切目录时,前台窗口按定义就是 Sigma FM,不可能是 Save As 对话框。**
> 所以 H1(SendInput 发送到前台)**在这条路径上永远赢不了**,与窗口站、UIPI、竞态都无关。

**这也说明**:诊断「为什么 `GetForegroundWindow()` 返回 0」的方向**本身就是错的** ——
即使它返回 Sigma FM 的 HWND 而不是 0,H1 依然会失败,因为对话框不是前台。
Claude 和我都被「0 这个值很奇怪」带偏了注意力。

### 3.3 修正后的因果链

```
对话框打开
  → opened_write_back → 对话框是前台 → H1 成功 → ★ 真正导航,用户能看到
用户随后在 Sigma FM 里切目录
  → /set_path → 对话框不是前台 → H1 必然失败 → H2 往 filename 框写路径再还原
  → 不是导航 → 用户看不到任何变化  ← ★ 用户抱怨的就是这一步
```

**所以用户的真实体验是**:「打开下载弹窗时它是对的,之后我在 Sigma FM 里切目录,它就不动了。」
这与「完全不能用」是不同的故障,而所有既有验收标准都没测这一步。

### 3.4 附带确认:COM 路径从未成功过

`COM SetFolder succeeded = 0 / 107`。用户的对话框一直是 WinUI 3 封装
(`has_filename_host=true has_1001=true`),`AccessibleObjectFromWindow` 拿不到经典
IFileDialog。这意味着**只要对话框是 WinUI 3,Stage 1(COM)就结构性不可用**。

---

## 4. 优化后的方案

### 4.1 采纳 Claude 的(与我一致)

| 编号 | 内容 | 优先级 |
|---|---|---|
| **A-1** | `writer.rs::write_path` 入口 `IsWindow` 检查 + 独立错误码 `dialog_gone` + 顺手 `remove_dialog` | P0 |
| **A-2** | `http_server.rs::respond_json` → `application/json; charset=utf-8` | P0 |

**A-1 的两条硬约束(与 Claude 一致,且我确认必要)**:
1. `dialog_gone` **不得**复用 `unsupported_dialog_type`
2. `dialog_gone` **不得**累加 `unsupported_dialog_count`
   —— 否则只是把噪声换个名字,focus-19 的阈值照样被污染

### 4.2 拒绝 Claude 的

| 编号 | 内容 | 处置 |
|---|---|---|
| ~~改动 1~~ | 简化 `should_opened_write_back` | **拒绝** —— gate 正在正常工作,改了会重新引入 sentinel 污染 |
| ~~改动 2~~ | `default_value` 退回 `"C:\\"` | **拒绝** —— 同上,且与改动 1 是共谋关系 |
| ~~改动 3~~ | 删 skip sentinel 分支 | **拒绝** —— 该分支是「安全门生效」的证据,删掉就静默了 |

> 若未来确有证据表明 gate 误拦,**先加观测再改**:
> 在 `else if` 分支已经有的 `opened_write_back_skip_sentinel` 日志里补上
> `first_push_received` / `initial_path` / `current` 三个值,然后看真实分布。

### 4.3 新增:修复真正的导航(这是 Claude 方案缺的核心)

**B-1 —— `/set_path` 路径下主动把对话框提到前台,然后重试 H1**

这是针对 §3.2 架构错配的直接解法,也是唯一能让「用户在 Sigma FM 切目录 → 对话框跟随」
真正工作的路径。

```rust
// /set_path 路径专用:对话框此刻不是前台,H1 必然失败。
// 标准做法:AttachThreadInput + SetForegroundWindow + BringWindowToTop,
// 再做一次前台确认,然后才允许 H1。
// 关键约束:必须带条件等待(轮询前台稳定),不要用固定 sleep。
fn try_focus_dialog(hwnd: HWND) -> bool {
    // AttachThreadInput(当前线程, 对话框所属线程, TRUE)
    // BringWindowToTop(hwnd); SetForegroundWindow(hwnd);
    // 轮询:GetForegroundWindow() == hwnd,最多 ~200ms
    // DetachThreadInput(..., FALSE)
}
```

**前置条件(必须先验证,否则可能白做)**:当前任务配置为
`RunLevel: Limited`(非提权)。若对话框属于**提权进程**,
非提权 sidecar 即使 `SetForegroundWindow` 也可能被 UIPI 拒绝。
**先做 B-2 的诊断,拿到对话框所属进程的完整性级别,再决定 B-1 是否可行。**

**B-2 —— 缺陷 C 诊断改为默认开启(修正 Claude 改动 7)**

范围扩展到:`GetForegroundWindow()` 原始值、窗口站名、桌面名、
**对话框所属进程的完整性级别**、以及**是哪个触发路径**(`opened` vs `set_path`)。

不需要 env var 门控 —— 它在注入路径上,每几百 ms 最多一次,开销可忽略。

**B-3 —— H2 read-back 验证(采纳 Claude 改动 6,但从「可选」升为「必做」)**

按 Claude Rule 58,H2 永远不是导航。既然如此,H2 就必须**诚实地报告自己是 no-op**,
否则 sidecar 会持续谎报 `Injection succeeded`,把真正的失败掩盖掉。

```rust
// H2 完成后读回 filename 控件与 dialog 面包屑,与 target 比对
if verified != target_path {
    return DialogOutcome::H2NotPersisted;   // 新增变体,不累加 unsupported_count
}
```

**这一步单独就能产生巨大价值**:它会把「H2 其实什么也没做」从推测变成日志里的事实,
而且**零风险**(只读不写)。

**B-4 —— 补一条缺失的端到端验收(见 §5)**

### 4.4 关于 Rule 63(诊断严格度 vs ship 阻塞)

我**部分同意**。但要加一条限定:

> 诊断可以并行、不阻塞。但**当诊断指向的问题是「功能核心不可用」时,
> 不能仅以「诊断还没做完」为由 ship 一个不解决核心问题的版本。**

具体到本例:缺陷 C(前台窗口)**就是**同步功能的核心。
所以 v0.5.6 若以「修好 A-1/A-2 + 诚实化 B-3」为范围 ship,**可以**;
但必须**明确声明它不修复「用户在 Sigma FM 切目录时对话框不跟随」**,
而不是宣称 ship 后功能恢复。

---

## 5. 验证计划(替换 Claude §4 中与核心功能相关的部分)

Claude 的编译/测试/binary probe/charset/死 HWND 验收**全部保留**。
以下是**新增**的:

```powershell
# V-1 ★ 核心:对话框打开后,在 Sigma FM 里切目录,对话框是否跟随
#   期望:spike.log 出现 H1-OK,或 B-3 生效后出现 H2NotPersisted(诚实失败)
#   判负:仍然只有 H2-OK 且用户看不到变化  => 功能未修复

# V-2 ★ B-3 生效性:确认 H2 到底做了什么
#   期望:日志能区分"H2 真提交了 breadcrumb"与"H2 是 no-op"
#   这一步不需要用户操作,只需要日志

# V-3 ★ 触发路径可分辨性
#   期望:每条注入日志都带 trigger=opened|set_path
#         => 今后一眼可判是"打开时同步"还是"切换时同步"

# V-4 对话框归属进程完整性级别(非提权 / 提权)
#   期望:拿到明确数值,决定 B-1 是否可行

# V-5 经典 IFileDialog 对照实验(零代码改动)
#   Edge 打开 edge://flags/#edge-legacy-file-picker → Enabled → 重启 Edge
#   期望:AccessibleObjectFromWindow 不再失败,COM SetFolder 首次出现 succeeded
#   若成立 => WinUI 3 是结构性问题,产品答案就是 legacy flag 而非继续打补丁
```

**V-5 特别值得先做**:零代码、零风险,而且它能一次性回答
「值不值得继续修 WinUI 3 这条路」。历史上 `COM SetFolder succeeded = 0/107`,
如果开了 legacy flag 后立刻变成成功,那么整个 H1/H2/F3 回退链都可以退休。

---

## 6. 决策建议

| 选项 | 内容 | 我的评价 |
|---|---|---|
| **A. Claude 原方案** | 改动 1-8 全做 | ❌ **不推荐**。1/2/3 有回归风险且不解决核心问题 |
| **B. Claude 方案 −改动1/2/3 +B-1/B-2/B-3** | 只保留正确的修复,补上真正的导航路径 | ✅ **推荐** |
| **C. 先做 V-5(legacy flag 对照)再决定** | 零代码实验,可能让整条 H 链退休 | ✅✅ **建议先做这个** |

### 推荐执行顺序

```
第 0 步  V-5  legacy file picker 对照实验(零代码,零风险)
         → 决定「继续修 WinUI 3」还是「改用产品既有答案」
第 1 步  B-2  诊断默认开启 + 加 trigger 路径标记
         → 之后每个现象都能一眼归因
第 2 步  B-3  H2 read-back 验证(必做,零风险)
         → 把「H2 是 no-op」从推测变成事实
第 3 步  A-1  死 HWND + dialog_gone
第 4 步  A-2  charset
第 5 步  B-1  提前台 + H1(取决于第 0 步与 V-4 结果)
```

**每步独立验证、独立可回滚**,遵守「一次只改一处」。

---

## 7. 给 Claude 的三个反问

1. **改动 1/2/3 的依据是什么?** 我实测 `first_push_received=true`、
   `opened_write_back` 有日志、`Write` 事件 48 次。若有其他证据,请给出日志出处。
2. **`initial_path` 在 `main.rs:355` 与 CLI 参数 `main.rs:30` 是同名不同作用域吗?**
   日志文案 `dialog path='{}' current='{}'` 说明前者是**对话框的路径**,
   后者是 sentinel。这两个同名变量是不是混淆了「Fix B + Fix F 共谋」的判断?
3. **`/set_path` 路径下 H1 必然失败这一点,方案里为什么没有体现?**
   按你们的 Rule 58,即便 H1 成功也只是往地址栏打字;
   但 H1 成功时(48 次)用户是**看到了导航的**,说明 `Ctrl+L` 在对话框前台时确实有效。
   那么「先把对话框提到前台,再 H1」为什么不在方案里?

---

## 8. 元判断

本次评估再次印证了我在 §1.5 沉淀的教训,而且多了一层:

> **不是「严格」或「宽松」的问题,是「证据够不够」的问题。**
>
> - 我因为没验证读取工具,得出一个假根因
> - Claude 因为没验证 gate 的实际运行状态,得出一个假根因
> - 我因为看到 `HWND(0x0)` 这个「异常值」,把注意力放错了地方,
>   而没有去看 **48 次成功**这个更大的样本
>
> **看到异常值就盯着异常值,是一种系统性偏误。**
> 正确做法是:先看全量分布,再决定异常值是否真的是异常。
> 本次如果我一开始就统计 `H1 succeeded / failed` 的历史总数,
> 而不是只读最近 20 行日志,就不会得出「H1 永久禁用」的错误结论。
>
> **新增规则(建议 Rule 66):**
> 在把任何观测值定性为「异常 / 永久 / 结构性」之前,
> 必须先统计该现象在**全部历史日志**中的分布。
> 最近 N 行只说明最近,分布才说明性质。
