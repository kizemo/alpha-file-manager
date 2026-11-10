# Handoff — Sigma FM Extension ↔ Sidecar IPC Protocol Mismatch (2026-09-30)

**Inherits**: `handoff-2026-09-29-sidecar-flash-focus-fix.md` (round 18 — binary fix DONE)

**Triggered by**: Phase 3 sandbox verify of round 18 deployment. After deploying round-18 binary + restarting Sigma FM, extension didn't spawn sidecar. Investigation revealed pre-existing IPC protocol mismatch.

---

## 一、核心发现 (CRITICAL)

**Sigma FM extension `release/extension/dist/index.js` uses HTTP POST for IPC, but sidecar `sigma-listary-spike/src/tcp_server.rs` only accepts raw TCP + line-based JSON.**

Concretely:

| 端 | 协议 | 代码位置 |
|---|---|---|
| Extension (Sigma FM side) | HTTP POST `http://127.0.0.1:37421/set_path` JSON body `{"path":"..."}` | `dist/index.js:30-35` (`sigma.http.request` call) |
| Extension deactivate | HTTP POST `http://127.0.0.1:37421/quit` | `dist/index.js:119` |
| Sidecar (spike binary) | Raw TCP, line-based JSON `{"cmd":"set_path","path":"..."}` | `tcp_server.rs:1-7` (file header comment) + `tcp_server.rs:60-83` (`handle_connection` uses `BufReader::read_line` + `serde_json::from_str`) |

**HTTP request 进 sidecar → serde 解析失败 → 返 `{"ok":false,"error":"invalid escape..."}` 然后断开连接**(实测:第一次 HTTP POST 测试 hang,因为 sidecar 等待 line-based 输入)。

---

## 二、为什么这是 pre-existing 问题

| 证据 | 路径 |
|---|---|
| Round 18 改动只动 spike 7 文件,**没碰** extension JS / manifest / CLI | `handoff-2026-09-29-sidecar-flash-focus-fix.md §2.1` |
| Sidecar CLI **从未**实现 `--ipc` / `--ipc http` flag | `sigma-listary-spike/src/main.rs:22-31` (Cli struct 三个 flag:`--port` `--initial-path` `--app-whitelist`) |
| `tcp_server.rs` 始终 raw TCP 实现,从未加 HTTP 层 | `sigma-listary-spike/src/tcp_server.rs:60-83` |
| Extension JS 始终 HTTP (mtime 2026-09-29 21:30,昨天,未改) | `release/extension/dist/index.js` file mtime |

**含义**: Focus Sync 集成 **从来没真正工作过**(协议层不兼容是架构问题,不是回归)。

---

## 三、Round 18 实际成果 vs 集成状态

| 层 | Round 18 状态 |
|---|---|
| Spike binary (round 18 改动) | ✅ Fix done: fg_bypass 撤回 + DialogEvent enum + dedup + on-demand write |
| Spike binary build/test | ✅ 15/15 unit tests PASS, smoke test (raw TCP get_status/set_path/quit) PASS |
| Binary deployment (3 locations) | ✅ release/extension/bin + per-user + installer rebuilt |
| **Sigma FM extension → sidecar IPC** | ❌ **协议层不兼容,从来没工作过** |
| **Sigma FM extension auto-spawn sidecar** | ❌ **未验证**(可能是 clap reject `--ipc http` 未知 arg,可能 sandbox validator,可能其他) |

**结论**: Round 18 的 2 个 bug fix(filename flash + focus steal)在 spike binary 内层是 done 且 verified by smoke test。但**端到端 GUI 回归测试**(Edge Save dialog + Sigma FM 切路径 → sidecar 写 dialog → 观察 flash/focus)**当前无法完成**,因为 Sigma FM extension 跟 sidecar 根本不通。

---

## 四、为什么 4 个 stale sidecar 在跑 (2026-09-30 07:51 发现)

- PID 26716 (startTime 22:36:35 09-29) — 持有 port 37421
- PID 44876 (startTime 22:36:37 09-29)
- PID 10864 (startTime 23:07:12 09-29)
- PID 13476 (startTime 23:07:11 09-29)

**理论**:
1. 昨天用户多次手动启动过 sidecar(用 `--port 37421` raw TCP args)
2. Extension `runWithProgress` 可能成功 spawn(如果 args 转发到 binary 时 clap lenient mode 或 args 在 spawn 前被剥掉 `--ipc http`)
3. 但即便 spawn 成功,HTTP/TCP 协议不兼容使 IPC 无效
4. 多次 Sigma FM 重启没杀干净旧 sidecar(deactivate 的 `quit` HTTP 也走不通,所以旧 sidecar 一直跑)

---

## 五、用户验证 Phase 3 的两条可行路径

### 路径 A — 手动 raw TCP bypass Sigma FM (推荐,5 min)

**绕过 Sigma FM extension,直接测 spike binary dialog 写行为**:

1. 手动启动 round-18 sidecar (raw TCP):
   ```powershell
   Start-Process 'C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe' -ArgumentList '--port','37421','--initial-path','C:\Users\Public' -WindowStyle Hidden
   ```

2. 打开 Edge → 任意下载 → "Save As" dialog (焦点在 Edge,不关)

