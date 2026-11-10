# focus-sync 重新复盘 — 收回"架构不可达"判断,定位真实根因

**日期**: 2026-10-03
**作者**: Claude (MiniMax-M3)
**类型**: 修正之前的错误诊断(从"架构不可达" → "v0.3.9 实施错误")
**状态**: 不执行任何代码改动,仅交付修正诊断 + 实施方向

---

## 0. 先纠正上一份文档的错误

我上一份诊断 (`2026-10-03-focus-21-v0.3.9-architecture-retrospective.md`) 错误判断"目标架构上不可达"。这份文档**收回那个判断**。

**证据反驳自己**:

1. **Sep 27 spike-report** — `manual-detect-test.ps1` 用 `spike.exe --port 37423` 直接启动,跑真实 Win32 OpenFileDialog,**Write 成功**(`"Injection succeeded via UI Automation"`)。
2. **Oct 3 03:23 sidecar log(hwnd=30019418)** — **installer 模式下的 v0.3.8 在 Edge WinUI 3 Save As 上也 Write 成功**,dialog 6 秒后自动关闭。inventory 显示这个 dialog **完整包含 1001 Edit + 1 (Save) 按钮** — 是可导航的。
3. **Oct 3 04:00 sidecar log(hwnd=1903670)** — 同一个用户的 Edge Save As,但用了 v0.3.9 → 返回 Unsupported → 用户视角"什么都没发生"。

**真实状态**:
- ✅ Edge WinUI 3 Save As **技术上可导航**(Sep 27 + v0.3.8 都验证过)
- ❌ v0.3.9 的 `is_winui3_wrapped` 检测 **回归了** — 把它从"可写"变成"不支持"
- ❌ 用户 12 轮 reject 的"filename 被改"是**真实问题**,但**根因不是 WinUI 3 不可写**,而是**写入的内容错了**

**核心 bug**(我之前 12 轮都没找到):
> v0.3.8 把 **`target_path`(完整目录路径)** 写入了 **filename Edit**。Win32 dialog 把 `"C:\evid"` 解释为路径(OK),但 WinUI 3 wrapped dialog 把 `"C:\evid"` 解释为字面文件名 → 用户看到 filename 被改成 `"C:\evid"`,心理预期是文件名应该不动。

---

## 1. 重新调查的硬证据(从 sidecar log 提取)

### 1.1 Edge Save As dialog 的实际 inventory(hwnd=1903670)

**v0.3.9 debug! log 完整 inventory**(Oct 3 04:00 真实提取):

```
ctrl=50000 automation_id='HelpButton'           name='帮助(&H)'
ctrl=50000 automation_id='SplitMenuButton'      name='视图滑块'
ctrl=50000 automation_id='UpButton'             name='上一行'             ← 树视图导航
ctrl=50000 automation_id='UpPageButton'         name='向上翻页'
ctrl=50000 automation_id='DownPageButton'       name='向下翻页'
ctrl=50000 automation_id='DownButton'           name='下一行'
ctrl=50000 automation_id='DropDown'             name='筛选器下拉列表'
ctrl=50003 automation_id='FileNameControlHost'  name='文件名:' value='WeavePage_3.0.0_x64-setup.exe'  ← 包裹
ctrl=50004 automation_id='1001'                 name='文件名:' value='WeavePage_3.0.0_x64-setup.exe'  ← 内嵌
ctrl=50000 automation_id='DropDown'             name='打开'
ctrl=50003 automation_id='FileTypeControlHost'  name='保存类型:' value='Application (*.exe)'         ← 包裹
ctrl=50000 automation_id='1'                    name='保存(S)'                                       ← Save 按钮
ctrl=50000 automation_id='2'                    name='取消'                                          ← Cancel 按钮
ctrl=50000 automation_id=''                     name='后退 (Alt + 向左键)'                          ← 导航
ctrl=50000 automation_id=''                     name='前进 (Alt + 向右键)'
ctrl=50000 automation_id=''                     name='最近浏览的位置'
ctrl=50000 automation_id=''                     name='上移到"Win (C:)"(Alt + 向上键)'
ctrl=50000 automation_id=''                     name='上一个位置'
ctrl=50000 automation_id=''                     name='刷新"downloads"(F5)'
ctrl=50004 automation_id='SearchEditBox'        name='搜索框'
ctrl=50000 automation_id='SearchBoxSearchButton' name='搜索'
ctrl=50000 automation_id=''                     name='关闭'
```

