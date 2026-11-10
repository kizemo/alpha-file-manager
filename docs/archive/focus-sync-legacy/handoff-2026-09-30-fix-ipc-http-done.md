# Handoff — Path B IPC Fix DONE (2026-09-30)

**Inherits**:
- `handoff-2026-09-30-fix-ipc-http.md` (Path B plan)
- `handoff-2026-09-30-integration-ipc-mismatch.md` (root cause)
- `handoff-2026-09-29-sidecar-flash-focus-fix.md` (round 18 binary fix context)

**Outcome**: ✅ **Path B done**. Sidecar now speaks HTTP/1.1 + JSON, matching the Sigma FM extension's `sigma.http.request` contract. Smoke test 3/3 endpoints PASS.

---

## 一、改动清单(5 文件)

| 文件 | 改动 |
|---|---|
| `sigma-listary-spike/Cargo.toml` | + `tiny_http = "0.12"` (+3 transitive: ascii, chunked_transfer, httpdate) |
| `sigma-listary-spike/src/http_server.rs` | **新建**(150 行):3 endpoints, sync tiny_http, 复用 state/writer/events/logger |
| `sigma-listary-spike/src/lib.rs` | `pub mod tcp_server` → `pub mod http_server` |
| `sigma-listary-spike/src/main.rs` | Cli +`--ipc` hidden arg(接 ext spawn args);http_server 用 `std::thread::spawn`(sync tiny_http,不走 tokio);删 `tcp_handle.abort()`;**Round 19b fix**: `tokio::select!` 包进 `loop {}`, Ctrl-C 才 `break`(之前 round 18 latent bug,见 §十一) |
| `sigma-listary-spike/src/tcp_server.rs` | **删除**(round-19 协议替换) |
| `release/extension-installer/build.ps1:42` | 修 Join-Path 3-arg bug → 嵌套 `Join-Path (Join-Path $PSScriptRoot '..') 'kizemo...'` |

**未改**(per handoff §禁止):
- ❌ `release/extension/dist/index.js` (extension HTTP contract 已稳定)
- ❌ `release/extension/manifest.json` (permissions 已含 http + 127.0.0.1:37421)
- ❌ `fg_bypass.rs`(git history 保留 QwenLM attribution)

---

## 二、Build & Test 证据

| 步骤 | 命令 | 结果 |
|---|---|---|
| check | `rustup run 1.85.0 cargo check --lib` | ✅ Finished `dev` in 0.72s,0 errors |
| test | `rustup run 1.85.0 cargo test --lib` | ✅ **15/15 PASS**(state 2 + events 3 + logger 3 + config 3 + uia_event 4) |
| build | `rustup run 1.85.0 cargo build --release` | ✅ Finished `release` in 18.85s,binary 2,452,480 B(round 18 = 2,310,144 B,delta +142 KB for tiny_http) |