3. PowerShell B 窗口发 set_path (PowerShell 5.1 用 `TcpClient`,HttpClient 不可用):
   ```powershell
   $client = New-Object System.Net.Sockets.TcpClient('127.0.0.1', 37421)
   $stream = $client.GetStream()
   $json = '{"cmd":"set_path","path":"D:\downloads"}' + "`n"
   $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
   $stream.Write($bytes, 0, $bytes.Length)
   $stream.Flush()
   Start-Sleep -Milliseconds 500
   (New-Object System.IO.StreamReader($stream)).ReadLine()
   $client.Close()
   ```

4. 观察:
   - ✅ Edge Save dialog 自动跳到 `D:\downloads` (round 18 fix 核心证明)
   - ✅ filename 框**不闪动**(round 17 旧 bug 没了 — setValue 不再 8ms poll 循环)
   - ✅ Edge dialog 不抢 Sigma FM 焦点(fg_bypass 撤回后)

### 路径 B — 修 IPC 协议不兼容 (深入,1-2 hours)

修复方向任选:
1. **在 spike 加 HTTP server**: 用 `axum` / `hyper` / `tiny_http` crate,handler `POST /set_path` `POST /quit` `GET /health`
3. **改 extension 用 raw TCP**: 把 `sigma.http.request` 换成 `sigma.tcp.connect` (Sigma FM API 是否暴露 TCP API 待查;若没,只能走 #1)

**修完后再验证**: 走路径 A 同样步骤,但 IPC 走 HTTP/TCP 一致。

---

## 六、当前部署状态(本次会话已做完)

| Artifact | 路径 | 大小 | 时间 |
|---|---|---|---|
| Round-18 binary (staged) | `C:\Temp\focus-sync-round18\spike.exe` | 2,310,144 B | 2026-09-29 23:18 |
| Round-18 binary (project source) | `F:\soft\00selfmade\filemanager\sigma-listary-spike\target\release\spike.exe` | 2,310,144 B | (round 18 build) |
| Round-18 binary (release/extension/bin/) | `release/extension/bin/focus-sync-sidecar.exe` | 2,310,144 B | 2026-09-30 07:29 |
| Round-18 binary (per-user, Sigma FM 运行时拉) | `%APPDATA%\...\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe` | 2,310,144 B | 2026-09-30 07:50 |
| Round-18 installer (extension-installer/) | `release/extension-installer/kizemo.focus-sync-0.2.0-setup.exe` | 803,539 B | 2026-09-30 07:29 |
| Round-18 installer (release/ root) | `release/kizemo.focus-sync-0.2.0-setup.exe` | 803,539 B | 2026-09-30 07:29 |

**Git status**: 主仓库分支 main,HEAD `286287f docs(recovery): pivot Phase 2a from Tauri sidecar to Sigma plugin route`,round 18 改动**全部未 commit**(per handoff §三). 9 个 untracked 文件包括本 handoff + round 17-19 多个 handoff + release/ 目录。

**已知 issue** (build.ps1): `release/extension-installer/build.ps1:42` Join-Path 给 3 个位置参数,Join-Path 只接 2 个 → 顶层 installer copy 失败。本会话用 `cp -f` 手动完成。修法:`Join-Path (Join-Path $PSScriptRoot '..') 'kizemo.focus-sync-0.2.0-setup.exe'` 或字符串拼接。

---

## 七、本次会话 active 进程状态

- **Sidecar (手动启动 round-18,raw TCP)**: PID 47636,startTime 2026-09-30 07:55:16,port 37421 listening。**留给用户做路径 A 验证**。
- **Sigma FM (重启过 2 次)**: 当前 PID 20232,startTime 2026-09-30 07:53:19。Extension 未 spawn sidecar(因协议不兼容 / spawn 失败)。
- **Stale sidecars**: 4 个全部 kill,port 37421 现在归 PID 47636。

用户可以现在就用路径 A 验证 round-18 修复,无需新会话。如果要走路径 B 修 IPC,需要新会话(本会话已用大量 round,见预算)。

---

## 八、新会话第一件事

1. 读本 handoff (§一 - §五 是核心,§六 - §七 是状态)
2. 决定走路径 A (验证现有 round-18) 还是路径 B (修 IPC 协议层)
3. 路径 A: 用户手动跑 §五 步骤 1-4 验证 + 报告观察结果(闪动?焦点?);Agent 不需要动作
4. 路径 B: 读 `sigma-listary-spike/Cargo.toml` 选 HTTP crate → 加 `tcp_server::run_http_server`(或新文件 `http_server.rs`)→ 同步改 `main.rs` 启动逻辑 → `cargo +1.85.0 build --release` → 重新部署 + 路径 A 验证

---

## 九、Out of scope (继续不做)

- ❌ 改 spike binary round 18 源码(已 done)
- ❌ 改 extension JS / manifest(已稳定,且不是 round 18 scope)
- ❌ 改 build.ps1 bug(下次会话修)
- ❌ 覆盖 staging 再来一次(已 done)
- ❌ 修 SIGMA FM engine compat check / sandbox validator / activate() 不 fire 的潜在问题(若走路径 B 修 IPC 后仍 spawn 失败,再 debug)

## 十、Reference

- Round 18 handoff: `handoff-2026-09-29-sidecar-flash-focus-fix.md`
- Sigma FM extension registration memory: `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md`
- Spike source code: `sigma-listary-spike/src/`
- Extension source code: `release/extension/dist/index.js`
- Manual test checklist (old,适用 spike standalone): `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`