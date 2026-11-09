# Focus Sync 侧车生命周期移交：由 AFM 主进程持有（2026-11-09）

## 一、目标

把 focus-sync 侧车（`focus-sync-sidecar.exe`）的生命周期所有者，从 **Windows 计划任务**
（`KizemoFocusSync`，AtLogOn 触发 + `--service` 参数）改为 **Alpha File Manager（AFM）进程本身**：

- AFM 启动 ⇒ 侧车启动
- AFM 退出（正常退出或崩溃） ⇒ 侧车退出

原机制的两个问题：① 登录即常驻，用户不开 AFM 也占着 37421/42721 端口与 UIA 轮询；
② 计划任务与安装器、扩展三者状态互相牵连，卸载/升级后容易留孤儿。

## 二、结论：已验证

真机验收脚本 `.superpowers/sdd/accept-v061-fs.ps1`，`VERDICT=PASS`，逐项实测：

| 判据 | 期望 | 实测 |
| --- | --- | --- |
| 静默安装 | exit 0 | `INSTALL_EXIT=0` |
| 安装器落 `tools\focus-sync-sidecar.exe` | 存在 | `EXE_OK 2872320` `D:\Program Files\Alpha File Manager\tools\focus-sync-sidecar.exe` |
| 安装器同步 `%APPDATA%` 副本 | 存在 | `EXE_OK 2872320` `...\extensions\kizemo.focus-sync\bin\focus-sync-sidecar\` |
| legacy 计划任务被删 | 不存在 | `TASK_PRESENT=FALSE` |
| 侧车由 AFM 拉起 | 父进程 = AFM pid | `PARENT_IS_AFM=True`（sidecar 371676 / AFM 324368） |
| 启动参数 | 不带 `--service` | `--port 37421` |
| HTTP 可用 | 200 | `HEALTH_STATUS=200`，`version=0.6.0`，`mode=spawn` |
| 强杀 AFM 后侧车被回收 | 消失 | `WATCHDOG_REAPED=True AFTER_SEC=1` |
| 端口释放 | 无监听 | `PORT_37421_LISTENERS=0` |

安装包：`sigma-file-manager\src-tauri\target\release\bundle\nsis\Alpha File Manager_3.13.4_x64-setup.exe`
（45,480,572 bytes / 2026-11-09 22:52:56）。

## 三、改了什么

### 1. 新建 `src-tauri/src/focus_sync_sidecar.rs`（约 4.7 KB）

- `SIDECAR_FILE_NAME = "focus-sync-sidecar.exe"`、`SIDECAR_PORT = 37421`、`CREATE_NO_WINDOW = 0x0800_0000`。
- `FocusSyncSidecarState { child: Mutex<Option<Child>> }`，用 `app.manage()` 存句柄。
- `resolve_sidecar_path`：① `current_exe()?.parent()/tools/focus-sync-sidecar.exe`（安装器 `bundle.resources` 提供）
  ② `app.path().app_data_dir()?/extensions/kizemo.focus-sync/bin/focus-sync-sidecar/focus-sync-sidecar.exe`。
  **不用 `app.path().resource_dir()`** —— 全仓 `src-tauri/src/*.rs` 无此用法。
- `start()`：找不到二进制只记 `log::info!` 并照常 `manage(state)`；找到则 `Command::new(path)`
  + `--port 37421` + 三个 `Stdio::null()` + `creation_flags(CREATE_NO_WINDOW)`。
  **绝不传 `--service`**：该 flag 会同时关掉心跳与父进程看门狗。
- `stop()`：`try_state` → `lock()` → `take()` → `kill()` + `wait()`。

### 2. `src-tauri/src/lib.rs` 三处接线

- `:40` `mod focus_sync_sidecar;`（按字母序，插在 `pub mod mcp;` 前）
- `:658-660` `focus_sync_sidecar::start(app.handle());`，紧跟 `webview_guard_runtime::start_webview_guard(...)`
- `:604-608` 只在 `RunEvent::Exit` 上 `focus_sync_sidecar::stop(app_handle)`。
  **不放在 `ExitRequested`**：托盘逻辑可能取消退出，此时不该拆侧车。

### 3. 侧车 `extensions/kizemo.focus-sync/sidecar/`

- `src/http_server.rs:133` `SPIKE_VERSION` → `"0.6.0"`（`/health` 的 `version` 取自这里，不是 `Cargo.toml`）。
- `src/main.rs`：新增 `spawn_parent_watchdog()`（`CreateToolhelp32Snapshot` 取父 pid →
  `OpenProcess(PROCESS_SYNCHRONIZE)` → `WaitForSingleObject(INFINITE)` → 父进程消失即 `exit(0)`），
  在 `if !cli.service` 分支里启动；`--service` 降级为 legacy 语义。
- `Cargo.toml`：`windows` crate features 增 `Win32_System_Diagnostics_ToolHelp`。

**兜底三层**：① AFM 正常退出 → `stop()` 主动 kill；② AFM 崩溃/被强杀 → 侧车看门狗 1 秒内退出；
③ 都没生效（极端） → 下次 AFM 启动时新侧车在非 service 模式下清理占用 37421 的孤儿实例。

### 4. `src-tauri/installer/hooks.nsh`

- STAGE 9（1000-1015 行）：`Stop-ScheduledTask` + `Unregister-ScheduledTask -Confirm:$$false`，
  **新老用户都删** `KizemoFocusSync`（避免遗留 `--service` 实例与新区车争端口）。
- 1020-1024 行：只删 `tools\register-scheduled-task.ps1`，**保留 `tools\focus-sync-sidecar.exe`**。
- STAGE 7（948-982）/ STAGE 8（990 行写 `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath`）未改。

### 5. 扩展回退到 HEAD

`extensions/kizemo.focus-sync/dist/index.js` 与 `package.json` 用 `git checkout --` 回退到 HEAD
（0.5.9，无 `shell` 权限），并同步覆盖部署目录
`%APPDATA%\com.sigma-file-manager.app\extensions\kizemo.focus-sync\`。
本轮在扩展里手加过的 `SIDECAR_EXE` / `probeSidecarVersion` / `quitStaleSidecar` / `ensureSidecar` 已全部不在代码里。

## 四、为什么不是「扩展激活时拉起侧车」（已证伪，不要重走）

扩展侧 `ensureSidecar()` 路线实测完全无效，且**不可观测**。已排除的假设：

1. ❌ 宿主清单过期 / 缺 `shell` —— 手工把 `user-extensions.json` 改成 `0.5.10 + permissions:["shell"]`
   后重启仍 `SIDECAR_FOUND=False`、`/health` 拒绝连接、`__focus_sync_trace` 为 0 条。
2. ❌ `activationEvents` 缺失 —— 宿主与磁盘清单都有 `onStartup`，`getActivationEvents` 缺省即 `['onStartup']`。
3. ❌ 扩展被禁用 —— `enabled: true`、`installPendingDependencies: false`。
4. ❌ `ensureSidecar` 的 catch 吞掉错误 —— trace 里连 `N01.activate.start` 都没有。
5. ❌ `init()` 没跑 —— 宿主日志有 `[ext-http] ... status=200 ... runStartupMarketplaceSync`
   （来自 `extensions.ts:2431`，在激活循环**之后**），证明 `init()` 走到了。
6. ❌ 侧车二进制缺失/语法错 —— 部署目录在位、`node --check` exit 0。

**仍未取证的疑点**（若将来仍要解释）：`extensions.ts` 2417 行激活循环**之前**某个前序 `await` 抛错，
或 `assertManifestEngineCompatibility(installed.manifest)` 抛错，都被 2425 行的 `console.error` 吞掉，
而本项目拿不到 webview console。⇒ 结论：**不要**把生命周期职责放在不可观测的扩展 `activate()` 里。

## 五、证据路径

```
.superpowers/sdd/accept-v061-fs.ps1        验收脚本（五步，纯 ASCII）
.superpowers/sdd/accept-v061-run.log       本轮验收输出，VERDICT=PASS
.superpowers/sdd/tauri-build-v060-fs2.cmd  打包脚本（先 copy 侧车再 tauri:build）
.superpowers/sdd/tauri-build-v061-rust-owner.log  打包日志，BUILD_EXIT=0
src-tauri/target/release/bundle/nsis/Alpha File Manager_3.13.4_x64-setup.exe
%LOCALAPPDATA%\kizemo\focus-sync\logs\spike.log    侧车日志
%LOCALAPPDATA%\com.sigma-file-manager.app\logs\Alpha File Manager.log  宿主日志（UTC，本地=UTC+8）
```

## 六、未验证 / 风险

- **老 `--service` 实例的在线迁移**：安装器删的是计划任务与注册脚本，不杀已在跑的 `--service` 进程。
  实测路径是「先卸载/清场再装」，未覆盖「升级时装着旧版且侧车在跑」的场景。
  按设计应由新侧车的非 service 模式清理占端口实例，但**本轮未实测**。
- **`ExitRequested` 被托盘取消**：此时不杀侧车（有意为之），AFM 仍在跑，侧车应继续可用 —— 未单独实测。
- **扩展侧的侧车发现**：扩展仍靠 `HKLM\SOFTWARE\kizemo\focus-sync\SidecarPath` 找二进制。
  本轮已验证侧车由 AFM 拉起且 `/health` 正常，但**未验证扩展业务链路端到端**
  （`extension_alive=false`、`first_push_received=false` 是刚启动 0 秒的正常值）。
- 版本号仍是 **3.13.4**（本轮未发版）。正式发布前需 `release:prepare` bump。

## 七、本轮工具坑（已踩实）

1. **同版本重构建的安装包不会覆盖 `release/` 顶层**：`scripts/publish-bundle-to-release.mjs`
   发现目标同名但内容不同时，会改写进 `release\local-builds\` 并告警（脚本 50-78 行）。
   所以「我刚刚打包产出的那个包」在
   `release\local-builds\Alpha File Manager_3.13.4_x64-setup.exe`（本轮 45,480,572 / 22:52:56），
   而 `release\` 顶层那个 45,359,512 / 17:10:27 是**更早**的产物。
   验收脚本引用安装包时必须用新产物的绝对路径
   （或直接用 `src-tauri\target\release\bundle\nsis\` 下那个），别指向 `release\` 顶层。
2. **`.cmd` 运行期间绝对不能编辑**（cmd.exe 按字节偏移逐行读，改了就执行错位片段）。
3. **`.cmd` 的 `echo` 里不能出现 `>`**（会被当重定向符，还会在仓库根造垃圾文件）。
4. **后台构建不要挂 stdout 管道**（`Tee-Object` / `Select-String` 会让进程树被标 aborted），
   日志由 pwsh 侧 `*>` 持有；读运行中的日志只能读 pwsh 侧那个句柄。
5. `git checkout --` 回退扩展源码后，**必须**再覆盖部署目录
   （`%APPDATA%\com.sigma-file-manager.app\extensions\...`），否则装包前跑的仍是手改版本。
6. 侧车单体编译用 `--manifest-path`，不要 `Set-Location`；toolchain 目录名里的 `pc-` 不能省。
7. `windows` crate 0.62 的 `OpenProcess` 第二参是 `bool`（写 `FALSE` 会 `E0308`）。
8. NSIS 里 PowerShell 的 `$false` 必须写 `$$false`。

## 八、下一步

- 推送两个仓到 GitHub（用户 2026-11-09 明确要求**暂缓**，只做本地提交）。
- 若要继续收敛：补一条「升级时装着旧版且 `--service` 侧车在跑」的验收分支。
- 若将来要让扩展自己拉起侧车：先解决 `activate()` 不可观测问题（把 `console.error` 换成可落盘的 trace）。