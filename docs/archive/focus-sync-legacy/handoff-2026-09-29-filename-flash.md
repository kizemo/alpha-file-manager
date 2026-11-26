---
name: 2026-09-29 handoff (round 17) - Sidecar writes filename box, causes flash loop + focus grab
description: Round 17. Sidecar's UIA SetValue writes to filename input box (AutomationId 1148). Browser resets filename + sidecar rewrites = flash loop. Plus fg_bypass::run_with_bypass steals focus. Path: switch to IFileDialog::SetFolder via COM.
type: project
---

# Handoff — Sidecar Filename Flash Loop (2026-09-29 round 17)

**Inherits**: `handoff-2026-09-29-loop-fix.md` (round 16 — sidecar stays alive)

**User reported**: "在弹出菜单中,文件名处反复在 C: 和 sigma 文件夹路径之间闪动切换,无法固定。且争夺 windows 置顶焦点,无法切换其他软件。路径应当是在地址栏,才能实现地址切换,不是在文件名,不应当一直反复修改。"

**Translation**:
- Edge Save dialog: 文件名栏 在 `C:\path\` 和 Sigma 当前目录 之间反复切换闪动
- Sidecar 抢 windows 顶层焦点,用户无法切到其他软件
- 路径应该在**地址栏**,不是**文件名**栏
- 不应该一直反复修改(应该一次性设置)

## 一、根因

`sidecar/src/uia_inject.rs:117-119` 把 path 写到 filename 输入框(AutomationId `1148` = IFileDialog 的 ComboBoxEx32):

```rust
let edit = best_edit.ok_or_else(|| Error::from(E_FAIL))?;
let value: IUIAutomationValuePattern = unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
unsafe { value.SetValue(&BSTR::from(target_path))? };
```

`filename_edit_score` 总是优先 `1148`(IFileDialog filename) = 100 分,即使 path 应写**地址栏**。

写 filename 触发循环:
1. Sidecar 写 `D:\Downloads` 到 filename 字段
2. Edge IFileDialog 解析 filename = `D:\Downloads` → 跳转到该目录 → 触发 folder change 事件
3. Sigma FM onPathChange 触发新一轮 path 推送
4. Sidecar 又写新 path 到 filename → 循环闪动

`fg_bypass::run_with_bypass` 包装 UIA 调用,用 `EnableWindow(FALSE)` 屏蔽 Chromium 焦点。这反复 disable/enable,可能让 Sigma FM 失去顶层焦点。

## 二、修复方案(下一会话要做)

### 2.1 优先方案:用 `IFileDialog::SetFolder` 设目录(不写 filename)

这是 Windows IFileDialog 官方 API,只设当前目录不修改 filename:

```rust
use windows::Win32::UI::Shell::{IFileDialog, IShellItem};
use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_INPROC_SERVER};

// Get IFileDialog from HWND (用 SendMessage(WM_GETISHELLBROWSER) 或 IServiceProvider)
let ifile_dialog: IFileDialog = ...;  // 从 dialog_hwnd 拿

// Path → IShellItem
let shell_item: IShellItem = path_to_shell_item(target_path)?;