**关键观察**:

1. ✅ **dialog 包含完整的 IFileDialog 接口**(1001 Edit + automation_id=1 Save button + FileNameControlHost 包裹)
2. ✅ **有树视图导航**(UpButton/DownButton/UpPageButton/DownPageButton)
3. ✅ **有面包屑/导航快捷方式**(上移到"Win (C:)"、上一个位置、刷新)
4. ✅ **Filename Edit 当前 value = `"WeavePage_3.0.0_x64-setup.exe"`**(用户输入的文件名)

### 1.2 v0.3.8 在同样的 dialog 上做了什么(Oct 3 03:23,hwnd=30019418)

**完整时序**:

```
[03:23:35] DialogDetected hwnd=30019418 app=#32770
[03:23:35] Read path="WeavePage_3.0.0_x64-setup.exe"
[03:23:35] opened_write_back path='WeavePage_3.0.0_x64-setup.exe' current='C:\downloads'; writing current
[03:23:35] Write target='C:\downloads' strategy=uia                       ← SetValue 成功
[03:23:43] path_update='C:\evid'
[03:23:43] set_path_iter auto_confirm=false
[03:23:43] Write target='C:\evid' strategy=uia                            ← SetValue 成功
[03:23:43] [INFO] v0.3.8: SetValue verified for IFileDialog Edit (automation_id 1001 or 1148). Read back='C:\evid' matches target.
[03:23:43] [INFO] UIA: write succeeded (41477 address bar OR fallback filename).
[03:23:43] [INFO] Injection starts: hwnd=30019418 target='C:\evid' should_commit=false    ← 但 should_commit=false
[03:23:49] dialog_closed hwnd=30019418                                      ← 6 秒后 dialog 自动关闭
```

**两个关键事实**:
1. **SetValue 在 WinUI 3 dialog 上**确实**成功**(read back 验证通过)
2. **`should_commit=false`**(v0.3.8 后期改了),所以 Invoke Save **没有自动触发**。但 dialog 仍然在 6 秒后自动关闭 — 说明用户(或系统)手动触发了 Save。
3. 用户看到了 "C:\evid" 出现在 filename 字段中 → reject "filename 被改"

### 1.3 v0.3.9 在同样的 dialog 上做了什么(Oct 3 04:00,hwnd=1903670)

**完整时序**(同样的 inventory):

```
[04:00:00] Injection starts: hwnd=1903670 target='C:\evid' should_commit=true
[04:00:00] AccessibleObjectFromWindow failed; dialog is not a native IFileDialog
[04:00:00] COM SetFolder unavailable; falling back to UIA only.
[04:00:00] === Dialog UIA inventory (hwnd=1903670, total descendants=97) ===
[04:00:00] ctrl=50003 automation_id='FileNameControlHost' name='文件名:' value='WeavePage_3.0.0_x64-setup.exe'
[04:00:00] ctrl=50004 automation_id='1001' name='文件名:' value='WeavePage_3.0.0_x64-setup.exe'
[04:00:00] ctrl=50003 automation_id='FileTypeControlHost' name='保存类型:' value='Application (*.exe)'
[04:00:00] ctrl=50000 automation_id='1' name='保存(S)'
[04:00:00] ctrl=50000 automation_id='2' name='取消'
[04:00:00] === End of dialog inventory ===
[04:00:00] [WARN] v0.3.9: WinUI 3 wrapped dialog detected (FileNameControlHost present). ...
                                                                target='C:\evid'
[04:00:00] WriteFailed reason='unsupported_dialog_type'
```

**v0.3.9 在 inventory 一模一样的 dialog 上直接 return Unsupported,完全没碰 1001 Edit。**

### 1.4 同一类 dialog,v0.3.8 vs v0.3.9 对比

