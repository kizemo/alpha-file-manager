# Path B — Fix Focus Sync IPC protocol mismatch

## Inherits

- `F:\soft\00selfmade\filemanager\handoff-2026-09-30-fix-ipc-http.md` (本路径规划,全上下文)
- `F:\soft\00selfmade\filemanager\handoff-2026-09-30-integration-ipc-mismatch.md` (根因 + Path A 决策背景)
- `handoff-2026-09-29-sidecar-flash-focus-fix.md` (round 18 修复上下文)

## 必交付清单

1. 在 `sigma-listary-spike/src/` 加 HTTP server module(推荐 `tiny_http`,最少依赖)
2. 实现 `POST /set_path` / `POST /quit` / `GET /get_status` 三个 endpoint(参考 handoff §二方案 A)
3. `main.rs` 把 `tcp_server::run_server` 换成 HTTP server;`lib.rs` 同步 `pub mod`
4. `cargo +1.85.0 check / test / build` 全过(15/15 unit test 不变,build ~50s)
5. Smoke test 三 endpoint(用 PowerShell `TcpClient` 手发 HTTP/1.1,验证 200 response)
6. 重做 Phase 1a/1b/2 deployment(参考 handoff §六.3;Phase 1b 需 Sigma FM 关闭)
7. 顺手修 `build.ps1:42` Join-Path bug(handoff §三)

## 环境/基线

- Working dir: `F:\soft\00selfmade\filemanager\`
- Branch: main,HEAD `286287f` (round 18 改动未 commit)
- 当前 Sigma FM (PID 20232) 在跑,Sidecar (PID 47636) raw TCP 占 port 37421(可 kill)
- 预算 <2h / Bash <80

## 禁止事项

- ❌ 不要恢复 `fg_bypass.rs`(已删,QwenLM attribution 在 git history)
- ❌ 不要改 extension JS / manifest / `dist/index.js`
- ❌ 不要在 dialog event 分支加回 write(round 17 闪动 loop 根因)
- ❌ 不要用 default stable cargo(必须 `+1.85.0`)
- ❌ 不要用 `axum`(transitive deps 多,优先 `tiny_http` 或手写最小 HTTP 解析)
- ❌ 不要保留 raw TCP 协议(方案 A 全替换,除非方案 B 有强理由)

## 端到端验证(用户手动)

1. 重启 Sigma FM(extension 应自动 spawn sidecar,因协议已兼容)
2. Edge Save As dialog + Sigma FM 切路径,观察: dialog 自动跳 / filename 不闪动 / Sigma FM 不失焦点