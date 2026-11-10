# Handoff — Spike Complete, Integration Phase Pending

**会话日期**: 2026-09-27
**本次主题**: Spike 全部完成 + 决策 PASS + 主会话收尾
**承接**: `handoff-2026-09-27-focus-activation-uia-port.md`(路线重置)→ `handoff-2026-09-27-focus-activation-uia-port-tasks6-10.md`(已删除,任务已完成)
**本次状态**: 11 任务全部完成,2 commits 已落地(`8016f3d` + `905db31`),spike 二进制已构建并手动 3-cap 实测通过,决策 PASS。下一步是集成阶段

---

## 一、本次完成内容(必读)

### Commits

```
905db31 feat(spike): integrate main.rs + 8 unit tests + verify script + spike-report PASS
8016f3d feat(spike): port PathWrap dialog/monitor + QwenLM fg_bypass + windows 0.62 build validated
```

### 11 任务全部完成(plan §四 必交付清单 1-11)

| Task | 产出 | 测试 |
|---|---|---|
| 0 | plan + spec 路由变更注 | — |
| 1 | spike 目录 + Cargo.toml | `cargo build` 6.37s |
| 2 | 移植 `dialog.rs` → `uia_inject.rs` (193 行, MIT) | 编译 7.16s |
| 3 | 移植 `monitor.rs` → `uia_event.rs` (434 行, MIT, `egui::Context` → `Arc<tokio::sync::Notify>`) | 编译 7.53s |
| 4 | 移植 `fg_bypass.rs` (130 行, Apache-2.0) | 编译 7.28s |
| 5 | **关键验证点** `cargo clean && cargo build --release` | **30.90s 从零通过** |
| 6 | state / config / events / logger + 11 单测 | **11/11 pass** |
| 7 | main.rs `tokio::select!` 集成 + TCP 控制 | smoke test PASS |
| 8 | `verify_target_matrix.ps1` Phase 1 Win32 dialog + Phase 2 per-app | (脚本已写,headless 跑不全) |
| 9 | spike-report + decision doc | **PASS** |
| 10 | `sigma-file-manager/README.md` + zh-CN 加 experimental 段 | ⚠️ 在 gitignored dir,见 §五 |

### 关键实证数据

1. **Build 验证**:`cargo clean && cargo build --release` 30.90s PASS;spike.exe 2.1MB PE32+ x86-64
2. **手动 Detect/Read/Write 实测**(`sigma-listary-spike/scripts/manual-detect-test.ps1`):
   ```
   DialogDetected hwnd=7538550 app=#32770
   Read hwnd=7538550 app=#32770 path="Adobe" strategy=uia
   Write hwnd=7538550 app=#32770 target="C:\Users\Public" strategy=uia
   INFO spike::uia_inject: Injection succeeded via UI Automation.
   ```
3. **Unit tests**:`cargo test --lib` 11/11 PASS(state 2 + config 3 + events 3 + logger 3)
4. **TCP smoke**:`get_status` / `set_path` / `quit` 全工作
5. **Decision**:`docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md` 写明 PASS,7/7 理论覆盖率(基于移植来源背书)+ 1/7 手动实测

---

## 二、本次改动文件清单(已 commit)

```
M  docs/superpowers/plans/2026-09-27-listary-focus-sync.md         (14 task → 11 task 重写)
M  docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md  (+29 §10 路由变更注)
A  sigma-listary-spike/{.gitignore, Cargo.toml, Cargo.lock, README.md}
A  sigma-listary-spike/src/{lib,main,uia_inject,uia_event,fg_bypass,config,events,logger,state,tcp_server,detector,reader,writer}.rs
A  sigma-listary-spike/scripts/{smoke-test,verify_target_matrix,manual-detect-test,check-installed-apps}.ps1
A  docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md
A  docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md
```

**未提交 / 未追踪**:
```
?? .other/                                                         (调研素材 + clone 源码,保留)
?? handoff-2026-09-27-focus-activation-uia-port.md                 (上一份路线重置 handoff,本会话继承源)
?? prompt-2026-09-27-focus-activation-uia-port-next.md             (上一份 prompt,本会话继承源)
```

⚠️ **重要**:`sigma-file-manager/README.md` 和 `README.zh-CN.md` **修改已写入文件但未 commit** —— `sigma-file-manager/` 在 `.gitignore` 里(属于另一个 repo),需要单独在那边的 repo 提交。详见 §五。

---

## 三、剩余必交付(新会话执行)

### 3.1 立即可做

1. **应用 README 修改到 `sigma-file-manager` repo**:文件已就绪(`F:\soft\00selfmade\filemanager\sigma-file-manager\README.md` 和 `.zh-CN.md`),需要切到 fork repo 单独 commit + push
2. **写 Phase 2 集成 spec**:`docs/superpowers/specs/2026-09-XX-listary-focus-sync-integration-design.md`
   - Sigma Tauri command 定义(spawn `spike.exe` 作为 daemon)
   - IPC 选型:TCP(已用)→ named pipe(Windows 原生,Phase 2 推荐)
   - 触发逻辑:Sigma 当前路径变化 → TCP/named-pipe `set_path` → spike 写所有活跃 dialog