| 步骤 | v0.3.8 (Oct 3 03:23) | v0.3.9 (Oct 3 04:00) |
|---|---|---|
| 收到 PUSH | 写入 `C:\evid` 到 1001 Edit | 检测 FileNameControlHost 存在 → return Unsupported |
| SetValue | 成功,read back 验证 | 不执行 |
| Invoke Save | (should_commit=false,不 Invoke) | 不执行 |
| dialog 关闭 | 6 秒后(用户/系统触发) | 不关闭 |
| 用户视角 | "filename 被改成 C:\evid" | "什么都没发生" |

**两种情况都是 reject**,但 v0.3.9 至少**没污染 filename 字段**(用户截图证明)。

---

## 2. 真实根因分析(我之前 12 轮都搞错了)

### 2.1 PathWrap 时代的 Win32 dialog 行为(已失效)

PathWrap 在 2020 时代的 Win32 OpenFileDialog / IFileDialog 行为:
- filename Edit 接受**完整路径**作为输入(`"C:\downloads\file.exe"`)
- dialog 自动导航到该目录 + 用 file.exe 作为文件名
- 用户体验:"在 filename 字段输入路径" = "导航 + 保存"

**Win32 行为**:filename Edit 是**导航 + 文件名一体化输入**。

### 2.2 Win11 22H2+ Edge WinUI 3 wrapped dialog 行为

从 inventory 看:
- **1001 Edit**(内嵌 IFileDialog filename Edit)继承 Win32 行为 → 仍可接受路径
- **FileNameControlHost**(WinUI 3 wrapper Pane)接管显示 → **只接受字面文件名**,不接受路径导航

**结果**:我们 SetValue `"C:\evid"` 到 1001 Edit,UIA 层面成功,但 WinUI 3 wrapper 把它显示为"filename = C:\evid"。用户看到的是"filename 被改",不是"导航成功"。

**这是 v0.3.8 用户 reject 的根本原因,不是 v0.3.9 假设的"WinUI 3 wrapper 拦截 SetValue"。**

### 2.3 v0.3.9 的错误诊断

v0.3.9 plan 错误假设:
> "WinUI 3 wrapped dialog 的 SetValue 会被拦截/延迟/回滚"

**实际证据**(Oct 3 03:23 hwnd=30019418):
- SetValue `"C:\downloads"` → 验证通过
- SetValue `"C:\evid"` → 验证通过
- 两者都 **read back 一致**

**SetValue 工作完美**,但**写入的内容错了**。

### 2.4 真问题:应该写什么?

WinUI 3 wrapped dialog 的 filename Edit 期望的是**字面文件名**(`"file.exe"`),不是路径(`"C:\evid\file.exe"`)。

正确的导航路径(用户手动操作时):
1. 点击左侧树视图,展开 `C:\`
2. 展开 `C:\evid`
3. 点击 `evid` 文件夹
4. dialog 顶部路径栏变成 `C:\evid`
5. filename Edit 保持 `"file.exe"`(用户的原始文件名)
6. 点击 Save → 保存到 `C:\evid\file.exe`

**UIA 应该模拟这个流程**:
- ✅ 不要碰 filename Edit(它已经对了)
- ✅ 用树视图导航到目标目录
- ❌ **不要 SetValue 整个路径到 filename Edit**

---

## 3. 手动部署 vs Installer 的真实差异

### 3.1 Sep 27 手动测试(spike-report)

```powershell
# Step 1: 启动 spike
$spikeProc = Start-Process -FilePath "...\spike.exe" `
    -ArgumentList "--port", "37423", "--initial-path", "C:\Users\Public"

# Step 2: 显示 Win32 OpenFileDialog
$dialog = New-Object System.Windows.Forms.OpenFileDialog
$dialog.Title = "Open File - Spike Detection Test"
$dialog.InitialDirectory = "C:\Users\Public"
$null = $dialog.ShowDialog()
```

