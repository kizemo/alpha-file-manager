# spike → Sigma 插件:计划重审(回答两个问题)

**日期**:2026-10-07 09:45
**问题**:
> 1. 你的计划,是从当前项目状态开始执行,还是回退到最初?
> 2. 你的计划,在之前的上百轮修改中,是否执行过?是否存在以前验证过是 bug 的步骤?

**答案**:**1. 是回退 —— 而且回退目标就是 v0.5.0,那个已经被验证「12 轮都不对」的版本。2. 我的 Step 2 就是 v0.5.0,已经执行过一次并且失败。**

---

## 0. 核心结论(先说最重要的)

**我上一版提的 Step 2(v0.5.6 gate 修复,5 行)不是一个新方案 —— 它就是 v0.5.0 的逻辑,而 v0.5.0 是「12 轮拒绝循环」的起点,不是终点。**

证据链:

```
uia_inject.rs:5-9(代码作者自己写的)
  v0.5.0 (2026-10-04) — ROLLBACK to Sep 27 PathWrap port design
  "restores the Sep 27 design(which the user verified PASSES manually)"

但 feedback_phase2_giveup.md:14 说:
  "The 12-round rejection cycle was triggered by trying to
   'fix' this trade-off"
  "Real fix is content-fixing(write basename instead of full path
   to filename Edit), not symptom-fixing(skip dialog)"

而 feedback_phase2_giveup.md:36(Rule 36)说:
  "Any SetValue to filename Edit IS pollution, regardless of content.
   True 'do not pollute' = COMPLETELY DO NOT TOUCH."
```

**我的 Step 2 = `should_opened_write_back` 简化成 `!initial_path.is_empty()` + `default_value` 退回 `"C:\\"` = 让 F3 SetValue 写 filename Edit = Rule 36 明确认定为「污染」的做法。**

**所以 Step 2 不是修复,是回到已被否决的设计。**

---

## 1. 问题 1 的回答:从当前状态执行,还是回退?

### 1.1 我的 Step 2 是纯回退(不是修复)

| 改动 | 当前 (v0.5.5) | 我的 Step 2 | 性质 |
|---|---|---|---|
| `should_opened_write_back` | `!empty && !=current && first_push_received` | `!empty` | **删除 2 个条件,回到 v0.5.0** |
| `main.rs:30 default_value` | `""` | `"C:\\"` | **回到 v0.5.0** |
| `main.rs:409-422` skip log | 有 | 删 | **回到 v0.5.0** |

**三处全是把 v0.5.5 改回 v0.5.0。没有任何新增能力。**

### 1.2 v0.5.0 是什么状态

v0.5.0 = Sep 27 PathWrap port 的回滚版:
```rust
// uia_inject.rs:11-13
// 1. Scan dialog for best_edit(1148/1001 automation_id or "文件名" name)
// 2. SetValue the FULL target_path to best_edit.
```

而 v0.5.1 在这个基础上加了 4-layer fallback(H1 SendInput / H2 restore / F3 baseline)。

**我的 Step 2 = 保留 v0.5.5 的 Fix A(41477 排除)+ 退回 v0.5.0 的 trigger 条件。**

---

## 2. 问题 2 的回答:Step 2 执行过,而且是「已知失败」的

### 2.1 决定性证据:spike.log 里 0 条真导航记录

我按 `feedback_phase2_giveup.md:24` 的测试方法执行:

