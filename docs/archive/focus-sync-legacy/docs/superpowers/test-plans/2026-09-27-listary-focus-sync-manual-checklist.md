# Manual Test Checklist — Listary-style Focus Sync

**日期**: 2026-09-27
**覆盖范围**: Sigma FM fork v2.2.0-focus.1 安装 + spike.exe 联动 + 7-app GUI 验证矩阵
**适用平台**: Windows 11 x64
**前提**: 完成 §0 预检后按顺序执行 §1 → §5

---

## §0 预检(必做,2 min)

### 0.1 SHA256 验证

```powershell
Get-FileHash -Algorithm SHA256 `
  "F:\soft\00selfmade\filemanager\sigma-file-manager\release\Sigma-File-Manager-2.2.0-focus.1-x64-setup.exe"
```

期望:`9da1212b68cb780842dc38a10b126578d6f37b01579d0602c4772d680cdcb545`
若不符:**停**,不要装。

### 0.2 安装目标路径

- 默认:`%LOCALAPPDATA%\Programs\Sigma File Manager` 或 `%LOCALAPPDATA%\Programs\sigma-file-manager`
- 卸载:`%LOCALAPPDATA%\Sigma File Manager\Uninstall Sigma File Manager.exe`(NSIS 生成的 uninstall.exe)

### 0.3 SmartScreen 旁路

首次运行会出现 SmartScreen 警告(未签名 fork):
"Windows protected your PC"
→ "More info" → "Run anyway"
预期 1 次,不再出现。

### 0.4 前置应用清单(本机已装 vs 待装)

| App | 状态 | 影响 |
|---|---|---|
| Chrome | ✓ 已装 | §5 矩阵 app #1 |
| MS Edge | ✓ 已装 | §5 矩阵 app #2 |
| 钉钉 DingTalk | ✓ 已装 | §5 矩阵 app #3 |
| 飞书 Feishu | ✓ 已装 | §5 矩阵 app #4 |
| Word | ✗ **待装** | §5 矩阵 app #5 |
| VS Code | ✗ **待装** | §5 矩阵 app #6 |
| WeChat 微信 | ✗ **待装** | §5 矩阵 app #7 |

⚠️ **装齐 7 个 app 之前,§5 只能跑前 4 个(headless 覆盖率上限 4/7 ≈ 57%,无法走决策 PASS)。**

---

## §1 Sigma FM 安装(5 min)

```powershell
# 双击或:
Start-Process `
  "F:\soft\00selfmade\filemanager\sigma-file-manager\release\Sigma-File-Manager-2.2.0-focus.1-x64-setup.exe"
```

- [ ] NSIS 安装向导出现("Sigma File Manager Setup")
- [ ] 默认勾选 "Add to Start Menu" + "Create Desktop Shortcut" — 保留
- [ ] 点击 Install → 进度条 5-15 秒
- [ ] Finish 出现,**不要**勾 "Launch Sigma File Manager"(先做 §2 sanity)
- [ ] 安装路径下确认 `sigma-file-manager.exe` 存在(42 MB)

---

## §2 App 启动 sanity(2 min)

- [ ] 从开始菜单启动 Sigma File Manager
- [ ] 窗口出现,标题 "Sigma File Manager — [version]"
- [ ] 左侧 folder tree 可见(本 fork 特有)
- [ ] 任意点击 tree 内一个文件夹,主区正确显示内容
- [ ] 关闭应用(Alt+F4 或右上角 X),**无残留进程**:

```powershell
Get-Process -Name "sigma-file-manager" -ErrorAction SilentlyContinue
```

期望:无输出。若有:TaskKill 后报告 bug。

---

## §3 v2.2.0-focus.1 自身 picker 焦点同步验证(老路线,5 min)

⚠️ **这一段验证的是 fork 内置的老 picker 焦点同步(已废弃的 IFileDialog COM 路线),不是 spike。** 仅作为"老方案可工作"的基线参考。Phase 2 spike 路线见 §4 + §5。

### 3.1 Sigma 自己的 file picker 触发

