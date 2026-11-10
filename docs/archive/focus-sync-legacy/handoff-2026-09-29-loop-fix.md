---
name: 2026-09-29 handoff (round 16) - Sidecar exit-on-event bug fixed
description: Round 16. Original kizemo/focus-sync main.rs had tokio::select! outside loop, causing sidecar to exit after first dialog event. Wrapped in loop {} so sidecar stays alive across multiple dialogs.
type: project
---

# Handoff — Sidecar Exit-on-Event Bug Fixed (2026-09-29 round 16)

**Inherits**: `handoff-2026-09-29-spawn-mystery.md` (round 15 — sidecar spawning investigation)

**User reported**: "切换目录，再切换回对话框，路径不更新"
Console showed: `[focus-sync] PUSH response status=200` (succeeded once) then `push failed: Error: HTTP request failed`

## 一、Root cause

`kizemo/focus-sync/sidecar/src/main.rs:125` 用了 `tokio::select!`,**但没在 `loop {}` 里**。

`tokio::select!` 是单次宏 — 一次 arm fire 后,程序继续往下走,执行 line 234:
```rust
// Graceful shutdown.
ipc_handle.abort();
log_event(&Event::SidecarError {
    kind: "shutdown".into(),
    message: "sidecar exiting cleanly".into(),
    ts: now_iso8601(),
});
Ok(())
```

→ sidecar 退出。

所以时序是:
1. Sidecar spawn → activate() 第一次 push 成功(sidecar alive)
2. 第一个 dialog event 触发 select arm → 处理完 → 程序退出
3. 后续 push 都失败(sidecar 已死)

用户的 console log 完全符合这个模式:
```
[focus-sync] sidecar started, taskId=ext-cmd-1790689329525-1   <- 起来
[focus-sync] PUSH response status=200                             <- 第一次 OK
[focus-sync] push failed: HTTP request failed                     <- 后续 fail
```

## 二、修复

包 `loop {}` 在 select! 外面:

```rust
loop {
    tokio::select! {
        _ = signal::ctrl_c() => { ... }
        Some(dialog_opt) = async_rx.recv() => { ... }
        _ = tokio::time::sleep(Duration::from_secs(u64::MAX / 4)) => { ... }
    }
}
```

`tokio::select!` 不再 fallthrough 到 shutdown,程序永远留在 loop 里处理 dialog events。

## 三、验证

Build 成功(新 binary 3,067,904 bytes, hash `3308371C503C6CB1...`)。已部署到:
- `%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe`
- `release\extension\bin\focus-sync-sidecar.exe`(installer staging)
- `release\kizemo.focus-sync-0.2.0-setup.exe`(installer rebuilt 1,010,113 bytes)

直接测试:启动 sidecar,20s 内连续10次HTTP请求,全部接收到(PID 35420 全程 alive)。

## 四、当前状态

| Item | Value |
|---|---|
| Sigma FM PID | 48792(21:55:48 启动) |
| Sidecar binary | 新版(loop 修复 + no-click)deployed |
| Installer | 1,010,113 bytes (含 loop 修复的 sidecar) |

## 五、用户验证步骤

1. Edge 打开下载 → 弹 Save 对话框
2. 切到 Sigma FM 改目录
3. 切回 Edge 对话框 → 路径应更新
4. **关键**: 反复切换 Sigma / 对话框 多次 → **每次都更新**
5. 打开 DevTools Console,过滤 `focus-sync`,应该看到:
   - 多次 `[focus-sync] PUSH raw=... normalized=... PUSH response status=200`
   - **没有 push failed 错误**

如果反复切换都成功 = **彻底修复**
如果还是失败,可能是 WebView6 的 keep-alive 问题,需要sidecar 加 retry 逻辑。

## 六、本轮改动文件清单

```
MOD  C:\Temp\focus-sync-src\sidecar\src\main.rs  (added loop {} around tokio::select!)
MOD  C:\Users\Duanyi\AppData\Roaming\com.sigma-file-manager.app\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\focus-sync-sidecar.exe  (deployed 3,067,904 bytes, hash 3308371C...)
MOD  F:\soft\00selfmade\filemanager\release\extension\bin\focus-sync-sidecar.exe  (staging)
MOD  F:\soft\00selfmade\filemanager\release\extension-installer\kizemo.focus-sync-0.2.0-setup.exe  (rebuilt 1,010,113 bytes)
MOD  F:\soft\00selfmade\filemanager\release\kizemo.focus-sync-0.2.0-setup.exe  (top-level copy)
```

## 七、新会话第一句话参考

> 承接 `handoff-2026-09-29-loop-fix.md`(round 16)。原 kizemo/focus-sync 源码 main.rs 有 bug:`tokio::select!` 没在 loop {} 里 → 处理一个 dialog event 后 sidecar 退出 → 后续 push 都失败。修复:包 loop,新 binary 3,067,904 bytes (hash 3308371C)。已 deploy + installer rebuild。需要用户测:多次切换 Sigma / 对话框,verify path 一直更新 + console log 无 push failed。