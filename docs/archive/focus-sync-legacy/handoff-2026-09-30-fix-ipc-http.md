# Handoff — Focus Sync IPC Fix (Path B, 2026-09-30)

**Inherits**:
- `handoff-2026-09-30-integration-ipc-mismatch.md` (root cause: HTTP/TCP protocol mismatch)
- `handoff-2026-09-29-sidecar-flash-focus-fix.md` (round 18 binary fix — done)
- `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md` (§5.7 sandbox validator + §5.5 binary path + §5.6 __binaries path storage)

**Decision (resolved 2026-09-30 by user)**: Path B — add HTTP server to spike, keep `release/extension/dist/index.js` as-is.

---

## 一、问题一句话

`release/extension/dist/index.js:30-35` 发 `POST http://127.0.0.1:37421/set_path`,但 `sigma-listary-spike/src/tcp_server.rs` 只支持 raw TCP + line-based JSON。协议层不兼容导致 Focus Sync 集成从未工作(参考 handoff §一)。

---

## 二、推荐实现路径(供新会话选择)

### 方案 A — 重写 tcp_server.rs 为 HTTP server(推荐)

**核心理念**: spike 当前 TCP 协议除了 smoke test 用,生产无消费者。可以整个替换。

**改动清单** (5 文件):
| 文件 | 改动 | 估计行数 |
|---|---|---|
| `sigma-listary-spike/Cargo.toml` | 加 `tiny_http = "0.12"`(轻量,无 async runtime overhead)或 `axum = "0.7"`(tokio-native,但 transitive deps 多) | +1-5 |
| `sigma-listary-spike/src/lib.rs` | `pub mod tcp_server;` → `pub mod http_server;` | ±1 |
| `sigma-listary-spike/src/http_server.rs` | **新建**:用 tiny_http 或 axum 实现以下 endpoints | +120-200 |
| `sigma-listary-spike/src/main.rs` | `tcp_server::run_server` → `http_server::run_server`;CLI 不变 | ±5 |
| `release/extension-installer/build.ps1` | 同步修复 §三 bug(顺手做) | ±1 |

**HTTP endpoints** (基于 `index.js` 实际调用):
- `POST /set_path` body `{"path": "D:\\downloads"}` → 200 + `{"ok": true}` 或 400 + `{"ok": false, "error": "..."}`;内部调现有 `state.set_current_path` + `writer::write_path`
- `POST /quit` 无 body → 200 + `{"ok": true}` 后 graceful shutdown
- `GET /get_status` (extension 不调,但 smoke test 需要) → 200 + `{"ok": true, "current_path": "...", "active_dialogs": N}`

**Reuse 现有代码**: `state::AppState`、`events::Event`、`writer::write_path`、`logger::log_event` 都不变。只替换传输层。

### 方案 B — 双协议 multiplex(复杂度高,不推荐)

端口 37421 listen,peek first byte 判 HTTP vs JSON,分给两个 handler。保留 raw TCP 兼容性,但代码复杂、未来维护成本高。除非 smoke test 不能改,否则选方案 A。

---

## 三、顺手修(独立小任务,必做)

**`release/extension-installer/build.ps1:42`** Join-Path 给了 3 个位置参数,Join-Path 只接 2 个 → 顶层 installer copy 失败。

修法: `Join-Path (Join-Path $PSScriptRoot '..') 'kizemo.focus-sync-0.2.0-setup.exe'`

**本会话已 `cp -f` 手动绕过**,但下次接入会再撞。

---

## 四、必须遵守的约束(违反任一 = FAIL)

| 约束 | 来源 |
|---|---|
| **cargo +1.85.0** 锁住(default 1.97.1 触发 245 dep 编译错) | memory §5.17 + handoff-2026-09-29 §六.3 |
| **不要恢复 fg_bypass.rs** (QwenLM attribution 在 git history,但已删) | memory §5.16 + handoff-2026-09-29 §六.4 |
| **不要在 dialog event 分支加回 write** (round 17 闪动 loop 根因) | handoff-2026-09-29 §六.4 |
| **不要动 extension JS / manifest / package.json** (round 18 scope 外,不是 IPC fix 范围) | 决策 §一 |
| **不要覆盖 staging 再来一次** (Phase 1a/1b/2 已 done) | handoff-2026-09-30 §六 |
| **Sigma FM extension `permissions` 必须含 `http` + host `127.0.0.1:37421`** | manifest 已有,verify 后再 build |
| **不要用 `uiautomation` 之外的 UIA 路径** (PathWrap 上游原版) | memory §5.13 |

---

## 五、当前部署状态(给新会话的环境基线)