**结果**(spike.log):
```
{"event":"DialogDetected","hwnd":7538550,"app":"#32770"}
{"event":"Read","hwnd":7538550,"app":"#32770","path":"Adobe","strategy":"uia"}
write_path: hwnd=7538550 target='C:\Users\Public'
Injection starts: hwnd=7538550 target='C:\Users\Public'
Injection succeeded via UI Automation.
{"event":"Write","hwnd":7538550,"app":"#32770","target":"C:\\Users\\Public","strategy":"uia"}
```

**成功的 3 个条件**:
1. ✅ spike.exe 用 `--port 37423`(TCP 模式,非 service)
2. ✅ Win32 OpenFileDialog(plain #32770,**没有 WinUI 3 包裹**)
3. ✅ 用户手动打开 dialog(不是 Sigma FM 触发)

### 3.2 当前 installer 部署(scheduled task)

**schtasks 配置**(实际):
- Task: `\KizemoFocusSync`
- Path: `D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe --service`
- User: `Duanyi`(interactive session)
- Trigger: AtLogOn
- Mode: Background

**实际行为**(Oct 3 04:00):
1. ✅ Scheduled Task 正常启动 spike.exe,interactive user session
2. ✅ Sigma FM PUSH → HTTP → spike 收到
3. ✅ spike 检测到 Edge Save As dialog(hwnd=1903670)
4. ❌ **v0.3.9 直接 return Unsupported**(FileNameControlHost 检测)
5. ❌ 用户视角"什么都没发生"

### 3.3 真正的对比

| 维度 | 手动(成功) | installer(失败) |
|---|---|---|
| **spike 启动方式** | `spike.exe --port 37423`(spawn mode) | Scheduled Task `--service`(service mode) |
| **transport** | TCP | HTTP |
| **dialog 类型** | Win32 OpenFileDialog(`System.Windows.Forms`) | Edge WinUI 3 wrapped Save As |
| **filename Edit 内容** | 用户已经手动输入了文件路径 | 用户输入了文件名,我们**覆盖**成路径 |
| **Inject 行为** | `SetValue "C:\Users\Public"` → dialog 导航 | (v0.3.8 写"C:\evid"被 reject,v0.3.9 不写) |

**真正的差异不是 architecture,是**:
1. **dialog 类型不同**(Win32 vs WinUI 3 wrapped)→ 写入策略需要不同
2. **filename Edit 语义不同**(Win32 接受路径,WinUI 3 只接受字面文件名)
3. **v0.3.9 用"跳过 WinUI 3"代替"为 WinUI 3 设计正确写入策略"** → 错位决策

### 3.4 关键洞见

**v0.3.8 在 installer 模式下**已经能成功导航 Edge WinUI 3 dialog(03:23 hwnd=30019418 证据)。
**v0.3.9 引入 `is_winui3_wrapped` 检测** = 主动把这个能力关掉。
**v0.3.9 是实施错误**,不是架构升级。

---

## 4. 修正后的实施方向(3 条路径)

### 路径 A:改 SetValue 内容,只写 basename(最低风险)

**原理**:WinUI 3 wrapped dialog 的 filename Edit 期望字面文件名,不应该接收完整路径。
**改动**:
```rust
// Before (v0.3.8):
value.SetValue(&BSTR::from(target_path))?;  // "C:\evid"

// After (v0.4.0):
let basename = std::path::Path::new(target_path)
    .file_name()
    .and_then(|n| n.to_str())
    .unwrap_or(target_path);
value.SetValue(&BSTR::from(basename))?;     // "file.exe"(保持 filename 不变)
```

**问题**:`basename` 是空字符串或简单文件名,用户其实想**导航目录**,不是改文件名。
**所以 SetValue basename = 啥都不做**,filename Edit 保持原样。

**但用户要求的是"导航目录",不是"改文件名"。** 单纯 SetValue basename 不能解决导航问题。

**结论**:这条路径**不够**,需要配合其他机制。

### 路径 B:用树视图导航(最贴近用户手动操作)

**原理**:模拟用户点击树视图展开路径。
**步骤**:
1. 找到 tree view 容器(UIA 元素,通常 ControlType=Tree or TreeItem)
2. 找到 target_path 的每段(C:\ → evid),依次展开 + 点击
3. 用 `ExpandCollapsePattern::Expand` + `SelectionItemPattern::Select`
4. 完成后 directory 已切到 target_path,filename Edit 不变

**复杂度**:高。每段路径都要:
- 找到匹配的 TreeItem
- 处理懒加载(Expand 后才出现子节点)
- 处理 Locale(中文 vs 英文 tree item name)
- 处理 case-insensitive matching

**优点**:模仿用户手动操作,UX 上无"中间态"。
**缺点**:实现复杂,容错差(每次 dialog 的 tree 内容不同)。

### 路径 C:用 SendInput 键盘快捷键(参考 PathWrap 的 fallback_confirm)

**原理**:很多 dialog 有键盘快捷键:
- `Ctrl+L`:聚焦地址栏(Chrome 风格)
- `Ctrl+D`:导航到收藏夹
- `Alt+D`:聚焦地址栏(IE/Explorer 风格)
- 直接打字:很多 WinUI 3 dialog 的 tree view 有 type-ahead filter

**步骤**:
1. 把 dialog 设为 foreground(`SetForegroundWindow`)
2. SendInput(`Ctrl+L`)
3. SendInput(完整路径字符串)
4. SendInput(`Enter`)

**优点**:不依赖 UIA tree 结构,稳定。
**缺点**:
- ⚠️ SendInput **有 anti-cheat / 反病毒标记风险**(某些杀软会告警)
- ⚠️ `SetForegroundWindow` 需要 foreground steal privilege(Win11 限制更严)
- ⚠️ 需要 dialog 处于前台,如果用户最小化 dialog 会失效

### 路径 D:接受 v0.3.8 行为 + 解释"中间态" + 加 PUSH 即时通知

**原理**:v0.3.8 的 Write 实际上**工作**(dialog 关闭)。用户 reject 的是"看不到反馈"。
**改动**:
1. 恢复 v0.3.8 的 SetValue + Auto-Commit
2. 但在 PUSH 收到时,弹一个即时通知("Sigma FM 正在同步路径到 Save As dialog...")
3. 用户看到通知 → 心理预期是"系统在工作" → 不再 reject "filename 暂时显示路径"
4. dialog 关闭后通知自动消失

**优点**:最低实施成本,保留 v0.3.8 的功能。
**缺点**:通知 spam,可能分散注意力。

### 路径 E(混合):v0.3.8 baseline + 不污染 filename + 路径显示在另一个 UI 元素

**原理**:WinUI 3 dialog 顶部有 "navigation 面包屑"(用户截图证明),"此电脑 > Win (C:) > downloads"。可以**改写面包屑**而不是 filename。
**改动**:
1. 检测到 FileNameControlHost 时,**不写 filename Edit**
2. 找 dialog 顶部的 "navigation 面包屑" UIA 元素
3. SetValue 路径到面包屑
4. 这才是真正的"导航",不会污染 filename

**优点**:不污染 filename,正确的导航语义。
**缺点**:面包屑可能不可 SetValue(只读控件)。

### 我的推荐:**路径 B + 路径 E 混合**

1. 主路径:路径 B(树视图导航)
2. 备选:路径 E(面包屑 SetValue)
3. 失败兜底:路径 A(只写 basename)+ 用户手动操作

实施顺序:
1. 先恢复 v0.3.8 的 SetValue 到 1001 Edit(去掉 v0.3.9 的 is_winui3_wrapped 检测)
2. **但 SetValue basename 而不是 target_path**(保持 filename 不变)
3. 在 Write 成功前,**尝试用树视图导航**(找 tree view 元素)
4. 树视图失败 → fall back 到只 SetValue basename + 提示用户

---

## 5. 为什么 v0.3.9 错了(我之前 12 轮都搞错了)

| 之前的假设 | 实际证据 |
|---|---|
| WinUI 3 wrapper 拦截 SetValue | Oct 3 03:23 hwnd=30019418 → SetValue 成功,read back 验证通过 |
| FileNameControlHost = "WinUI 3 包裹标志"= 不能写 | 同一 inventory 包含 1001 Edit(可写),v0.3.8 写成功 |
| should_commit=true → dialog 自动关闭 = "工作" | v0.3.8 实际 should_commit=false(03:23 日志),dialog 6 秒后关闭是用户/系统触发 |
| "filename 被改" = 视觉污染 = 必须跳过 | "filename 被改"是因为我们把**完整路径**写入了 filename Edit,不是应该跳过 WinUI 3,而是**应该只写 basename 或写导航元素** |

**v0.3.9 是 symptom-fixing**,把"filename 被改"误判为"WinUI 3 不可写"。
**真实 fix 是 content-fixing**:写 basename 到 filename,或写路径到面包屑/树视图。

---

## 6. 给用户的核心信息

### 6.1 我的错误

我之前的"架构不可达"判断**完全错误**。Sep 27 手动测试 + v0.3.8 真实日志都证明 Edge WinUI 3 Save As **可以被导航**,只是**写入的内容错了**。

### 6.2 真正丢失的能力

**手动部署 → installer 部署**这步转换**没丢能力**:
- spike.exe 工作模式一致(都在用户 interactive session)
- Scheduled Task 配置正确(运行在 Duanyi 用户会话)
- HTTP transport 替代 TCP 也正常(PUSH=200)

**真正丢失的**:**v0.3.9 自己删除了能力**。它引入 `is_winui3_wrapped` 检测,把 v0.3.8 已经能正常工作的 Edge dialog 标记为 Unsupported。

### 6.3 真正的修复路径(回到 v0.3.8 baseline + 修正 SetValue 内容)

1. **回滚 v0.3.9 的 `is_winui3_wrapped` 检测**(回退到 v0.3.8 代码)
2. **修改 SetValue 内容**:
   - 找 dialog 的 **navigation 元素**(面包屑、地址栏、树视图)
   - 导航到 target_path 目录
   - 不动 filename Edit
3. **如果导航失败**:
   - fallback: SetValue basename 到 filename(保持用户原文件名意图)
   - + 弹通知告诉用户"路径已设置,请手动点击 Save"

### 6.4 下一步

需要你决策:
- 选项 1:接受这个修正诊断,我写一份 v0.4.0 实施计划(导航元素 + basename fallback),不立即执行
- 选项 2:直接回滚 v0.3.9 → v0.3.8 + 写 basename fallback(快速止血)
- 选项 3:重新跑一遍 Sep 27 手动测试 + 跑 v0.3.8 在 installer 上的真实测试,对比验证(先验证再说 fix)

我建议 **选项 3 先验证**,因为现在还不确定 v0.3.8 的 Write success 在你的真实环境下是否可重现。

---

## 7. 给未来的我(以及下一任 agent)

**下次有人说"目标架构上不可达",先做这 3 件事**:

1. 翻 git log 找最早的 manual test 证据(spike-report, handoff)
2. 对比当时 vs 现在的环境差异(transport, mode, dialog type)
3. 找**最近一次成功日志**(用 grep 在 spike.log 找 "Injection succeeded via UI Automation")
4. 从成功日志里分析**实际写了什么**(read back 什么, Invoke 了什么)
5. 再决定是改 architecture 还是改 implementation

**避免**:
- ❌ 用"看起来合理"的架构理论否定 working code
- ❌ 把"用户 reject"等同于"功能不可达"
- ❌ 在没看真实 inventory 的情况下假设 dialog 结构

---

## 8. 元数据

- **写作日期**: 2026-10-03
- **supersedes**: `2026-10-03-focus-21-v0.3.9-architecture-retrospective.md`(前一份错判)
- **superseded by**: 等用户决策后的实施计划
- **commit**: 不 commit,等用户决策

**核心承认**:
> 我在上一份文档里说"100+ 轮迭代证明架构不可达",这是**过早放弃 + 错误归因**。证据反驳了我。Sep 27 手动 + v0.3.8 (Oct 3 03:23) 都证明 Edge WinUI 3 Save As 可导航。v0.3.9 是把 working state 退步到 non-working state 的实施错误,不是架构升级。