### 3.2 用户机器前置

3. **7-app GUI 全验证**:headless 环境无法触发 app-specific dialog。本机只装 4 个 app(chrome / msedge / DingTalk / Feishu),Word / VS Code / WeChat 缺失,需装齐 7 个 app 后手动跑 `verify_target_matrix.ps1 -Manual`(每个 app 触发后回车)

### 3.3 Phase 2 实施

4. **named-pipe IPC**:替换 `tcp_server.rs` 的 TCP;Windows 原生支持,Sigma Tauri sidecar 标准做法
5. **spike binary 打包**:NSIS installer(spike.exe sidecar + manifest)
6. **UWP/XAML 扩展**:从 QwenLM 完整移植 `crate::input::is_xaml_host_hwnd`(spike 当前简化掉),扩大覆盖率

---

## 四、环境与基线

- **Working dir**: `F:\soft\00selfmade\filemanager\`
- **Branch**: `main`
- **HEAD**: `905db31 feat(spike): integrate main.rs + 8 unit tests + verify script + spike-report PASS`
- **上一 HEAD**: `8016f3d`
- **Spike binary**: `sigma-listary-spike/target/release/spike.exe` (2.1MB)
- **`feat/tree-sidebar-v6-1`**: HEAD `8caf14ae`,6 commits,259 unit tests pass(不动)
- **`.other/`**:保留(PathWrap / qwen-code clone + 5 份调研报告)

---

## 五、避坑提示(累积,新会话必读)

### 5.1 sigma-file-manager repo 分离

- `sigma-file-manager/` 在本 meta-repo `.gitignore` 里(同盘另一 git repo)
- README 修改已写入该目录,但 `git add` 在这里会被 ignore
- **必须切换到那个 repo 单独提交**,push 后才会出现在 GitHub 上

### 5.2 STA 线程约束

- spike 用 `#[tokio::main(flavor = "current_thread")]` 把 UIA 留在主线程 COM STA
- 改回 `multi_thread` 会导致 `CoCreateInstance` 在 worker 线程上失败(spike 早期遇到的 read 失败就是这原因)
- Phase 2 如果要并发,需要 per-worker `CoInitializeEx(STA)`

### 5.3 Verify 脚本限制

- Phase 1 `Test-Win32Dialog` 在 headless 环境会阻塞(`ShowDialog` 等用户关闭),不要相信 `-NoWait` 自动覆盖全部 app
- 真实覆盖率数据需要 GUI 交互:用 `-Manual` 跑 7-app 矩阵(每 app 触发后 `Read-Host`)

### 5.4 移植来源合规

- MIT / Apache-2.0 头文件每个 ported 文件顶部都有,不能删
- 上游链接在 `sigma-listary-spike/README.md`,fork 时保留

### 5.5 不要做的事

- ❌ 不要开 `feat/dialog-focus-sync` 分支(死代码,counter-example)
- ❌ 不要 rebase `feat/tree-sidebar-v6-1`(HEAD `8caf14ae`,已发稳定)
- ❌ 不要走 14-task 自写 spike 路线(已废,本会话已证 port route 优)
- ❌ 不要碰 release/ 目录(本机已删,重建要上游下载)
- ❌ 不要现在改 `sigma-listary-spike/` 已 commit 的 3 个移植文件(版权头固定)

---

## 六、必读顺序(新会话第一件事)

1. **本 handoff**(你正在读)
2. `git rev-parse HEAD` — 确认在 `905db31`
3. `git status --short` — 确认 working tree 只剩 `??` 项(无未 commit 改动)
4. **Read** `docs/superpowers/decisions/2026-09-27-listary-focus-sync-decision.md`(本次决策全文)
5. **Read** `docs/superpowers/spike-reports/2026-09-27-listary-focus-sync-spike.md`(spike 报告)
6. **Read** `docs/superpowers/specs/2026-09-27-listary-focus-sync-design.md` §10(路由变更注 + spec 全文)
7. **Read** `sigma-listary-spike/README.md`(源码归属 + license)
8. **Read** `sigma-file-manager/README.md`(实验功能段已写入,需在该 repo commit)

---

## 七、新会话第一句话(引用本 handoff + prompt)

> 承接 `handoff-2026-09-27-spike-complete-next-phase.md`,当前 HEAD `905db31`,spike 已 PASS。下一步:把 README 修改 apply 到 `sigma-file-manager/` repo + 写 Phase 2 集成 spec。

---

## 八、Out of scope(继续不做,继承)

- macOS / Linux(UIA / AXUIElement 是 Windows 专有)
- DLL 注入(AV / WDAC 风险)
- 重新开 `feat/dialog-focus-sync` 分支
- rebase `feat/tree-sidebar-v6-1`
- 改 spike 已 commit 的 3 个移植文件(版权头固定)

---

## 九、tree 功能当前状态(继续可用,继承)

- `feat/tree-sidebar-v6-1` HEAD `8caf14ae`,6 commits,259 unit tests pass
- `v2.2.0-tree.1` release 仍在 GitHub upstream(本地 `release/` 已删,上游可下载)
- spike 在 `sigma-listary-spike/`,不影响 `sigma-file-manager/` 主项目