- [ ] 在 Sigma 内按 `Ctrl+O` 或菜单 → File → Open,触发 Sigma 的 file picker dialog
- [ ] **不关闭** picker,Alt-Tab 回到 Sigma 主窗口
- [ ] 在主窗口左侧 tree 里点击一个**不同**的文件夹(例如 `D:\downloads\`)
- [ ] Alt-Tab 回 picker dialog
- [ ] **期望**:picker 的当前目录跳到了新路径
- [ ] **若跳了**:老路线 OK,记录时间戳
- [ ] **若没跳**:记录现象 + 弹窗位置 + tree 点击路径

### 3.2 回归基线(老 picker 仅对 Sigma 自己的 dialog 起效)

- [ ] **不要**期望老路线对外部 app 起效(Chrome / Word / VS Code 等都不在老路线覆盖范围)
- [ ] 老路线 ≠ 全局焦点同步,spike 才是

---

## §4 spike.exe sidecar 启动(spike 路线,Phase 2 prep)

⚠️ **当前 spike 是独立二进制,Sigma FM fork v2.2.0-focus.1 没有内嵌集成它**(NSIS 解包确认无 spike.exe sidecar,无 UIAutomationCore import)。这一段是 **手动模拟** Phase 2 集成效果。

### 4.1 启动 spike

```powershell
# PowerShell 窗口 A(保持开着):
cd F:\soft\00selfmade\filemanager\sigma-listary-spike
.\target\release\spike.exe --port 37421
```

预期 stdout:
```
spike starting on port 37421
TCP server listening port=37421
```

### 4.2 单独 TCP 健康检查(可在任何窗口跑)

```powershell
# 窗口 B:
$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 37421)
$stream = $client.GetStream()
$bytes = [System.Text.Encoding]::UTF8.GetBytes('{"cmd":"get_status"}' + "`n")
$stream.Write($bytes, 0, $bytes.Length)
$stream.Flush()
Start-Sleep -Milliseconds 200
(New-Object System.IO.StreamReader($stream)).ReadLine()
$client.Close()
```

期望响应:`{"ok":true,"current_path":"C:\\","active_dialogs":0}`

### 4.3 set_path 推送(模拟 Sigma 当前路径变化)

```powershell
# 窗口 B:
$client = New-Object System.Net.Sockets.TcpClient("127.0.0.1", 37421)
$stream = $client.GetStream()
$json = '{"cmd":"set_path","path":"D:\\downloads"}' + "`n"
$bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
$stream.Write($bytes, 0, $bytes.Length)
$stream.Flush()
Start-Sleep -Milliseconds 200
(New-Object System.IO.StreamReader($stream)).ReadLine()
$client.Close()
```

期望响应:`{"ok":true}`
spike 窗口 A 应打印 `path_update event ...`

---

## §5 7-app GUI 验证矩阵(spike 路线核心,30-45 min)

⚠️ **必装齐 §0.4 列出的 7 个 app**。否则覆盖不全。

### 5.1 矩阵概览

| # | App | 触发方式 | 期望 |
|---|---|---|---|
| 1 | Chrome | `chrome://downloads` → 任意下载 → "另存为" dialog | spike 检测到 dialog + 写路径 |
| 2 | MS Edge | `edge://downloads` → 任意下载 → "Save as" | spike 检测 + 写路径 |
| 3 | 钉钉 DingTalk | 联系人 → 发送文件 → 选择本地文件 dialog | spike 检测 + 写路径 |
| 4 | 飞书 Feishu | 消息 → 发送文件 → 选择本地文件 | spike 检测 + 写路径 |
| 5 | Word | 文件 → 打开 → "打开" dialog | spike 检测 + 写路径 |
| 6 | VS Code | File → Open Folder → "打开文件夹" dialog | spike 检测 + 写路径 |
| 7 | 微信 WeChat | 发送文件 → 选择本地文件 | spike 检测 + 写路径 |

### 5.2 每 app 通用流程(每个 5 min)

