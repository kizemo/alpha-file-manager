# Handoff: focus-21 v0.5.5 — Step 1+2 PASS, Step 3 BLOCKED (Sigma FM 未装)

**接力**:Claude (MiniMax-M3) → user (next session in same machine)
**日期**:2026-10-06 20:30
**分支**:`feat/recovery-2026-09-28`(working tree,未 commit)

## TL;DR

| Step | 状态 |
|---|---|
| **Step 1** cargo test + build | ⚠️ **PARTIAL** — build ✅; 1 个 pre-existing test bug(`resolve_port_strict_on_conflict`,`#[should_panic]` 与 `Result` 不匹配,**不是 v0.5.5 引入的**,按 §7 不改)|
| **Step 2** binary probe | ✅ **PASS**(5/5 字符串 + version=0.5.5)|
| **Step 3** UI 实测 | ⚠️ **PARTIAL** — smoke PASS(API + flag 转换);视觉 UI 测试**本机无法执行**(Sigma FM 未装)|
| **Step 4** 部署 | ⏸️ **DEFERRED**(Step 3 未全 PASS,按 §5 step 4 条件)|
| **Step 5** commit + push | ⏸️ **DEFERRED**|

## 本会话执行内容

### Step 1
- 用 `rustup` 找到 cargo 1.85.0 toolchain(默认 1.97.1 让 `once_cell` 编译失败)
- `cargo test --release`:**52 pass;1 fail**(`config::tests::resolve_port_strict_on_conflict`)
- `cargo build --release`:✅ **spike.exe 2,847,744 bytes,Oct 6 20:23**(≥ 2,792,960 期望)

### Step 2(binary 字符串探针)
| grep | 期望 | 实际 |
|---|---|---|
| `v0.5.3 SendInput` | 0 | 0 ✅ |
| `v0.5.4 SendInput` | 0 | 0 ✅ |
| `v0.5.5 SendInput` | 5 | 5 ✅ |
| `Do NOT revert`(注释) | 0 | 0 ✅ |
| `compute_score_for_automation_id`(strip) | 0 | 0 ✅ |
| `0.5.5` 总数 | > 0 | 6 ✅ |
| `0.5.4` / `0.5.3` 总数 | 0 | 0 / 0 ✅ |

### Step 3 smoke(启 sidecar 38421,curl 验证)
| 端点 | 期望 |
|---|---|
| `spike.exe --port 38421` 启动 | ✅ `mode=spawn, session_id=1` |
| `GET /health`(初始)| ✅ `version="0.5.5", first_push_received=false, uia_healthy=true` |
| `POST /set_path {C:/downloads}` | ✅ `{"ok":true}`;`first_push_received` → **true** |
| `POST /set_path {D:/projects}`(不同 path)| ✅ `{"ok":true}`;flag 保持 true |
| `POST /set_path {X:/invalid}` | ✅ ok(无 drive 校验)|
| `POST /set_path {path:""}`(空)| ✅ `{"ok":false,"error":"path is empty"}`(400)|
| `POST /quit` | ✅ 干净退出 |

Fix B (PUSH 设 flag):✅ 运行时验证
Fix N-4 (dedup branch mark):✅ code-verified(http_server.rs:184-199)

### Step 3 视觉 UI 测试 — BLOCKED
- 本机搜不到 Sigma File Manager 安装目录(`D:\Program Files\`、`C:\Program Files\`、E: 盘均无;仅 `E:\ClaudeBackup\shadow-appdata-20260927-0008\` 有历史扩展数据)
- 无法测试 Edge Save As 对话框、Sigma FM DevTools、新建文件夹 41477-only 行为

## 必读文档
1. `.superpowers/sdd/handoff-2026-10-06-focus-21-v0.5.5-ship/progress.md` — 完整 ledger(含 ruling + 测试证据)
2. `docs/superpowers/plans/2026-10-06-focus-21-v0.5.5-ship-ready-plan-v6.md` — 最终 ship-ready plan
3. `handoff-2026-10-06-focus-21-v0.5.5-ship.md` — 上一轮 handoff

## 下一步 user 必做

1. **装 Sigma File Manager** 到 `D:\Program Files\Sigma File Manager\`(按 memory 历史位置)
2. **执行 §5 Step 3 视觉 UI 测试**(Edge Save As + Sigma FM DevTools + 新建文件夹)
3. **Step 3 全 PASS 后**:
   - Step 4 部署:`cp target/release/spike.exe D:/Program Files/Sigma File Manager/tools/focus-sync-sidecar.exe` + extension dist
   - Step 5 commit + push:`git commit -m "feat(focus-sync): v0.5.5 — ..."; git push origin feat/recovery-2026-09-28`

## Rulings 已记

1. Toolchain 用 rustup 1.85.0(不是默认 1.97.1)— once_cell 1.20.2 在 1.97.1 编译失败
3. 不改 `resolve_port_strict_on_conflict` test(`#[should_panic]` 错但 §7 禁止改测试)
4. 不部署/不 push(Step 3 视觉测试未执行,本机缺 Sigma FM)

## Production 二进制状态

- `target/release/spike.exe`(本地开发目录):**v0.5.5,2,847,744 bytes,Oct 6 20:23**(新)
- `D:\Program Files\Sigma File Manager\tools\focus-sync-sidecar.exe`(production):**未触**(仍 v0.5.4)
- 12 Fix 源码:`feat/recovery-2026-09-28` 工作树,**未 commit**(等 user 跑完 Step 3 后一次性 commit)