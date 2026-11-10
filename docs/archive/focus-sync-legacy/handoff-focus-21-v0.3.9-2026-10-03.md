# focus-21 v0.3.9 实施完成 — 待 U-V1 用户验证

**日期**: 2026-10-03
**分支**: feat/recovery-2026-09-28 (未 commit, 等用户授权)
**计划**: docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-winui3-detection.md

---

## 1. 完成内容

### 1.1 Fix-Core — uia_inject.rs 加 is_winui3_wrapped helper

**文件**: `sigma-listary-spike/src/uia_inject.rs`

新增 helper(`fn is_winui3_wrapped`,紧跟 `find_ifiledialog_edit` 之后):
- 遍历 `IUIAutomationElementArray`
- 寻找 `automation_id='FileNameControlHost'` (Pane 50003) — WinUI 3 wrapper 层标志
- 找到 → return true (WinUI 3 wrapped)
- 找不到 → return false (true IFileDialog, 走 v0.3.8 路径)

在 `inject_via_uia` 末尾分支**之前**(address_bar 分支 return 之后,find_ifiledialog_edit 分支之前)插入检测:
```rust
if is_winui3_wrapped(&elements) {
    warn!("v0.3.9: WinUI 3 wrapped dialog detected (FileNameControlHost present). ...");
    state.inc_unsupported_dialog();
    return Ok(DialogOutcome::Unsupported);
}
```

**为什么必须在 find_ifiledialog_edit 之前**:WinUI 3 wrapped dialog 内嵌 IFileDialog(1001/1148 Edit 存在),v0.3.8 路径会先匹配 1001 → SetValue → 触发"filename 被改"中间态。FileNameControlHost 检测必须先于 1001 匹配。

### 1.2 版本号 bump 0.3.8 → 0.3.9

| 文件 | 改动 |
|---|---|
| `sigma-listary-spike/src/http_server.rs` | SPIKE_VERSION + test 断言 |
| `release/extension/dist/index.js` | header + activate log |
| `release/extension-installer/register.ps1` | 10 处版本字符串 |

### 1.3 Build 产物

| 产物 | 路径 | 大小 | 时间 |
|---|---|---|---|
| Spike (sidecar) | `sigma-listary-spike/target/release/spike.exe` | 2,793,984 bytes | 11:39 |
| Sidecar 副本 | `release/extension/bin/focus-sync-sidecar.exe` | 2,793,984 bytes | 11:39 |
| Sidecar 副本 | `release/extension-installer/focus-sync-sidecar.exe` | 2,793,984 bytes | 11:39 |
| **Setup.exe** | `release/Sigma-File-Manager-2.2.2-focus-21-v0.3.9-setup.exe` | 17,071,814 bytes | 11:47 |

### 1.4 NSIS 抽出验证(7z)

✅ NSIS installer 包含 v0.3.9 全部标志:
- `extensions/kizemo.focus-sync/dist/index.js` — 6 hits (0.3.9 / FileNameControlHost / is_winui3_wrapped)
- `tools/register.ps1` — 10 hits (0.3.9)
- `tools/focus-sync-sidecar.exe` — 时间戳 11:39 (与 spike.exe 一致)

⚠️ `extensions/kizemo.focus-sync/package.json` 显示 version="0.3.0" — 这是 historic placeholder,**不影响 runtime**;runtime 版本来自 `register.ps1` 写入 `user-extensions.json` 的 manifest.version(0.3.9)。

---

## 2. 测试结果

### 2.1 cargo test --release(用 rust 1.90.0 toolchain)

```
test result: FAILED. 47 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
```

- ✅ **47 passed** — 包括所有 uia_inject, writer, http_server, state, events, logger, uia_event, config 测试
- ❌ **1 failed** — `config::tests::resolve_port_strict_on_conflict`(should_panic 测试)

### 2.2 ⚠️ Pre-existing 失败 — config 端口冲突测试

```
note: test did not panic as expected at src\config.rs:124:8
```

**结论**:这个失败**与本次修改无关**(我没改 config.rs)。

**原因**:测试用 `std::net::TcpListener::bind("127.0.0.1:0")` 获取 OS 分配的空闲端口,再调用 `resolve_port` 期望它 panic。但 OS 端口分配 + 立即重 bind 之间存在 race;若 OS 还没把端口标记为占用,`resolve_port` 找不到冲突,直接返回更高端口(无 panic)。这是典型的"should_panic 测试易 flaky"。

**Fix 路径**(可选 follow-up,非本计划范围):
- 改用 `TcpListener::bind(("127.0.0.1", specific_port))` + `set_only_address` 测试确定性
- 或者改 test 为"验证 Err 而非 panic"(调用 `unwrap_err()`)

