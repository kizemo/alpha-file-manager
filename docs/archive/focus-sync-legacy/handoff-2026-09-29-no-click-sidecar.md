---
name: 2026-09-29 handoff (round 14) - No-click sidecar rebuilt from source
description: Round 14. Sidecar was auto-clicking Save/Open button after setting path. Rebuilt sidecar from source with click block removed. Only sets path, no auto-confirm.
type: project
---

# Handoff — No-Click Sidecar Rebuilt (2026-09-29 round 14)

**Inherits**: `handoff-2026-09-29-path-normalize.md` (round 13 — path normalize fixed)

**User reported**: "切换回对话框,会直接启动下载" — sidecar was setting path AND clicking Save/Open button, triggering immediate download.

## 一、Root cause

`kizemo/focus-sync` 源码 `sidecar/src/uia_inject.rs:122-129`:

```rust
let edit = best_edit.ok_or_else(|| Error::from(E_FAIL))?;
let value: IUIAutomationValuePattern = unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
unsafe { value.SetValue(&BSTR::from(target_path))? };  // ← 改路径

// Prefer clicking the default button; fall back to sending Enter to the filename box.
match best_button {
    Some(button) if best_button_score > 0 => {
        let invoke: IUIAutomationInvokePattern =
            unsafe { button.GetCurrentPatternAs(UIA_InvokePatternId)? };
        unsafe { invoke.Invoke()? };  // ← 这里点 Save/Open!
    }
    _ => fallback_confirm(&edit),  // ← 或发 Enter
}
```

Original kizemo.focus-sync 设计就是:改路径后自动点 Save。这是 Listary 的核心 UX。但用户的预期不同:只改路径,不要点。

## 二、修复

### 2.1 修改 `uia_inject.rs`

把 line 122-129 替换成只设路径,跳过点击:

```rust
let edit = best_edit.ok_or_else(|| Error::from(E_FAIL))?;
let value: IUIAutomationValuePattern = unsafe { edit.GetCurrentPatternAs(UIA_ValuePatternId)? };
unsafe { value.SetValue(&BSTR::from(target_path))? };

// Path-only mode (per user request 2026-09-29): do NOT click Save/Open
// and do NOT send Enter to the filename box. Just update the path; let
// the user manually press Save/Open when ready.
info!("Path injected; skipping auto-confirm.");

Ok(())
}
```

### 2.2 从 GitHub 拉源码 + 用 Rust 1.90 编译

```bash
git clone --depth=1 https://github.com/kizemo/focus-sync.git C:\Temp\focus-sync-src
cd C:\Temp\focus-sync-src\sidecar
# 编辑 src/uia_inject.rs (line 122-129)
cargo build --release
```

工具链:`C:\Users\Duanyi\.rustup\toolchains\1.90.0-x86_64-pc-windows-msvc\bin`
镜像:`RUSTUP_DIST_SERVER=https://rsproxy.cn`(加速)

编译时间:~46 秒,产出 `focus-sync-sidecar.exe` (3,068,416 bytes)。

### 2.3 替换已部署的 binary

| 操作 | 路径 |
|---|---|
| Backup 原始 | `C:\Temp\focus-sync-sidecar-original.exe` |
| Deploy | `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe` |
| 更新 installer staging | `release\extension\bin\focus-sync-sidecar.exe` |
| 重新编译 installer | `release\kizemo.focus-sync-0.2.0-setup.exe` (1,010,744 bytes) |

原 binary sha256: `98b7b180907ad1b1f87b30bfab55c9fad8a47f4e67bdca201145e81b9749b848` (原 manifest 写错了)
新 binary 需更新 manifest 的 integrity hash

## 三、当前状态

| Item | Value |
|---|---|
| Sigma FM PID | 38676 |
| Sidecar PID | 已 killed(被新 binary 替换前) |
| Port 37421 | 当前空闲(等 Sigma FM 重启后激活) |
| dist/index.js | normalizePath 已加(round 13) |
| Sidecar binary | no-click variant, deployed to %APPDATA% |
| Installer | rebuilt 1,010,744 bytes (含新 sidecar) |

## 四、用户验证步骤

1. **关闭 Sigma FM**(因为 sidecar binary 文件变了)
2. **重新打开 Sigma FM** — extension 重新加载时 spawn 新 sidecar
3. **Edge 打开下载对话框**
4. **切到 Sigma FM 改目录**
5. **切回 Edge 对话框** — 路径应更新,**但不会自动下载**(没有点击)
6. **手动点 Save/Open** 来确认下载

如果用户仍然看到自动点击,可能:
- 还有旧 sidecar 在跑(用 `Get-Process focus-sync-sidecar` 检查并 kill)
- JS 里其他逻辑触发了点击(应没有,但可以查)

## 五、技术沉淀

### 5.1 Sidecar binary 修改路径

源码位置: https://github.com/kizemo/focus-sync
关键文件:`sidecar/src/uia_inject.rs`
点击逻辑:line 122-129 (`InvokePattern::Invoke` + fallback `SendMessageW(WM_KEYDOWN, VK_RETURN)`)

修改 + 编译需要 Rust 工具链。Sigma FM 已经在用 Tauri (Rust) 工具链,所以可共用。

### 5.2 Listary-style focus sync 的设计意图

原始设计:
1. 设置路径(`ValuePattern::SetValue`)
2. 自动点击 Save/Open(`InvokePattern::Invoke`)或发 Enter(`SendMessageW`)
3. **目标**:让焦点切换到对话框时,**直接完成下载**,无需用户确认

用户场景与此设计冲突:用户希望**先预览路径再确认**,所以只改路径不点击。

### 5.3 Sidecar manifest integrity mismatch

Round 1 写的 manifest 中 sidecar sha256 (`ca0107da...`) 与实际不符(`98b7b180...`)。这个 mismatch 不影响本地安装(marketplace 下载才校验),但应该更新。下次 installer rebuild 会自动包入新 binary,但 manifest 仍是旧 hash。

## 六、本轮改动文件清单

```
NEW  C:\Temp\focus-sync-src\                          (cloned kizemo/focus-sync source)
NEW  C:\Temp\focus-sync-sidecar-original.exe          (backup of pre-rebuild binary)
MOD  C:\Temp\focus-sync-src\sidecar\src\uia_inject.rs  (removed click + fallback_confirm)
NEW  C:\Temp\focus-sync-src\sidecar\target\release\focus-sync-sidecar.exe  (rebuilt)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe  (deployed no-click)
MOD  release\extension\bin\focus-sync-sidecar.exe     (staging)
MOD  release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (rebuilt 1,010,744 bytes)
MOD  release\kizemo.focus-sync-0.2.0-setup.exe       (top-level copy)
```

## 七、新会话第一句话参考

> 承接 `handoff-2026-09-29-no-click-sidecar.md`(round 14)。Sidecar 从 GitHub 拉源码 + 编译,改 `uia_inject.rs` 去掉 `InvokePattern::Invoke` 和 `SendMessageW(Enter)`,只保留 `ValuePattern::SetValue`(改路径不点击)。新 binary 已 deploy 到 %APPDATA%,installer 也重建含新 binary。用户需:关 Sigma FM → 重开 → 测 Edge 下载对话框 → 应该路径更新但不自动下载。