// Set folder (写路径,不触发 filename 字段)
unsafe { ifile_dialog.SetFolder(&shell_item)?; }
```

这需要:
1. `IFileDialog`/`IShellItem` 已 available 在 `windows-rs`
2. Path → IShellItem 转换(用 `SHCreateItemFromParsingName`)
3. 从 dialog HWND 拿到 IFileDialog(用 SendMessage + IServiceProvider::QueryService)

### 2.2 简化方案(过渡):写 filename 加 `\` + 去重

如果 COM 太复杂,可用快速 hack:
- path 末尾加 `\`(让 dialog 当成目录)
- 跳过写入 cwd 内容到 filename(避免重复)

```rust
let mut target = target_path.to_string();
if !target.ends_with('\\') && !target.ends_with('/') {
    target.push('\\');
}
unsafe { value.SetValue(&BSTR::from(&target))?; }
```

但这不解决循环,只是减小触发概率。

### 2.3 改写 sidebar 的面包屑(breadcrumb)或地址栏

Edge Chromium Save dialog 内部有 IShellView,可能暴露面包屑。UIA 可以枚举 toolbar 找 ToolbarWindow32 class。

比 SetValue 复杂,但避免 filename 副作用。

## 三、当前状态

| Item | Value |
|---|---|
| Sigma FM PID | 32640(22:10:25 启动) |
| Sidecar | 跑(loop-fix 版),但 SetValue 仍写 filename |
| Installer | 含 no-click + loop-fix 二进制 |

## 四、待修源码位置

`sidecar/src/uia_inject.rs`:
- `fn inject_folder_path` line 39-54 — 入口函数
- `fn inject_via_uia` line 84-127 — 写入逻辑
- `fn filename_edit_score` line 134-150 — 文件名输入框优先级最高
- `fn confirm_button_score` line 152-167 — 不需要改动
- `fn fallback_confirm` line 181-194 — 未使用,可删

`sidecar/src/writer.rs:19-36` — 调用 inject_folder_path,可改成:
```rust
crate::fg_bypass::run_with_bypass(hwnd_u32, || {
    crate::i_file_dialog::set_folder(hwnd_u32, &target);
});
```

新增 `sidecar/src/i_file_dialog.rs` — COM IFileDialog wrapper。

## 五、Cargo.toml 依赖

`windows = { version = "0.62", features = ["Win32_UI_Shell"] }` — 已支持 IFileDialog/IShellItem。

## 六、关键技术沉淀

### 6.1 IFileDialog 是 COM 接口

从 HWND 拿 IFileDialog:
```rust
let mut pv_obj = std::ptr::null_mut();
SendMessageW(hwnd, WM_GETOBJECT, 0, OBJID_NATIVEOM as _);
// 然后 IServiceProvider::QueryService(SID_STopLevelBrowser, IID_IFileDialog, &mut pv_obj)
```

更简单的:用 `IShellItemImageFactory` 反向路径解析。

### 6.2 Path → IShellItem

```rust
use windows::Win32::UI::Shell::SHCreateItemFromParsingName;
let mut psi = None;
unsafe {
    SHCreateItemFromParsingName(
        PCWSTR(path_wide.as_ptr()),
        None,
        windows::Win32::UI::Shell::SHCreateItemParsingNameAttributes::default(),
        &mut psi,
    )?;
}
```

### 6.3 SetFolder 调用

```rust
unsafe {
    let ifile_dialog: IFileDialog = psi.query_interface()?;  // from IServiceProvider
    ifile_dialog.SetFolder(&psi_folder)?;
}
```

### 6.4 去掉 fg_bypass::run_with_bypass

不需要 disable Chromium — SetFolder 不触发焦点切换。原本 fg_bypass 是为了防止 InvokePattern::Invoke 的焦点抢夺(我们已去掉 click)。所以可以撤掉 fg_bypass wrapper。

## 七、临时方案(用户测过可采用)

如果下一会话无法做完整 IFileDialog SetFolder,简单方案:
1. 在 uia_inject.rs SetValue 前 path 末尾加 `\`
2. 加去重:track 上次写的 path,如果 path 相同则不写
3. 撤掉 fg_bypass wrapper(因为 click 已去掉)

## 八、本轮交付物清单

```
MOD  C:\Temp\focus-sync-src\sidecar\src\main.rs  (已加 loop {} — round 16)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe  (loop-fix deployed)
MOD  F:\soft\00selfmade\filemanager\release\extension\bin\focus-sync-sidecar.exe  (staging)
MOD  F:\soft\00selfmade\filemanager\release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (rebuilt 1,010,113 bytes)
MOD  F:\soft\00selfmade\filemanager\release\kizemo.focus-sync-0.2.0-setup.exe  (top-level)
```

无新改动(这轮只是诊断问题,等下一会话做 IFileDialog::SetFolder)。

## 九、新会话 prompt(写在 `prompt-next.txt`)

见同目录 `prompt-next-sidecar-fix.txt`。