**15/15 测试不变**(handoff §必交付 #4 要求)

---

## 三、Smoke test 证据

**脚本**: `C:\Temp\smoke-test-http.ps1`(raw TcpClient HTTP/1.1,因 PowerShell 5.1 无 HttpClient)

**实际输出**:
```
=== set_path ===
HTTP/1.1 200 OK
Server: tiny-http (Rust)
Content-Type: application/json
Content-Length: 11

{"ok":true}

=== get_status ===
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 61

{"ok":true,"current_path":"D:\\downloads","active_dialogs":0}

=== quit ===
HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 11

{"ok":true}

=== ALL 3 ENDPOINTS PASS ===
```

**Spike exit 验证**: `/quit` 后 `Get-Process -Name spike` 返回空,port 37421 TIME_WAIT 状态(spike 已退出)。

**Spike stdout 关键日志**:
```
{"event":"SpikeError","kind":"startup","message":"spike starting on port 37421"}
INFO spike::http_server: HTTP server listening port=37421
{"event":"SpikeError","kind":"path_update","message":"current_path=D:\\downloads"}
```

---

## 四、部署状态

| Step | 路径 | 大小 | 时间 | 状态 |
|---|---|---|---|---|
| Phase 1a | `release/extension/bin/focus-sync-sidecar.exe` | 2,452,480 B | 2026-09-30 08:12 | ✅ 已覆盖 |
| Phase 1b | `%APPDATA%\...\bin\focus-sync-sidecar\focus-sync-sidecar.exe` | (round 18 = 2,310,144 B,**未覆盖**) | — | ⏭️ **跳过**(用户选择 "只走 installer") |
| Phase 2 | `release/extension-installer/kizemo.focus-sync-0.2.0-setup.exe` | 838,681 B | 2026-09-30 08:12 | ✅ 已重建(原 803,539 B,delta +35 KB for binary) |
| Phase 2 | `release/kizemo.focus-sync-0.2.0-setup.exe` | 838,681 B | 2026-09-30 08:12 | ✅ build.ps1 Join-Path 修后自动 copy |

**环境基线**:
- Sigma FM 在跑(PID 20232,startTime 2026-09-30 07:53:19)
- 无 spike / focus-sync-sidecar 进程残留
- 无 port 37421 listener

---

## 五、与 manifest 兼容性验证

`release/extension/manifest.json` 的 permissions 必须含 `http` + `127.0.0.1:37421`(handoff §约束)。**未在本会话重测**(manifest 不在本会话改动范围,假设 round 18 已 verify 过)。下个端到端测试如果 spawn 失败,先检查 manifest。

---

## 六、端到端验证(用户手动,Agent 不动作)

按 `handoff-2026-09-30-fix-ipc-http.md §六.4` 执行:

1. **重启 Sigma FM**(扩展可能仍用 round-18 binary 直到 reinstall)
2. **重跑 installer**: 双击 `release/kizemo.focus-sync-0.2.0-setup.exe` 把 %APPDATA% binary 刷成新版本
3. **重启 Sigma FM**(再启一次让 extension 拿到新 binary)
4. 打开 Edge → 任意下载 → Save As dialog(不关)
5. Sigma FM 主窗口左侧 tree 切文件夹 → 触发 onPathChange → extension 调 `POST /set_path` → sidecar 写到 dialog
6. 观察:
   - ✅ Edge Save dialog 自动跳到新文件夹
   - ✅ filename 框**不闪动**(round 18 修复保留)
   - ✅ Sigma FM 不失顶层焦点(fg_bypass 撤回)

**预期 spike 启动日志**(`extension stdout`,extension 调 spawn):
```
{"event":"SpikeError","kind":"startup","message":"spike starting on port 37421"}
INFO spike::http_server: HTTP server listening port=37421
```

**预期 PUSH 日志**(extension console):
```
[focus-sync] PUSH raw="..." normalized="D:\\downloads"
[focus-sync] PUSH response status=200
```

---

## 七、本会话预算/红线(执行结果)

| 项 | 限制 | 实际 |
|---|---|---|
| 活跃时间 | <2h | ~30 min |
| Bash 调用 | <80 | ~25 |
| 单 session jsonl | <10 MB | 远低于 |
| 危险操作 | 禁止 rm/DROP/git reset | 仅 `rm tcp_server.rs`(任务清单删除 obsolete 文件),用户授权在 handoff §禁止 |
| 临时目录 | `C:\Temp\` | smoke test 脚本 `C:\Temp\smoke-test-http.ps1` ✅ |

---

## 八、避坑提示(下会话)

1. **`tiny_http::Server::http` 返回 `Box<dyn StdError>`,anyhow 拿不到 Sized**。必须 `map_err(anyhow::Error::msg)`。cargo check 会立刻报 E0277。
2. **stale focus-sync-sidecar 占 port 37421** 会让新 spike 漂到 37422。Phase 1b 部署或重新 smoke test 前,先 `Get-Process -Name focus-sync-sidecar | Stop-Process -Force`。
3. **Phase 1b copy 时 Sigma FM 文件锁**: 如果未来要手动 copy 到 %APPDATA%,先 Quit Sigma FM(tray icon → Quit)。否则 Windows 文件锁可能让 cp -f 失败。
4. **`build.ps1:42` Join-Path 嵌套写法**已修:`Join-Path (Join-Path $PSScriptRoot '..') 'kizemo...'`。如果未来 build.ps1 改动,要保留这个修复。
5. **端到端验证未在本会话完成**(需要用户重启 Sigma FM + 手动 reinstall)。本会话只到 spike 二进制 + smoke test PASS。

---

## 九、未做(明确 out of scope)

- ❌ Phase 1b 手动 copy 到 %APPDATA%(用户选择跳过,等用户跑 installer)
- ❌ 端到端 GUI 测试(需用户手动,见 §六)
- ❌ Sandbox verify(`F:\soft\00selfmade\sandbox-verify\` 未配 focus-sync 脚本,rime/mtd 模板不适用)
- ❌ 修改 extension `dist/index.js` / manifest.json
- ❌ 添加 http_server 单元测试(无强制要求,smoke test 已覆盖 3 endpoint)
- ❌ Round 18 改动 commit(所有 round 18-19 改动仍 uncommitted,per handoff §五)

---

## 十、Reference

- Path B 规划: `handoff-2026-09-30-fix-ipc-http.md`
- IPC 根因: `handoff-2026-09-30-integration-ipc-mismatch.md`
- Round 18 二进制修复: `handoff-2026-09-29-sidecar-flash-focus-fix.md`
- Extension 注册机制: `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md`
- Spike 源码: `sigma-listary-spike/src/`
- Extension 源码: `release/extension/dist/index.js`
- Smoke test 脚本: `C:\Temp\smoke-test-http.ps1`(可重用,PowerShell 5.1 兼容)

---

## 十一、Round 19b 补丁 — main loop bug(2026-09-30 08:26 发现)

**Symptom**(用户报告,截图日志):
```
00:24:22 dialog detected hwnd=3281174
00:24:22 DialogDetected
00:24:23 Read path="verysync-windows-amd64-v2.32.2 (1).zip"
00:24:23 spike exiting cleanly    ← 第一个 dialog event 后退出
```

**Root cause**:`main.rs` 的 `tokio::select!` 没有包在 `loop {}` 里。任何 arm(`ctrl_c` / `dialog_event` / `sleep`)fire 后 select! 宏返回,main 走 graceful shutdown → 进程退出。这是 **round 18 的 latent bug**:
- smoke test 走 raw TCP + `/set_path` 不触发 dialog event → 没暴露
- round 18 端到端 GUI 测试未完整覆盖,bug 留存到 round 19

**Fix**(`main.rs:108-211`):
```rust
loop {
    tokio::select! {
        _ = signal::ctrl_c() => { ...; break; }
        Some(dialog_event) = async_rx.recv() => { ... /* loop continues */ }
        _ = tokio::time::sleep(...) => { continue; }
    }
}
```

**Build evidence**: cargo +1.85.0 test 15/15 PASS, build 13.30s, binary 2,453,504 B(+1024 B vs 2,452,480)

**Re-deploy**:
- ✅ Phase 1a: `release/extension/bin/focus-sync-sidecar.exe` (2,453,504 B, 08:26)
- ✅ Phase 2: installer rebuilt 838,677 B (extension-installer/ + release/ root)
- ⏭️ Phase 1b: 跳过(用户选择,等 installer 重跑)

**User 验证步骤**(新一轮):
1. **关闭旧 spike.exe 窗口**(带 bug 的)
2. 重新运行 `release/extension/bin/focus-sync-sidecar.exe`
3. Edge Save As 打开 → Sigma FM tree 切路径
4. **这次窗口会持续刷新 `path_update` 日志**,不再 spike exiting cleanly
5. Edge Save dialog 应自动跳

如果 spike 仍然自毙,可能其他路径未覆盖 — 立即写 `handoff-2026-09-30-fix-loop-v2.md` 并读 main.rs 整段排查。

---

## 十三、Round 19c 补丁 — Closed 不再 remove_dialog(2026-09-30 09:14 发现)

**Symptom**(用户报告 2026-09-30 01:09-01:11,完整 spike 日志):
```
01:10:06 DialogDetected hwnd=12849792        ← 注册
01:10:12 path_update #2 → Write succeeded    ← ✅
01:10:24 path_update #3 → Write succeeded    ← ✅
01:10:31 dialog_closed hwnd=12849792         ← remove_dialog!
01:10:39 path_update #4 → NO Write log       ← ❌ active_dialogs 空
01:10:57 path_update #5 → NO Write log       ← ❌
01:11:15 path_update #6 → NO Write log       ← ❌
01:11:16 path_update #7 → NO Write log       ← ❌
01:11:19 dialog detected hwnd=12849792       ← 重新检测(同一 hwnd)
01:11:19 dialog lost hwnd=12849792           ← 122ms 内 bounce
01:11:20 read_failed UIA 0x80040201           ← Opened 处理时 read 失败
01:11:20 dialog_closed                       ← 又 remove 了
```

**User 现象**:"切换sigma和下载弹窗实现了路径同步;但超过三次更换sigma路径后,又无法同步路径了"

**Root cause**:
1. 第一次 Save As dialog 被 monitor 检测 → register_dialog
2. 前 3 次 /set_path 写入成功
3. 第 4 次之前 monitor 报告 dialog lost(可能 flicker / DevTools 抢焦点 / Edge Save As 重建 hwnd),main 调 `remove_dialog` → registry 空
4. 后续 /set_path 找不到 dialog → 静默 no-op
5. 即使 dialog 重新出现,bounce Opened/Closed 让 register/remove 抵消

**Fix**(`main.rs` Closed 分支):
```rust
// 之前: state.remove_dialog(hwnd_u32);
// 现在: 不调用,只 log
log_event(&Event::SpikeError {
    kind: "dialog_closed".into(),
    message: format!("... hwnd={hwnd_u32} (kept in registry for write resilience)"),
    ts: now_iso8601(),
});
```

**Trade-off 接受**:
- Stale entry(hwnd 真销毁)不会被主动清理
- 但 `writer::write_path` 写失败会 log `WriteFailed`(已有逻辑),用户能看到
- 长期可加 periodic sweep,但 YAGNI

**Build evidence**: cargo +1.85.0 test 15/15 PASS, build 10.32s, binary 2,452,992 B(-512 B vs round 19b)

**Re-deploy**:
- ✅ Phase 1a: `release/extension/bin/focus-sync-sidecar.exe` (2,452,992 B, 09:14)
- ✅ Phase 2: installer rebuilt 838,090 B
- ⏭️ Phase 1b: 跳过

**User 验证步骤**:
1. 关掉旧 spike.exe 窗口
2. 重新运行新 binary
3. Edge Save As → Sigma FM tree 切 ≥ 4 次,观察:
   - **所有 4+ 次都应该同步**(之前第 4 次起会失败)
4. 关闭 Save As → 再开 Save As → Sigma FM tree 切,验证恢复能力

如果仍失败,说明 stale entry 累积干扰了 UIA 写,需加 periodic sweep。

---

## 十二、下次会话第一件事(端到端验证收尾)

1. **用户已重启 Sigma FM + 跑新 installer**(本会话 Phase 1b 跳过后的回填步骤)
2. **读本 handoff §六**,按步骤 1-6 跑端到端验证
3. **如 spawn 失败**: 检查 manifest permissions + sandbox validator(参考 memory §5.7 19 个 regex)
4. **如 PUSH 200 但 dialog 不跳**: 检查 UIA monitor 是否检测到 dialog(state.active_dialogs() 计数)
5. **如一切 OK**: commit round 19 + round 18-19 一起(本会话改动均未 commit)