| 状态项 | 值 |
|---|---|
| Working dir | `F:\soft\00selfmade\filemanager\` |
| Branch | main |
| HEAD | `286287f docs(recovery): pivot Phase 2a from Tauri sidecar to Sigma plugin route` |
| Round 18 改动 | 已 staged at 3 个位置, **未 commit**(per handoff-2026-09-30 §六) |
| Round 18 binary | 2,310,144 B (round 17 = 3,067,904 B, delta = 删 fg_bypass) |
| Sidecar 进程(PID 47636) | **手动启动 raw TCP 模式跑在 port 37421**,留给 Path A 验证用,可 kill |
| Sigma FM (PID 20232) | 跑着, extension 未激活(协议不兼容) |
| 9 个 untracked 文件 | 多个 handoff-2026-09-29*.md / handoff-2026-09-30*.md + release/ 目录 |

---

## 六、测试计划 (新会话执行)

### 6.1 Build + Unit Test
```bash
cd sigma-listary-spike
cargo +1.85.0 check --lib        # 必须 no warnings/errors
cargo +1.85.0 test --lib         # 必须 15/15 PASS(不变)
cargo +1.85.0 build --release    # 预期 ~50s,产物 spike.exe
```

### 6.2 Smoke Test (新会话自测)

**必须**验证 HTTP 三 endpoint:
1. `POST /set_path` body `{"path":"D:\\downloads"}` → 200 `{"ok":true}`
2. `GET /get_status` → 200 `{"ok":true,"current_path":"D:\\downloads","active_dialogs":0}`
3. `POST /quit` → 200 `{"ok":true}` 后 server 退出

PowerShell 5.1 没有 HttpClient,需要用 `TcpClient` 手发 HTTP request(参考 handoff-2026-09-30 §五路径 A 步骤 3 的格式,改 `cmd` JSON → HTTP/1.1 POST)。

### 6.3 Phase 1a/1b/2 重做

按 handoff-2026-09-29 §八路径A 重做 3 个 deployment step:
1. `cp -f target/release/spike.exe → release/extension/bin/focus-sync-sidecar.exe`
2. `cp -f → %APPDATA%\...\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
3. `cd release/extension-installer && powershell -File build.ps1`(build.ps1 bug 修后这里自动 copy 到 release/ root)

**注意**: Phase 1b 需要 Sigma FM 已关闭。

### 6.4 端到端验证(用户手动,Agent 不动作)

1. 启动 Sigma FM(extension 应自动 spawn sidecar,因为协议已兼容)
2. 打开 Edge → 任意下载 → Save As dialog(不关)
3. 在 Sigma FM 主窗口左侧 tree 切文件夹(触发 onPathChange → HTTP POST → sidecar write)
4. 观察:
   - ✅ Edge Save dialog 自动跳到新文件夹(<500ms)
   - ✅ filename 框不闪动(round 18 修复核心)
   - ✅ Sigma FM 不失顶层焦点(fg_bypass 撤回)

---

## 七、必读顺序(新会话第一件事)

1. **本 handoff**(正在读)
2. `handoff-2026-09-30-integration-ipc-mismatch.md` — Path A/B 决策背景(读 §一 + §四 + §五)
3. `handoff-2026-09-29-sidecar-flash-focus-fix.md` — round 18 binary fix 上下文(读 §一 + §二 + §六)
4. `sigma-listary-spike/src/main.rs` — Cli struct + main entrypoint
5. `sigma-listary-spike/src/tcp_server.rs` — 现有 TCP 实现作改造起点
6. `sigma-listary-spike/src/writer.rs` — write_path 实现(被 HTTP handler 调)
7. `sigma-listary-spike/src/state.rs` — AppState(被 HTTP handler 调)
8. `sigma-listary-spike/Cargo.toml` — 当前 deps(确认 +1.85.0 兼容)
9. `release/extension/dist/index.js` — extension 实际发什么(确认 HTTP contract)

---

## 八、新会话预算/红线提醒

- 单 session <2h 活跃(Bash <80),本会话已积 ~28/80
- 不要 `--resume` >5MB jsonl
- 写到 `C:\Temp\` 不要写到 `~/.tmp/` 或 `~/`
- destructive op(覆盖 staging / kill 进程)前**逐项复述影响**
- Sigma FM extension sandbox validator §5.7 检查 19 个 regex,改 spike 代码不触发(改 extension 才触发)

---

## 九、Out of scope (继续不做)

- ❌ 改 extension `dist/index.js`(已稳定,改 spike 适配)
- ❌ 改 manifest `permissions`(已有 `http` + host `127.0.0.1:37421`)
- ❌ 改 build.ps1 之外的 release/extension-installer 脚本
- ❌ 引入 HTTP framework 之外的传输方案(gRPC/WebSocket/etc.)
- ❌ 修改 `active_dialogs` 字段语义
- ❌ 重构 `state` / `writer` / `events` 模块
- ❌ 加 integration test(等 IPC 修通再补)

## 十、Reference

- IPC mismatch 根因 + 路径 A 验证: `handoff-2026-09-30-integration-ipc-mismatch.md`
- Round 18 binary fix: `handoff-2026-09-29-sidecar-flash-focus-fix.md`
- Sigma FM extension 集成机制: `E:\Users\Duanyi\.claude\projects\F--soft-00selfmade-filemanager\memory\sigma_fm_extension_registration.md`
- Spike 源码: `F:\soft\00selfmade\filemanager\sigma-listary-spike\src\`
- Extension 源码: `F:\soft\00selfmade\filemanager\release\extension\dist\index.js`
- 旧 manual checklist(仅参考): `docs/superpowers/test-plans/2026-09-27-listary-focus-sync-manual-checklist.md`