1. **打开 spike(若未开)**:见 §4.1
2. **启动目标 app**(已装 + 登录)
3. **触发该 app 的"打开 / 另存为" dialog**(见 5.1)
4. **dialog 不要关**,切到 spike 窗口 A 看 stdout
5. **期望 stdout 出现**:
   ```
   DialogDetected hwnd=<6位数字> app=<classname>
   ```
   - hwnd 是真实窗口句柄(每次启动会变)
   - app 应是 `#32770`(Win32 标准 dialog)或 `Chrome_WidgetWin_*`(Chromium 类)
6. **手动 set_path**(模拟 Sigma 当前路径变化):
   ```powershell
   # 窗口 B:
   # (用 §4.3 的模板,改 path)
   '{"cmd":"set_path","path":"D:\\downloads"}'
   ```
7. **dialog 应在 100-500ms 内跳到 `D:\downloads`**
8. **记录**:
   - ✅/❌ 是否检测到
   - ✅/❌ 是否跳转
   - 跳转延迟(从 set_path 到 dialog 实际跳转)
   - stderr / 异常(若有)

### 5.3 Win32 vs Chromium 区分

- **Win32 (#32770)**:经典文件 dialog,绝大多数 Windows app。spike 直接打 InvokePattern 点 OK + SetValue 写路径。
- **Chromium 类**:`Chrome_WidgetWin_*` 类名。spike 用 fg_bypass(EnableWindow FALSE)屏蔽 Chromium 自抢焦点,实测 Chromium 7/8 → 0 z-drops。
- **UWP / XAML**(本矩阵不含):spike 当前简化掉了 UWP 分支,若遇到 Excel / Photos 之类 UWP app 的 dialog 可能不工作 — **记录下来但不算 FAIL**。

### 5.4 决策矩阵

| 通过率 | 决策 |
|---|---|
| 7/7 (100%) | spike PASS,进入 Phase 2 |
| 5-6/7 (≥70%) | spike PASS 但有限制(UWP / XAML 等),记录限制 + 进 Phase 2 |
| 4/7 (57%) | **FAIL**,回到 spike 路线优化(最常见失败原因:UWP 分支缺失、classname 误检) |
| ≤3/7 | **严重 FAIL**,可能需要重写 detector 或回退到 PathWrap / QwenLM 完整移植 |

---

## §6 故障排查清单(若 FAIL)

### 6.1 spike 检测不到 dialog

- [ ] 检查 dialog classname(用 Spy++ 或 AutoIt Window Info)
- [ ] 是否用了 `SetWinEventHook` 范围之外的 dialog(MFC 自绘? WPF 自绘?)
- [ ] `check-installed-apps.ps1` 是否能识别 app 已启动
- [ ] 重新跑 `verify_target_matrix.ps1 -Manual` 看 headless 输出

### 6.2 检测到但跳不转

- [ ] dialog 当前路径控件是 ComboBoxEx 还是 DirectUI(用 UIA Verify 工具)
- [ ] InvokePattern 是否可调用(部分 app 禁用)
- [ ] fg_bypass 是否被触发(Chromium app 的 hwnd 应在 RAII guard 包裹下)

### 6.3 spike 自身报错

- [ ] spike stderr 文件:`C:\Users\Duanyi\AppData\Local\Temp\spike-smoke-*.log.err`
- [ ] 端口冲突:`netstat -ano | findstr 37421`(理论上 Phase 2 会切 named pipe 解决)
- [ ] STA 线程:spike 用 single-thread tokio 维持 COM STA,不要改 main.rs

---

## §7 验证完成后的动作

- [ ] 填好 §5 矩阵表格(贴到下一个 handoff)
- [ ] 若 spike PASS:进入 Phase 2 spec 实施(见 `docs/superpowers/specs/2026-09-XX-listary-focus-sync-integration-design.md`,待写)
- [ ] 若 spike FAIL:回退到 spike 路线优化,不要 push 当前 README commit
- [ ] push README commit(commit hash `54186f87` 等验证完成后):`git push origin main`(在 fork repo 内)

---

## 引用

- Spike 源码:`F:\soft\00selfmade\filemanager\sigma-listary-spike\`
- 决策文档:`docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`
- Spike 报告:`docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md`
- Fork release README:`sigma-file-manager\release\README.md`