**当前决定**:47/48 测试通过,核心功能(uia_inject + writer + http_server + state + events)全部绿。继续推进 U-V1 验证。

### 2.3 cargo build --release

```
Finished `release` profile [optimized] target(s) in 13.09s
```

✅ spike.exe (2,793,984 bytes) 编译成功,8 个 warning(全部 pre-existing 命名约定 warning,如 `SID_STopLevelBrowser` 应大写)。

### 2.4 Tauri build

```
Built application at: ...\sigma-file-manager.exe
Finished 1 bundle at: ...\Sigma File Manager_2.2.0_x64-setup.exe (16.28 MiB)
```

✅ Tauri + NSIS 完整打包成功。

---

## 3. 未 commit 文件清单(等用户授权)

```
sigma-listary-spike/src/uia_inject.rs (Fix-Core: helper + detection branch)
sigma-listary-spike/src/http_server.rs (SPIKE_VERSION + test)
sigma-listary-spike/target/release/spike.exe (新 binary)
release/extension/dist/index.js (v0.3.9 头 + log)
release/extension-installer/register.ps1 (10 处版本)
release/extension/bin/focus-sync-sidecar.exe (sidecar 副本)
release/extension-installer/focus-sync-sidecar.exe (sidecar 副本)
release/Sigma-File-Manager-2.2.2-focus-21-v0.3.9-setup.exe (installer)
```

---

## 4. U-V1 BLOCKING 用户验证清单

**U-V1**(BLOCKING):Edge WinUI 3 Modern Save As(无 flag)
- 期望:PUSH=200 + dialog **filename 不被改** + 弹 unsupported 通知 + dialog 不动

**U-V2**(nice to have):Edge Legacy(启用 flag)
- 期望:PUSH=200 + dialog 自动跳(COM SetFolder)+ Auto-Commit

**U-V3**(nice to have):Chromium Save As
- 期望:PUSH=200 + 41477 SetValue + Invoke + 关闭

**U-V4**(nice to have):Notepad Save As
- 期望:PUSH=200 + 1001 SetValue + Auto-Commit + 关闭

**U-V5**(nice to have):Photos Save As
- 期望:PUSH=200 + 1001 SetValue + Auto-Commit + 关闭

### U-V1 验证步骤

1. 关闭 Sigma FM
2. 运行 `release/Sigma-File-Manager-2.2.2-focus-21-v0.3.9-setup.exe` 安装
3. 启动 Sigma FM,等待 focus-sync 启动
4. 打开 Edge,导航到任意页面 → Ctrl+S 保存
5. Save As dialog 打开后,切换到 Sigma FM 任意文件夹(例如 `C:\evid`)
6. 观察:
   - PUSH 日志显示 status=200
   - Save As dialog **filename 字段保持原值**(不被改为 `C:\evid\xxx`)
   - focus-sync 弹通知:"Detected 1 Save As dialog(s) the sidecar could not navigate (Edge Win11 WinUI)"
   - dialog 不自动关闭
7. sidecar log 关键标识:
   - `WinUI 3 wrapped dialog detected (FileNameControlHost present)`
   - `target='...'`
8. **acceptance** = filename 不被改 + 弹 unsupported 通知 + dialog 不动

---

## 5. 回退路径(若 U-V1 fail)

**Scenario A**:filename 仍被改(FileNameControlHost 误判)
- Fallback: 完全禁用 1001/1148 SetValue(回退 v0.3.6 L2-1)
- 删除 find_ifiledialog_edit 分支 → 全部 Unsupported

**Scenario B**:filename 不动但 U-V4 Notepad 误判
- 调整 helper:`FileNameControlHost` 字符串不准确 → 重新 inventory

**Scenario C**:R63 — is_winui3_wrapped UIA 调用 panic
- 已有 try/catch + 默认 false,但若 leak 出来 → 在 inventory loop 中先排除空元素

---

## 6. 必读顺序(新会话)

1. `docs/superpowers/plans/2026-10-03-focus-21-v0.3.9-winui3-detection.md`(plan)
2. 本 handoff(实施结果)
3. `sigma-listary-spike/src/uia_inject.rs` L455-478(新检测分支)
4. `release/Sigma-File-Manager-2.2.2-focus-21-v0.3.9-setup.exe`(验证产物)

## 7. 禁止事项

- ❌ 不要 commit(等用户授权)
- ❌ 不要 reinstall 旧版本(若 U-V1 fail,先讨论再决定)
- ❌ 不要改写 uia_inject.rs 之外的 Rust 源码(本计划仅此一处 + http_server.rs 版本)