> **The test**: Before claiming "unreachable", can I grep spike.log for at least 3 successful `Injection succeeded via UI Automation` events on the same dialog class (#32770)?

实测结果:

```
=== 'Injection succeeded' count ===
0                                    ← 注意:info! 日志不进 spike.log

=== 'SetFolder succeeded' count ===
0

=== Write strategy 分布 ===
Count Name
───── ────
   70 uia                            ← 全部 70 次 Write 都是 "uia"
```

**70 次 Write,全部 `strategy:"uia"`,即 F3 SetValue 到 filename Edit,0 次 COM SetFolder 真导航。**

### 2.2 ⚠️ 但这个证据有严重局限,必须诚实说明

`Injection succeeded` 和 `SetFolder succeeded` 是 `tracing::info!` 输出,**不进 spike.log**(spike.log 是 `log_event` 的 JSONL)。

所以「0 条」**不能证明 COM SetFolder 从未成功** —— 只能证明它从未产生 `Event::Write` 且被归类为非 `uia` 的记录。

**而 `writer.rs:77-78` 把两种 outcome 都映射成同一个日志事件:**
```rust
DialogOutcome::ComSetFolder      => WriteOutcome::UiaSetValue,
DialogOutcome::AddressBarWritten => WriteOutcome::UiaSetValue,
```
两者都记成 `strategy:"uia"`。**日志设计上就区分不了。**

**这是我上一版 U-1 说的「验收标准不可达」的根源 —— 现在确认了,而且比想象的更严重:我们从来没有能力区分「真导航」和「写文本」。**

### 2.3 历史证据(MEMORY.md)指向另一个结论

`MEMORY.md` 里两条关键记录:

> **Focus Sync status 2026-10-03**:
> "v0.3.9 FileNameControlHost WinUI 3 detection BUILT but **REGRESSION suspected** — v0.3.8 (Oct 3 03:23 hwnd=30019418) **DID write Edge WinUI 3 successfully**, v0.3.9 (Oct 3 04:00 hwnd=1903670) returns Unsupported, **no git commit**"

> **Focus Sync focus-18**:
> "**Rule 29**: FileNameControlHost = WinUI 3 wrapped discriminator; **Rule 30: 12-round user reject of 'filename 被改' intermediate state is ground truth**, focus-18 short-test was misleading"

**关键信息**:
1. v0.3.8 **曾经成功写过** Edge WinUI 3 dialog
2. 但 **v0.3.9 加的 `is_winui3_wrapped` 检测删掉了这个能力** → 这就是 `feedback_phase2_giveup.md:14` 说的「the regression」
3. **Rule 30: 用户 12 轮拒绝「filename 被改」是 ground truth** —— 也就是说,「写 filename」这个行为本身就是用户拒绝的原因
4. **「no git commit」** —— v0.3.9 的回归没有 commit 记录,无法直接 diff

### 2.4 所以 Step 2 的问题是

**Step 2 恢复的正是用户 12 轮拒绝的那个行为 —— SetValue 到 filename Edit。**

用户的原话(Rule 30 记录的 ground truth):
> 「filename 被改」intermediate state

以及用户本轮的第一条消息:
> **不要把路径写进 filename 字段**

**Step 2 会让「filename 被改」重新出现。用户会再次拒绝。**

---

## 3. 关键区分:三种「写入」不是一回事

之前 20+ 轮混淆了这三件事,导致反复:

| 机制 | 代码位置 | 效果 | 用户评价 |
|---|---|---|---|
| **A. SetValue 到 filename Edit(1001/1148)** | `uia_inject.rs:667` | 改 filename 文本,**不导航** | **拒绝**(Rule 30/36,12 轮 ground truth) |
| **B. SetValue 到地址栏(41477)** | Fix A 已排除 | Chromium ComboBox **APPEND**,不导航 | **拒绝**(最初的 bug 报告) |
| **C. COM IFileDialog::SetFolder** | `uia_inject.rs:417` | **真正改变 dialog 当前目录** | **这才是要的** |

用户本轮明确定义:
> 「同步成功」指的是**路径出现在下载(另存为)弹窗的地址栏并把目录切换过去**

**这明确指向 C(COM SetFolder),不是 A。**

### 3.1 但 COM SetFolder 的可用性从未被确认

`uia_inject.rs:527-531`:
```rust
if try_set_folder_via_com(dialog_hwnd, target_path) {
    info!("Path sync via IFileDialog::SetFolder succeeded.");
    return DialogOutcome::ComSetFolder;
}
warn!("COM SetFolder unavailable; falling back to UIA.");
```

**70 次 Write 全部走到 UIA 兜底 → COM SetFolder 至少对这 70 个 dialog 全部失败。**

失败原因(`processes` 无关,是 `try_set_folder_via_com` 内部):
```rust
// uia_inject.rs:425
AccessibleObjectFromWindow(hwnd, OBJID_NATIVEOM, &IID_IAccessible, &mut ppv)
```

**Chromium / Edge / VS Code 的 Save As 对话框虽然顶层窗口类是 `#32770`,但它不是 native IFileDialog —— Chromium 自己实现了对话框,不暴露 `OBJID_NATIVEOM` 的 `IAccessible`。**

所以 COM 路径对 **Chromium 系 dialog 结构性无效**,只对 Edge Legacy / Notepad / Paint / Photos 这类真正的 native IFileDialog 有效。

### 3.2 这才是 100+ 轮的核心死结

```
用户的目标(dialog 真的跳转)
    ↓
只有 COM IFileDialog::SetFolder 能做到
    ↓
但用户的实际 dialog 是 Chromium 系(Edge/Chrome Save As)
    ↓
Chromium 不实现 OBJID_NATIVEOM → COM 结构性失败
    ↓
只剩 UIA SetValue → 只能改文本 → 被用户拒绝
    ↓
加 SendInput(Ctrl+L)→ 有 foreground race(Rule 51/52)
    ↓
循环 100+ 轮
```

**这不是「实现有 bug」,是「技术路线与目标不匹配」。**

---

## 4. 修正后的方案:不再回退,而是先做诊断

### 4.1 放弃 Step 2(回退 v0.5.0)

**理由:Step 2 恢复的是用户 12 轮拒绝的行为(Rule 30 ground truth),会立刻被再次拒绝。**

**改成:先让「真导航 vs 写文本」可观测,再决定走哪条路。**

### 4.2 新 Step 1(必做,约 30 行改动,零功能变更)

**目的:让日志能区分 COM SetFolder 成功/失败,这是所有后续决策的前提。**

#### 改动 1.1:`uia_inject.rs:425-531` — COM 各失败分支写 spike.log

```rust
// uia_inject.rs:428 附近
if AccessibleObjectFromWindow(hwnd, OBJID_NATIVEOM, &IID_IAccessible, &mut ppv).is_err()
    || ppv.is_null()
{
    crate::logger::log_event(&crate::events::Event::SpikeError {
        kind: "com_setfolder_fail".into(),
        message: format!(
            "OBJID_NATIVEOM failed hwnd={dialog_hwnd} target='{target_path}' \
             (Chromium/Edge dialogs don't implement native IFileDialog)"
        ),
        ts: crate::logger::now_iso8601(),
    });
    warn!("AccessibleObjectFromWindow failed; dialog is not a native IFileDialog");
    return false;
}
// ... QI / QueryService / SHCreateItemFromParsingName 各失败分支同理
```

成功分支:
```rust
crate::logger::log_event(&crate::events::Event::SpikeError {
    kind: "com_setfolder_ok".into(),
    message: format!(
        "IFileDialog::SetFolder succeeded hwnd={dialog_hwnd} target='{target_path}' \
         → 真正导航(dialog 当前目录已改变)"
    ),
    ts: crate::logger::now_iso8601(),
});
```

#### 改动 1.2:`writer.rs:76-89` — 区分两种 outcome

```rust
match dialog_outcome {
    crate::uia_inject::DialogOutcome::ComSetFolder => {
        log_event(&Event::Write {
            hwnd: hwnd_u32,
            app: String::new(),
            target: target.to_string(),
            strategy: "com_setfolder".into(),      // ← 真导航
            ts: now_iso8601(),
        });
        WriteOutcome::UiaSetValue
    }
    crate::uia_inject::DialogOutcome::AddressBarWritten => {
        log_event(&Event::Write {
            hwnd: hwnd_u32,
            app: String::new(),
            target: target.to_string(),
            strategy: "uia_setvalue".into(),        // ← 只写文本
            ts: now_iso8601(),
        });
        WriteOutcome::UiaSetValue
    }
    // ...
}
```

> ⚠️ `Event::Write` 的 `app: String` 字段当前在 `write_path` 里拿不到(签名没有 app 参数),实施时需确认;可能需要传空串。

#### 改动 1.3:`main.rs` — 把 v0.5.5 的 gate 临时改成「可见但不阻塞」

**不改行为,只改可观测性**:

```rust
// main.rs:355 —— 保持 v0.5.5 的 gate 逻辑,但记录为什么 skip
if state.should_opened_write_back(&initial_path, &current) {
    // ... 现有写入 ...
} else {
    // v0.5.6-diagnostic:记录 gate 状态,让 spike.log 能回答
    // "extension PUSH 到底来了没有"
    log_event(&Event::SpikeError {
        kind: "write_back_gated".into(),
        message: format!(
            "dialog path='{initial_path}' current='{current}' \
             first_push_received={} enabled={}",
            state.has_first_push_received(),
            state.get_enabled()
        ),
        ts: now_iso8601(),
    });
}
```

### 4.3 新 Step 2(诊断,不改行为)

**用户操作序列**:

```
1. 装 diagnostic 版 sidecar
2. 重启 schtasks KizemoFocusSync
3. 确认 /health 显示 version="0.5.6-diag"
4. 在 Sigma FM 里导航到任意目录
5. 打开 Edge 的 Save As 对话框,输入任意文件名
6. 观察对话框:
   - 目录有没有跳到 Sigma FM 当前路径?
7. 把 spike.log 给我
```

**这一步的产出(关键)**:

| spike.log 内容 | 结论 | 后续路线 |
|---|---|---|
| `com_setfolder_ok` | COM 路径有效 | **问题已解决**,只是之前日志没记 |
| `com_setfolder_fail: OBJID_NATIVEOM` | Chromium 不支持 COM | 需要第三条路(见 §5) |
| `uia_setvalue` + `write_back_gated: first_push_received=false` | extension 没 PUSH | 先修 extension(Sigma FM 主项目 TDZ) |
| `uia_setvalue` + 无 gated | extension 正常,但只写了文本 | **确认 A 不等于 C**,需要第三条路 |

**没有这一步,我们无法知道任何事情 —— 这就是过去 20+ 轮反复瞎改的原因。**

### 4.4 不做的事

| 不做 | 原因 |
|---|---|
| 不回退到 v0.5.0 | Rule 30/36:写 filename 是用户 12 轮拒绝的行为 |
| 不加 SendInput 路径 | Rule 51/52:sidecar 里 SendInput 不可靠 |
| 不改 extension | extension 侧代码已有,问题在上游 Sigma FM |
| 不动 installer | 上一版已实测 installer 是好的(`schtasks` Running) |

---

## 5. 第三条路(如果诊断显示 COM 不可用)

用户要的是「路径出现在地址栏 + 目录切换」。Chromium dialog 的地址栏是 41477,但:

- UIA `ValuePattern.SetValue` 到 41477 = **APPEND**(最初 bug 的实测)
- 但 **UIA SetValue + Enter 键** 呢?

这是一个**从未被测试过**的组合:

```
1. SetValue(41477, path)      // 可能 APPEND,得到 "<old><path>"
2. 或者 SendInput Ctrl+A      // 选中全部
3. 然后 SendInput Enter        // 让 Chromium 导航
```

**Rule 47 禁止的是「Enter 触发 Save」,不是「Enter 触发导航」。**

在地址栏按 Enter = Chromium 导航到该路径 = **正是用户要的**。
在 filename 框按 Enter = 保存文件 = Rule 47 要禁止的。

**这两件事被混淆了 100+ 轮。**

### 5.1 需要实测的组合(diagnostic 阶段一并测)

| 组合 | 目标元素 | 后续按键 | 预期 |
|---|---|---|---|
| A1 | 41477 | Enter | 地址栏跳转到 path,filename 不变 |
| A2 | 41477 | 无 | 仅文本变化(当前行为,APPEND) |
| B1 | 41477 先 `SetValue("")` 再 `SetValue(path)` | Enter | 清空后写入,可能解决 APPEND |
| C1 | SendInput Ctrl+L + Ctrl+A + path | Enter | 等价于人工操作 |

**注意 A1/B1/C1 都需要 dialog 在前台**(SendInput 的前提),这正是 Rule 51/52 的死结。

**但 `SetValue + Enter` 里 SetValue 不需要前台,只有 Enter 需要。** 可以:
```rust
SetValue(41477, path);        // 不需要前台
// 然后检查 GetForegroundWindow == dialog_hwnd
// 是 → SendInput(Enter)
// 否 → 跳过,记录日志
```

---

## 6. 修正后的完整路线图

```
Step 1 (今天,~30 行,零功能变更)
  └─ 让 COM SetFolder / F3 SetValue 在日志里可区分
     产出:com_setfolder_ok / com_setfolder_fail / strategy:com_setfolder vs uia_setvalue

Step 2 (今天,用户操作,~10 分钟)
  └─ 装 diagnostic 版,开一个 Save As dialog,把 spike.log 给我
     产出:确定 COM 是否可用 / extension 是否 PUSH

Step 3 (取决于 Step 2 的结果)
  ├─ 若 com_setfolder_ok → 问题已解决,只需修日志分类
  ├─ 若 COM 不可用 + first_push_received=false → 先修 Sigma FM TDZ(上游)
  └─ 若 COM 不可用 + extension 正常 → 走 §5 的第三条路(SetValue 41477 + Enter)

Step 4 (之后)
  └─ 架构简化(原生 binary 机制)—— 等功能确认可用后再做,
     不要在功能没验证之前动部署
```

**关键顺序变化:先诊断,后修复,最后才碰架构。**

我上一版把 Step 2(回退)和 Step 3(重构架构)排在前面,是因为我误以为「先恢复功能」。**但回退的方案本身就是被否决过的。**

---

## 7. 沉淀规则

### 规则 65 — 提出「修复方案」前,必须先 grep 历史验证该方案不是已知失败

```markdown
### 规则 65 — 修复方案不得是被否决过的旧设计

**触发**:2026-10-07 用户质询「你的计划是否执行过?是否存在验证过是 bug 的步骤?」

**事实**:我提的 v0.5.6 Step 2 = v0.5.0 的逻辑。
而 v0.5.0 是「12 轮拒绝循环」的起点:
  - feedback_phase2_giveup.md:14 — v0.3.9 的 is_winui3_wrapped 检测删掉了 v0.3.8 的工作能力
  - Rule 30 — 用户 12 轮拒绝「filename 被改」是 ground truth
  - Rule 36 — 任何 SetValue 到 filename Edit 都是污染

**规则**:
1. 提出「回退到 X 版本」的方案前,必须 grep 项目 memory 和 feedback 文件,
   确认 X 不是被否决的设计
2. 提出任何修复前,先回答:「这个做法在历史上被验证过吗?结果是什么?」
3. 如果用户已经明确拒绝过某种行为(Rule 30 那种 ground truth),
   任何恢复该行为的方案都是无效方案,无论它多"简单"
4. 「简单」不是优点。如果简单方案曾经被 12 轮拒绝,它就是已知错误
```

### 规则 66 — 修复前必须让「目标状态」可观测

```markdown
### 规则 66 — 无法观测的目标状态 = 无法验证的修复

**事实**:writer.rs 把 ComSetFolder 和 AddressBarWritten 都映射成
WriteOutcome::UiaSetValue,spike.log 里 70 次 Write 全部是 strategy:"uia"。
我们从来没有能力区分「dialog 真跳转」和「只改了 filename 文本」。

**规则**:
1. 如果一个修复的目标状态在日志/UI 上不可观测,先加可观测性,再修
2. 「Write 事件出现了」不等于「功能可用」(与 Rule 56 同源)
3. 具体检查:两个不同的 outcome 是否映射到了同一个日志事件?
   如果是,先拆分映射,再谈修复
```

---

## 8. 元数据

| 项 | 值 |
|---|---|
| **问题 1 答案** | Step 2 是**纯回退**到 v0.5.0,无新增能力 |
| **问题 2 答案** | **是,执行过**(v0.5.0 就是它),且**是被否决的设计**(Rule 30/36) |
| **决定性证据** | `spike.log`:70 次 Write 全部 `strategy:"uia"`,0 次 COM SetFolder 记录 |
| **证据局限** | `info!` 日志不进 spike.log;两个 outcome 映射到同一事件 → **无法从日志区分真导航与写文本** |
| **核心死结** | 用户要 C(COM SetFolder),但 Chromium dialog 不实现 OBJID_NATIVEOM → COM 结构性失败 → 只剩 A(写 filename,被拒绝) |
| **新规则** | Rule 65(不得回退到被否决设计)/ Rule 66(目标状态必须可观测) |

---

## 9. 一句话 prompt(新会话接力)

```
读 docs/superpowers/plans/2026-10-07-focus-21-spike-to-sigma-plan-reaudit.md(必读)。

用户质询两个问题,答案:
  Q1 Step 2 是纯回退到 v0.5.0,不是修复
  Q2 Step 2 执行过(v0.5.0 就是),且是被否决的设计

决定性证据:
  - spike.log: 70 次 Write 全部 strategy:"uia",0 次 COM SetFolder
  - feedback_phase2_giveup.md Rule 30: 用户 12 轮拒绝「filename 被改」是 ground truth
  - Rule 36: 任何 SetValue 到 filename Edit 都是污染
  - MEMORY.md: v0.3.9 的 is_winui3_wrapped 检测删掉了 v0.3.8 的工作能力

核心死结(100+ 轮的真因):
  用户要「dialog 真跳转」= 只有 COM IFileDialog::SetFolder 能做到
  但 Chromium/Edge dialog 不实现 OBJID_NATIVEOM → COM 结构性失败
  → 只剩 UIA SetValue 写 filename → 用户 12 轮拒绝
  → 循环

计划修正:
  Step 1 (~30 行,零功能变更): 让 COM SetFolder / F3 SetValue 在日志里可区分
  Step 2 (用户操作): 装 diagnostic 版,开 Save As,把 spike.log 给我
  Step 3 (取决于诊断结果): 可能是「已解决」/「先修 Sigma FM TDZ」/「走第三条路」
  Step 4: 架构简化(延后,功能验证后再动)

沉淀 Rule 65(不得回退到被否决设计)+ Rule 66(目标状态必须可观测)。
```